use crate::{
    crypto::{hash, now, random, time},
    types::AuthContext,
    AccessError, AccessKit, CookieSession, Role, Transport,
};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Create {
    pub name: String,
    pub scopes: Vec<String>,
    #[serde(default = "default_days")]
    pub expires_in_days: i64,
    #[serde(default = "both")]
    pub audiences: Vec<Transport>,
}
fn default_days() -> i64 {
    30
}
fn both() -> Vec<Transport> {
    vec![Transport::Api, Transport::Mcp]
}
pub(crate) async fn cookie(
    kit: &AccessKit,
    h: &HeaderMap,
    write: bool,
) -> Result<CookieSession, AccessError> {
    if h.contains_key("authorization") {
        return Err(AccessError::unauthorized());
    }
    if write && !same_origin(kit, h) {
        return Err(AccessError::denied());
    }
    let session = kit
        .0
        .host
        .cookie_principal(h)
        .await?
        .ok_or_else(AccessError::unauthorized)?;
    if kit
        .0
        .host
        .active_principal(session.principal.user_id)
        .await?
        .is_none()
    {
        return Err(AccessError::unauthorized());
    }
    kit.rate(
        format!(
            "management-{}:{}",
            if write { "write" } else { "read" },
            session.principal.user_id
        ),
        if write { 60 } else { 240 },
    )?;
    Ok(session)
}
pub(crate) fn same_origin(kit: &AccessKit, h: &HeaderMap) -> bool {
    if h.get("sec-fetch-site").and_then(|v| v.to_str().ok()) == Some("cross-site") {
        return false;
    }
    h.get("origin")
        .map(|v| {
            v.to_str().ok() == Some(kit.0.config.origin.origin().ascii_serialization().as_str())
        })
        .unwrap_or(true)
}
pub(crate) async fn create(kit: &AccessKit, user: i64, body: Create) -> Result<Value, AccessError> {
    kit.validate_scopes(&body.scopes)?;
    if body.name.trim().is_empty()
        || body.name.chars().count() > 80
        || !(1..=90).contains(&body.expires_in_days)
        || body.audiences.is_empty()
        || body.audiences.len() > 2
    {
        return Err(AccessError::bad(
            "invalid_credential",
            "Invalid name, expiry or audiences",
        ));
    }
    let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM access_credentials WHERE user_id=? AND revoked_at IS NULL AND expires_at>?").bind(user).bind(now()).fetch_one(&kit.0.db).await?;
    if count >= 32 {
        return Err(AccessError::bad(
            "credential_limit",
            "Credential limit reached",
        ));
    }
    let id = random();
    let token = format!("ak_{}", random());
    let prefix = format!("ak_{}", &id[..8]);
    let created = now();
    let scopes = serde_json::to_string(&body.scopes).map_err(|_| AccessError::unavailable())?;
    let audiences =
        serde_json::to_string(&body.audiences).map_err(|_| AccessError::unavailable())?;
    sqlx::query("INSERT INTO access_credentials(id,user_id,name,prefix,token_hash,scopes,audiences,created_at,expires_at)VALUES(?,?,?,?,?,?,?,?,?)").bind(&id).bind(user).bind(body.name.trim()).bind(prefix).bind(hash(&token)).bind(scopes).bind(audiences).bind(created).bind(created+body.expires_in_days*86400).execute(&kit.0.db).await?;
    let items = list(kit, user).await?;
    let item = items
        .into_iter()
        .find(|v| v["id"] == id)
        .ok_or_else(AccessError::unavailable)?;
    Ok(json!({"credential":item,"token":token}))
}
pub(crate) async fn list(kit: &AccessKit, user: i64) -> Result<Vec<Value>, AccessError> {
    let rows=sqlx::query("SELECT id,name,prefix,scopes,audiences,created_at,expires_at,last_used_at,revoked_at FROM access_credentials WHERE user_id=? ORDER BY created_at DESC,id DESC").bind(user).fetch_all(&kit.0.db).await?;
    rows.into_iter().map(|r|Ok(json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"prefix":r.get::<String,_>("prefix"),"scopes":decode(r.get("scopes"))?,"audiences":decode(r.get("audiences"))?,"created_at":time(r.get("created_at")),"expires_at":time(r.get("expires_at")),"last_used_at":optional_time(r.get("last_used_at")),"revoked_at":optional_time(r.get("revoked_at"))}))).collect()
}
pub(crate) fn decode(value: String) -> Result<Value, AccessError> {
    serde_json::from_str(&value).map_err(|_| AccessError::unavailable())
}
pub(crate) fn optional_time(value: Option<i64>) -> Option<String> {
    value.map(time)
}
pub(crate) async fn authenticate(
    kit: &AccessKit,
    h: &HeaderMap,
    transport: Transport,
) -> Result<AuthContext, AccessError> {
    let token = h
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|v| !v.is_empty() && v.len() < 256)
        .ok_or_else(AccessError::unauthorized)?;
    let token_hash = hash(token);
    let at = now();
    let auth = if token.starts_with("ak_") {
        let r=sqlx::query("SELECT id,user_id,scopes,audiences FROM access_credentials WHERE token_hash=? AND expires_at>? AND revoked_at IS NULL").bind(&token_hash).bind(at).fetch_optional(&kit.0.db).await?.ok_or_else(AccessError::unauthorized)?;
        let audiences: Vec<Transport> = serde_json::from_str(&r.get::<String, _>("audiences"))
            .map_err(|_| AccessError::unavailable())?;
        if !audiences.contains(&transport) {
            return Err(AccessError::unauthorized());
        }
        let user = r.get("user_id");
        let principal = kit
            .0
            .host
            .active_principal(user)
            .await?
            .ok_or_else(AccessError::unauthorized)?;
        let id: String = r.get("id");
        sqlx::query("UPDATE access_credentials SET last_used_at=? WHERE id=?")
            .bind(at)
            .bind(&id)
            .execute(&kit.0.db)
            .await?;
        AuthContext {
            principal,
            scopes: serde_json::from_str(&r.get::<String, _>("scopes"))
                .map_err(|_| AccessError::unavailable())?,
            credential_id: Some(id),
            client_id: None,
            grant_id: None,
            request_id: random(),
            audit_ticket: None,
        }
    } else {
        let resource = kit.0.config.url(match transport {
            Transport::Api => "/api/v1",
            Transport::Mcp => "/mcp",
        });
        let r=sqlx::query("SELECT g.id,g.user_id,g.client_id,t.scopes FROM access_tokens t JOIN access_grants g ON g.id=t.grant_id WHERE t.token_hash=? AND t.kind='access' AND t.expires_at>? AND t.revoked_at IS NULL AND g.expires_at>? AND g.revoked_at IS NULL AND g.resource=?").bind(&token_hash).bind(at).bind(at).bind(resource).fetch_optional(&kit.0.db).await?.ok_or_else(AccessError::unauthorized)?;
        let principal = kit
            .0
            .host
            .active_principal(r.get("user_id"))
            .await?
            .ok_or_else(AccessError::unauthorized)?;
        let grant_id: String = r.get("id");
        sqlx::query("UPDATE access_grants SET last_used_at=? WHERE id=?")
            .bind(at)
            .bind(&grant_id)
            .execute(&kit.0.db)
            .await?;
        AuthContext {
            principal,
            scopes: serde_json::from_str(&r.get::<String, _>("scopes"))
                .map_err(|_| AccessError::unavailable())?,
            credential_id: None,
            client_id: Some(r.get("client_id")),
            grant_id: Some(grant_id),
            request_id: random(),
            audit_ticket: None,
        }
    };
    kit.rate(format!("bearer-user:{}", auth.principal.user_id), 120)?;
    kit.rate(format!("bearer:{}", hash(token)), 120)?;
    Ok(auth)
}
pub(crate) async fn revoke(
    kit: &AccessKit,
    user: &crate::Principal,
    id: &str,
) -> Result<(), AccessError> {
    let owner: Option<i64> =
        sqlx::query_scalar("SELECT user_id FROM access_credentials WHERE id=?")
            .bind(id)
            .fetch_optional(&kit.0.db)
            .await?;
    if owner != Some(user.user_id) && user.role != Role::Admin {
        return Err(AccessError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "Credential not found",
        ));
    }
    sqlx::query("UPDATE access_credentials SET revoked_at=? WHERE id=?")
        .bind(now())
        .bind(id)
        .execute(&kit.0.db)
        .await?;
    Ok(())
}
