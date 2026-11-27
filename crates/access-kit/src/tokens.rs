use crate::{
    crypto::{equal, hash, now, pkce, random, time},
    AccessError, AccessKit,
};
use serde_json::{json, Value};
use sqlx::{Row, Sqlite, Transaction};
use std::collections::HashMap;
fn invalid() -> AccessError {
    AccessError::bad("invalid_grant", "Invalid or expired grant")
}
fn required<'a>(b: &'a HashMap<String, String>, key: &str) -> Result<&'a str, AccessError> {
    b.get(key)
        .map(String::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 2048)
        .ok_or_else(invalid)
}
pub(crate) async fn exchange(
    kit: &AccessKit,
    b: HashMap<String, String>,
) -> Result<Value, AccessError> {
    if b.keys().any(|k| {
        ![
            "grant_type",
            "client_id",
            "code",
            "redirect_uri",
            "resource",
            "code_verifier",
            "refresh_token",
            "scope",
        ]
        .contains(&k.as_str())
    }) {
        return Err(AccessError::bad(
            "invalid_request",
            "Unsupported token parameter",
        ));
    }
    required(&b, "client_id")?;
    match required(&b, "grant_type")? {
        "authorization_code" => code(kit, &b).await,
        "refresh_token" => refresh(kit, &b).await,
        _ => Err(AccessError::bad(
            "unsupported_grant_type",
            "Unsupported grant type",
        )),
    }
}
async fn issue(
    tx: &mut Transaction<'_, Sqlite>,
    grant: &str,
    scopes: &str,
    expiry: i64,
) -> Result<Value, AccessError> {
    let access = format!("at_{}", random());
    let refresh = format!("rt_{}", random());
    let at = now();
    let access_expiry = (at + 3600).min(expiry);
    for (kind, raw, expires) in [
        ("access", &access, access_expiry),
        ("refresh", &refresh, expiry),
    ] {
        sqlx::query("INSERT INTO access_tokens(id,token_hash,grant_id,kind,scopes,created_at,expires_at)VALUES(?,?,?,?,?,?,?)").bind(random()).bind(hash(raw)).bind(grant).bind(kind).bind(scopes).bind(at).bind(expires).execute(&mut **tx).await?;
    }
    let scope: Vec<String> =
        serde_json::from_str(scopes).map_err(|_| AccessError::unavailable())?;
    Ok(
        json!({"access_token":access,"token_type":"Bearer","expires_in":access_expiry-at,"refresh_token":refresh,"scope":scope.join(" ")}),
    )
}
async fn code(kit: &AccessKit, b: &HashMap<String, String>) -> Result<Value, AccessError> {
    let client = required(b, "client_id")?;
    let code = required(b, "code")?;
    let redirect = required(b, "redirect_uri")?;
    let resource = required(b, "resource")?;
    let verifier = required(b, "code_verifier")?;
    if !(43..=128).contains(&verifier.len())
        || !verifier
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-._~".contains(&c))
    {
        return Err(invalid());
    }
    let r=sqlx::query("SELECT * FROM access_codes WHERE code_hash=? AND client_id=? AND redirect_uri=? AND resource=? AND expires_at>? AND used_at IS NULL").bind(hash(code)).bind(client).bind(redirect).bind(resource).bind(now()).fetch_optional(&kit.0.db).await?.ok_or_else(invalid)?;
    if !equal(&r.get::<String, _>("challenge"), &pkce(verifier)) {
        return Err(invalid());
    }
    let user: i64 = r.get("user_id");
    if kit.0.host.active_principal(user).await?.is_none() {
        return Err(invalid());
    }
    kit.rate(format!("token-user:{user}"), 30)?;
    let scopes: String = r.get("scopes");
    let id = random();
    let expiry = now() + 30 * 86400;
    let mut tx = kit.0.db.begin().await?;
    let changed = sqlx::query(
        "UPDATE access_codes SET used_at=? WHERE code_hash=? AND used_at IS NULL AND expires_at>?",
    )
    .bind(now())
    .bind(hash(code))
    .bind(now())
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if changed != 1 {
        return Err(invalid());
    }
    sqlx::query("INSERT INTO access_grants(id,user_id,client_id,scopes,resource,created_at,expires_at)VALUES(?,?,?,?,?,?,?)").bind(&id).bind(user).bind(client).bind(&scopes).bind(resource).bind(now()).bind(expiry).execute(&mut *tx).await?;
    let response = issue(&mut tx, &id, &scopes, expiry).await?;
    tx.commit().await?;
    Ok(response)
}
async fn refresh(kit: &AccessKit, b: &HashMap<String, String>) -> Result<Value, AccessError> {
    let client = required(b, "client_id")?;
    let raw = required(b, "refresh_token")?;
    let r=sqlx::query("SELECT t.id token_id,t.used_at,t.scopes,g.id,g.user_id,g.resource,g.expires_at FROM access_tokens t JOIN access_grants g ON t.grant_id=g.id WHERE t.token_hash=? AND t.kind='refresh' AND t.expires_at>? AND t.revoked_at IS NULL AND g.client_id=? AND g.revoked_at IS NULL AND g.expires_at>?").bind(hash(raw)).bind(now()).bind(client).bind(now()).fetch_optional(&kit.0.db).await?.ok_or_else(invalid)?;
    if b.get("resource")
        .is_some_and(|v| *v != r.get::<String, _>("resource"))
    {
        return Err(invalid());
    }
    let previous: Vec<String> = serde_json::from_str(&r.get::<String, _>("scopes"))
        .map_err(|_| AccessError::unavailable())?;
    let scopes = match b.get("scope") {
        None => previous.clone(),
        Some(s) => s.split_whitespace().map(str::to_owned).collect::<Vec<_>>(),
    };
    if scopes.is_empty() || scopes.iter().any(|s| !previous.contains(s)) {
        return Err(AccessError::bad(
            "invalid_scope",
            "Refresh cannot increase scopes",
        ));
    }
    if kit
        .0
        .host
        .active_principal(r.get("user_id"))
        .await?
        .is_none()
    {
        return Err(invalid());
    }
    let id: String = r.get("id");
    let mut tx = kit.0.db.begin().await?;
    let changed=sqlx::query("UPDATE access_tokens SET used_at=? WHERE id=? AND used_at IS NULL AND revoked_at IS NULL AND expires_at>?").bind(now()).bind(r.get::<String,_>("token_id")).bind(now()).execute(&mut *tx).await?.rows_affected();
    if changed != 1 {
        revoke_transaction(&mut tx, &id).await?;
        tx.commit().await?;
        return Err(invalid());
    }
    let grant_active: bool = sqlx::query_scalar(
        "SELECT revoked_at IS NULL AND expires_at>? FROM access_grants WHERE id=?",
    )
    .bind(now())
    .bind(&id)
    .fetch_one(&mut *tx)
    .await?;
    if !grant_active {
        return Err(invalid());
    }
    kit.rate(format!("token-user:{}", r.get::<i64, _>("user_id")), 30)?;
    let response = issue(
        &mut tx,
        &id,
        &serde_json::to_string(&scopes).map_err(|_| AccessError::unavailable())?,
        r.get("expires_at"),
    )
    .await?;
    tx.commit().await?;
    Ok(response)
}
pub(crate) async fn revoke_transaction(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> Result<(), AccessError> {
    sqlx::query("UPDATE access_grants SET revoked_at=COALESCE(revoked_at,?) WHERE id=?")
        .bind(now())
        .bind(id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("UPDATE access_tokens SET revoked_at=COALESCE(revoked_at,?) WHERE grant_id=?")
        .bind(now())
        .bind(id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
pub(crate) async fn revoke_token(
    kit: &AccessKit,
    b: &HashMap<String, String>,
) -> Result<(), AccessError> {
    let token = required(b, "token")?;
    let client = required(b, "client_id")?;
    let id:Option<String>=sqlx::query_scalar("SELECT g.id FROM access_tokens t JOIN access_grants g ON g.id=t.grant_id WHERE t.token_hash=? AND g.client_id=?").bind(hash(token)).bind(client).fetch_optional(&kit.0.db).await?;
    if let Some(id) = id {
        let mut tx = kit.0.db.begin().await?;
        revoke_transaction(&mut tx, &id).await?;
        tx.commit().await?;
    }
    Ok(())
}
pub(crate) async fn grants(kit: &AccessKit, user: i64) -> Result<Vec<Value>, AccessError> {
    let rows=sqlx::query("SELECT g.*,c.client_name FROM access_grants g JOIN access_clients c ON c.client_id=g.client_id WHERE user_id=? ORDER BY created_at DESC,id DESC").bind(user).fetch_all(&kit.0.db).await?;
    rows.into_iter().map(|r|Ok(json!({"id":r.get::<String,_>("id"),"client_id":r.get::<String,_>("client_id"),"client_name":r.get::<String,_>("client_name"),"scopes":crate::credentials::decode(r.get("scopes"))?,"resource":r.get::<String,_>("resource"),"created_at":time(r.get("created_at")),"expires_at":time(r.get("expires_at")),"last_used_at":crate::credentials::optional_time(r.get("last_used_at")),"revoked_at":crate::credentials::optional_time(r.get("revoked_at"))}))).collect()
}
pub(crate) async fn revoke_grant(
    kit: &AccessKit,
    user: &crate::Principal,
    id: &str,
) -> Result<(), AccessError> {
    let owner: Option<i64> = sqlx::query_scalar("SELECT user_id FROM access_grants WHERE id=?")
        .bind(id)
        .fetch_optional(&kit.0.db)
        .await?;
    if owner != Some(user.user_id) && user.role != crate::Role::Admin {
        return Err(AccessError::denied());
    }
    let mut tx = kit.0.db.begin().await?;
    revoke_transaction(&mut tx, id).await?;
    tx.commit().await?;
    Ok(())
}
