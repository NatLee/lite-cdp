//! Installation diagnostics never write analytics rows.
use super::sites::ensure_visible;
use crate::{
    auth::{self, Admin, CurrentUser},
    db,
    error::{AppError, AppResult},
    policy::CollectionPolicy,
    state::AppState,
};
use axum::{
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
    Json,
};
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;

pub async fn config(State(state): State<AppState>, Path(key): Path<String>) -> AppResult<Response> {
    let row: Option<(i64, String, bool)> =
        sqlx::query_as("SELECT id,collection_policy,active FROM sites WHERE key=?")
            .bind(key)
            .fetch_optional(&state.db)
            .await?;
    let Some((id, policy, active)) = row else {
        return Err(AppError::not_found("沒有這個追蹤金鑰"));
    };
    Ok(([(header::ACCESS_CONTROL_ALLOW_ORIGIN,"*"),(header::CACHE_CONTROL,"no-store")],Json(json!({"version":2,"site_id":id,"namespace":format!("site_{id}"),"active":active,"policy":CollectionPolicy::parse(&policy)}))).into_response())
}
pub async fn create_test(
    State(state): State<AppState>,
    Admin(admin): Admin,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    ensure_visible(&state, &admin, id).await?;
    sqlx::query("DELETE FROM installation_tests WHERE expires_at < ?")
        .bind(db::now())
        .execute(&state.db)
        .await?;
    let token = auth::random_hex(24);
    let expires = db::format_time(chrono::Utc::now() + chrono::Duration::minutes(15));
    sqlx::query("INSERT INTO installation_tests(token,site_id,expires_at) VALUES(?,?,?)")
        .bind(&token)
        .bind(id)
        .bind(&expires)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({"token":token,"expires_at":expires})))
}
pub async fn receipt(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((id, token)): Path<(i64, String)>,
) -> AppResult<Json<Value>> {
    ensure_visible(&state, &user, id).await?;
    let row: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT expires_at,receipt,received_at FROM installation_tests WHERE site_id=? AND token=?",
    )
    .bind(id)
    .bind(token)
    .fetch_optional(&state.db)
    .await?;
    let Some((expires, receipt, received_at)) = row else {
        return Err(AppError::not_found("找不到這次安裝測試"));
    };
    Ok(Json(
        json!({"expires_at":expires,"expired":expires < db::now(),"received_at":received_at,"receipt":receipt.and_then(|s|serde_json::from_str::<Value>(&s).ok())}),
    ))
}
pub async fn identity_secret(
    State(state): State<AppState>,
    Admin(admin): Admin,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    ensure_visible(&state, &admin, id).await?;
    sqlx::query("UPDATE sites SET identity_secret=? WHERE id=? AND identity_secret=''")
        .bind(auth::random_hex(32))
        .bind(id)
        .execute(&state.db)
        .await?;
    let secret: String = sqlx::query_scalar("SELECT identity_secret FROM sites WHERE id=?")
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    Ok(([(header::CACHE_CONTROL,"no-store")],Json(json!({"secret":secret,"algorithm":"HMAC-SHA256","message":"site_key + newline + user_id + newline + expires_at","encoding":"hex; UTF-8 secret"}))).into_response())
}
pub fn verify_identity(
    secret: &str,
    key: &str,
    user_id: &str,
    proof: &crate::ingest::IdentityProof,
) -> bool {
    let now = chrono::Utc::now().timestamp();
    if secret.len() != 64
        || proof.expires_at < now
        || proof.expires_at > now + 3600
        || proof.signature.len() != 64
        || !proof.signature.is_ascii()
        || user_id.is_empty()
        || user_id.contains(char::is_control)
    {
        return false;
    }
    let signature: Option<Vec<u8>> = (0..64)
        .step_by(2)
        .map(|i| u8::from_str_radix(&proof.signature[i..i + 2], 16).ok())
        .collect();
    let Some(signature) = signature else {
        return false;
    };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(format!("{key}\n{user_id}\n{}", proof.expires_at).as_bytes());
    mac.verify_slice(&signature).is_ok()
}
pub async fn store_receipt(
    state: &AppState,
    site_id: i64,
    token: &str,
    receipt: Value,
) -> AppResult<bool> {
    let result=sqlx::query("UPDATE installation_tests SET receipt=?,received_at=? WHERE site_id=? AND token=? AND expires_at>?")
        .bind(receipt.to_string()).bind(db::now()).bind(site_id).bind(token).bind(db::now()).execute(&state.db).await?;
    Ok(result.rows_affected() == 1)
}
