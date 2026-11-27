use crate::{
    crypto::{now, random, time},
    AccessError, AccessKit,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use url::Url;
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Registration {
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    #[serde(default = "none")]
    pub token_endpoint_auth_method: String,
    #[serde(default = "grants")]
    pub grant_types: Vec<String>,
    #[serde(default = "responses")]
    pub response_types: Vec<String>,
}
fn none() -> String {
    "none".into()
}
fn grants() -> Vec<String> {
    vec!["authorization_code".into(), "refresh_token".into()]
}
fn responses() -> Vec<String> {
    vec!["code".into()]
}
pub(crate) fn valid_redirect(value: &str) -> bool {
    let Ok(u) = Url::parse(value) else {
        return false;
    };
    let local = matches!(
        u.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    );
    u.host_str().is_some()
        && u.username().is_empty()
        && u.password().is_none()
        && u.fragment().is_none()
        && !value.contains('*')
        && value.len() <= 2048
        && (u.scheme() == "https" || u.scheme() == "http" && local)
}
pub(crate) async fn register(kit: &AccessKit, b: Registration) -> Result<Value, AccessError> {
    if b.client_name.trim().is_empty()
        || b.client_name.chars().count() > 80
        || b.redirect_uris.is_empty()
        || b.redirect_uris.len() > 8
        || b.redirect_uris.iter().any(|u| !valid_redirect(u))
        || b.token_endpoint_auth_method != "none"
        || b.response_types != responses()
        || b.grant_types.iter().any(|g| !grants().contains(g))
        || !b.grant_types.contains(&"authorization_code".into())
    {
        return Err(AccessError::bad(
            "invalid_client_metadata",
            "Unsupported client metadata",
        ));
    }
    let id = random();
    let at = now();
    let mut tx = kit.0.db.begin().await?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM access_clients")
        .fetch_one(&mut *tx)
        .await?;
    if count >= 256 {
        return Err(AccessError::bad("client_limit", "Client limit reached"));
    }
    sqlx::query(
        "INSERT INTO access_clients(client_id,client_name,redirect_uris,created_at)VALUES(?,?,?,?)",
    )
    .bind(&id)
    .bind(b.client_name.trim())
    .bind(serde_json::to_string(&b.redirect_uris).map_err(|_| AccessError::unavailable())?)
    .bind(at)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(
        json!({"client_id":id,"client_name":b.client_name.trim(),"redirect_uris":b.redirect_uris,"token_endpoint_auth_method":"none","grant_types":grants(),"response_types":responses(),"client_id_issued_at":at}),
    )
}
pub(crate) async fn list(kit: &AccessKit) -> Result<Value, AccessError> {
    let rows=sqlx::query("SELECT client_id,client_name,redirect_uris,created_at FROM access_clients ORDER BY created_at DESC").fetch_all(&kit.0.db).await?;
    let items=rows.into_iter().map(|r|Ok(json!({"client_id":r.get::<String,_>("client_id"),"client_name":r.get::<String,_>("client_name"),"redirect_uris":crate::credentials::decode(r.get("redirect_uris"))?,"created_at":time(r.get("created_at"))}))).collect::<Result<Vec<_>,AccessError>>()?;
    Ok(json!({"items":items}))
}
