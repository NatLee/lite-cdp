use crate::{
    credentials,
    crypto::{equal, hash, now, random, time},
    AccessError, AccessKit, CookieSession,
};
use axum::{
    body::Bytes,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use url::Url;
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Authorize {
    client_id: String,
    redirect_uri: String,
    response_type: String,
    state: String,
    scope: String,
    resource: String,
    code_challenge: String,
    code_challenge_method: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Decision {
    decision: String,
}
pub(crate) fn session_binding(s: &CookieSession) -> String {
    s.session_binding
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub(crate) fn browser(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all("cookie")
        .iter()
        .filter_map(|h| h.to_str().ok())
        .flat_map(|h| h.split(';'))
        .filter_map(|v| v.trim().split_once('='))
        .find(|(k, _)| *k == "access_browser")
        .map(|(_, v)| v.to_owned())
        .filter(|v| {
            v.len() == 43
                && v.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        })
}
pub(crate) fn browser_cookie(kit: &AccessKit, value: &str) -> Result<HeaderValue, AccessError> {
    HeaderValue::from_str(&format!(
        "access_browser={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age=300{}",
        if kit.0.config.secure_cookies() {
            "; Secure"
        } else {
            ""
        }
    ))
    .map_err(|_| AccessError::unavailable())
}
pub(crate) fn redirect(location: &str) -> Result<Response, AccessError> {
    let mut r = StatusCode::SEE_OTHER.into_response();
    r.headers_mut().insert(
        "location",
        HeaderValue::from_str(location)
            .map_err(|_| AccessError::bad("invalid_redirect", "Invalid redirect"))?,
    );
    r.headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-store"));
    Ok(r)
}
pub(crate) fn valid_next(next: &str) -> bool {
    if !next.starts_with("/oauth/authorize?") || next.len() > 8192 {
        return false;
    }
    let Ok(u) = Url::parse(&format!("https://local.invalid{next}")) else {
        return false;
    };
    u.path() == "/oauth/authorize"
        && u.fragment().is_none()
        && u.host_str() == Some("local.invalid")
}
pub(crate) async fn start(
    kit: &AccessKit,
    h: &HeaderMap,
    b: Authorize,
) -> Result<Response, AccessError> {
    let r = sqlx::query("SELECT redirect_uris FROM access_clients WHERE client_id=?")
        .bind(&b.client_id)
        .fetch_optional(&kit.0.db)
        .await?
        .ok_or_else(|| AccessError::bad("invalid_request", "Unknown client"))?;
    let redirects: Vec<String> = serde_json::from_str(&r.get::<String, _>("redirect_uris"))
        .map_err(|_| AccessError::unavailable())?;
    if !redirects.contains(&b.redirect_uri) {
        return Err(AccessError::bad(
            "invalid_request",
            "Redirect URI is not registered",
        ));
    }
    let scopes = b
        .scope
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    kit.validate_scopes(&scopes)?;
    if b.response_type != "code"
        || b.state.is_empty()
        || b.state.len() > 1024
        || b.code_challenge_method != "S256"
        || b.code_challenge.len() != 43
        || !b
            .code_challenge
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        || ![kit.0.config.url("/mcp"), kit.0.config.url("/api/v1")].contains(&b.resource)
    {
        return Err(AccessError::bad(
            "invalid_request",
            "Invalid authorization request",
        ));
    }
    let Some(session) = kit.0.host.cookie_principal(h).await? else {
        let next = format!(
            "/oauth/authorize?{}",
            serde_urlencoded::to_string(&b).map_err(|_| AccessError::unavailable())?
        );
        let location = format!(
            "{}?{}",
            kit.0.host.paths().login,
            serde_urlencoded::to_string([("next", next)]).map_err(|_| AccessError::unavailable())?
        );
        return redirect(&location);
    };
    if kit
        .0
        .host
        .active_principal(session.principal.user_id)
        .await?
        .is_none()
    {
        return Err(AccessError::unauthorized());
    }
    let browser = browser(h).unwrap_or_else(random);
    let bh = hash(&browser);
    kit.rate(format!("authorize:{}", session.principal.user_id), 10)?;
    let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM access_consents WHERE expires_at>? AND used_at IS NULL AND browser_hash=?").bind(now()).bind(&bh).fetch_one(&kit.0.db).await?;
    if count >= 8 {
        return Err(AccessError::bad("flow_limit", "Too many pending requests"));
    }
    let global: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM access_consents WHERE expires_at>? AND used_at IS NULL",
    )
    .bind(now())
    .fetch_one(&kit.0.db)
    .await?;
    if global >= 2048 {
        return Err(AccessError::unavailable());
    }
    let id = random();
    sqlx::query("INSERT INTO access_consents(id,user_id,session_binding,browser_hash,client_id,redirect_uri,state,scopes,resource,challenge,expires_at)VALUES(?,?,?,?,?,?,?,?,?,?,?)").bind(&id).bind(session.principal.user_id).bind(session_binding(&session)).bind(bh).bind(&b.client_id).bind(b.redirect_uri).bind(b.state).bind(serde_json::to_string(&scopes).map_err(|_|AccessError::unavailable())?).bind(b.resource).bind(b.code_challenge).bind(now()+300).execute(&kit.0.db).await?;
    let mut response = redirect(&format!("{}?request_id={id}", kit.0.host.paths().consent))?;
    response
        .headers_mut()
        .insert("set-cookie", browser_cookie(kit, &browser)?);
    Ok(response)
}
async fn pending(
    kit: &AccessKit,
    id: &str,
    s: &CookieSession,
    h: &HeaderMap,
) -> Result<sqlx::sqlite::SqliteRow, AccessError> {
    let b = browser(h).ok_or_else(AccessError::denied)?;
    let r=sqlx::query("SELECT c.*,a.client_name FROM access_consents c JOIN access_clients a ON a.client_id=c.client_id WHERE c.id=? AND c.expires_at>? AND c.used_at IS NULL AND c.user_id=?").bind(id).bind(now()).bind(s.principal.user_id).fetch_optional(&kit.0.db).await?.ok_or_else(AccessError::denied)?;
    if !equal(&r.get::<String, _>("session_binding"), &session_binding(s))
        || !equal(&r.get::<String, _>("browser_hash"), &hash(&b))
    {
        return Err(AccessError::denied());
    }
    Ok(r)
}
pub(crate) async fn details(
    kit: &AccessKit,
    id: &str,
    h: &HeaderMap,
) -> Result<Value, AccessError> {
    let s = credentials::cookie(kit, h, false).await?;
    let r = pending(kit, id, &s, h).await?;
    Ok(
        json!({"request_id":id,"client_name":r.get::<String,_>("client_name"),"scopes":credentials::decode(r.get("scopes"))?,"resource":r.get::<String,_>("resource"),"expires_at":time(r.get("expires_at")),"user":{"id":s.principal.user_id,"username":s.principal.username,"display_name":s.principal.display_name}}),
    )
}
pub(crate) async fn consent(
    kit: &AccessKit,
    id: &str,
    h: &HeaderMap,
    body: &Bytes,
) -> Result<Response, AccessError> {
    let s = credentials::cookie(kit, h, true).await?;
    let decision: Decision = if h
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/x-www-form-urlencoded"))
    {
        serde_urlencoded::from_bytes(body)
            .map_err(|_| AccessError::bad("invalid_request", "Invalid consent"))?
    } else {
        crate::http::parse_json(h, body)?
    };
    if !["approve", "deny"].contains(&decision.decision.as_str()) {
        return Err(AccessError::bad("invalid_request", "Invalid decision"));
    }
    let r = pending(kit, id, &s, h).await?;
    let mut target =
        Url::parse(&r.get::<String, _>("redirect_uri")).map_err(|_| AccessError::unavailable())?;
    let mut tx = kit.0.db.begin().await?;
    let changed = sqlx::query(
        "UPDATE access_consents SET used_at=? WHERE id=? AND used_at IS NULL AND expires_at>?",
    )
    .bind(now())
    .bind(id)
    .bind(now())
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if changed != 1 {
        return Err(AccessError::denied());
    }
    if decision.decision == "approve" {
        let code = random();
        sqlx::query("INSERT INTO access_codes(code_hash,user_id,client_id,redirect_uri,scopes,resource,challenge,expires_at)VALUES(?,?,?,?,?,?,?,?)").bind(hash(&code)).bind(s.principal.user_id).bind(r.get::<String,_>("client_id")).bind(r.get::<String,_>("redirect_uri")).bind(r.get::<String,_>("scopes")).bind(r.get::<String,_>("resource")).bind(r.get::<String,_>("challenge")).bind(now()+300).execute(&mut *tx).await?;
        target.query_pairs_mut().append_pair("code", &code);
    } else {
        target
            .query_pairs_mut()
            .append_pair("error", "access_denied");
    }
    target
        .query_pairs_mut()
        .append_pair("state", &r.get::<String, _>("state"));
    tx.commit().await?;
    redirect(target.as_str())
}
