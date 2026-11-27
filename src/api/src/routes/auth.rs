use axum::{
    extract::State,
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    auth::{self, CurrentUser},
    error::{AppError, AppResult},
    state::AppState,
};

#[derive(sqlx::FromRow)]
struct LoginRow {
    id: i64,
    username: String,
    display_name: String,
    role: String,
    password_hash: String,
    disabled: i64,
    account_status: String,
    password_login_enabled: bool,
}

async fn user_count(state: &AppState) -> AppResult<i64> {
    Ok(sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await?)
}

/// 前端一開始問的：要不要先建立管理員、現在是誰登入。
pub async fn state(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let user = auth::optional_user(&state.db, &headers).await?;
    Ok(Json(json!({
        "setup_required": user_count(&state).await? == 0,
        "user": user,
    })))
}

async fn signed_in(state: &AppState, user: CurrentUser) -> AppResult<Response> {
    let token = auth::create_session(&state.db, user.id).await?;
    let cookie = auth::session_cookie(&token, state.config.cookie_secure);
    Ok(([(header::SET_COOKIE, cookie)], Json(user)).into_response())
}

#[derive(Deserialize)]
pub struct SetupBody {
    username: String,
    display_name: Option<String>,
    password: String,
}

/// 第一次使用：建立管理員。已經有帳號之後這個 API 就關掉。
pub async fn setup(
    State(state): State<AppState>,
    Json(body): Json<SetupBody>,
) -> AppResult<Response> {
    if user_count(&state).await? > 0 {
        return Err(AppError::conflict("已經設定過了，請直接登入"));
    }
    let username = body.username.trim().to_lowercase();
    let display_name = body
        .display_name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| username.clone());
    auth::validate_username(&username)?;
    auth::validate_password(&body.password)?;
    let display_name = auth::clean_display_name(&display_name)?;
    let hash = auth::hash_password(body.password).await?;
    // The empty-database check and insertion must be one SQLite statement: two
    // first-time requests can both pass the early check while hashing passwords.
    let created = sqlx::query(
        "INSERT INTO users (username, display_name, password_hash, role, created_at)
         SELECT ?, ?, ?, 'admin', ? WHERE NOT EXISTS (SELECT 1 FROM users)",
    )
    .bind(&username)
    .bind(&display_name)
    .bind(hash)
    .bind(crate::db::now())
    .execute(&state.db)
    .await?;
    if created.rows_affected() == 0 {
        return Err(AppError::conflict("已經設定過了，請直接登入"));
    }
    let id = created.last_insert_rowid();
    tracing::info!("建立了第一個管理員：{username}");
    let user = CurrentUser {
        id,
        username,
        display_name: display_name.trim().to_owned(),
        role: "admin".into(),
    };
    signed_in(&state, user).await
}

#[derive(Deserialize)]
pub struct LoginBody {
    username: String,
    password: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginBody>,
) -> AppResult<Response> {
    let username = body.username.trim().to_lowercase();
    state.limiter.check(&username)?;
    let row = sqlx::query_as::<_, LoginRow>(
        "SELECT id, username, display_name, role, password_hash, disabled, account_status, password_login_enabled
         FROM users WHERE username = ?",
    )
    .bind(&username)
    .fetch_optional(&state.db)
    .await?;
    let ok = match &row {
        Some(row) if row.password_login_enabled => {
            auth::verify_password(body.password, row.password_hash.clone()).await
        }
        Some(_) => {
            let _ = auth::hash_password(body.password).await;
            false
        }
        None => {
            // 帳號不存在時也算一次雜湊，回應時間才不會洩漏帳號存不存在
            let _ = auth::hash_password(body.password).await;
            false
        }
    };
    let Some(row) = row.filter(|_| ok) else {
        state.limiter.record_failure(&username);
        return Err(AppError::Unauthorized("帳號或密碼不對".into()));
    };
    if row.account_status != "active" {
        return Err(AppError::forbidden(if row.account_status == "pending" {
            "帳號正等待管理員審核"
        } else {
            "帳號申請已被拒絕"
        }));
    }
    if row.disabled != 0 {
        return Err(AppError::forbidden("這個帳號已經停用"));
    }
    state.limiter.clear(&username);
    let user = CurrentUser {
        id: row.id,
        username: row.username,
        display_name: row.display_name,
        role: row.role,
    };
    signed_in(&state, user).await
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    auth::delete_session(&state.db, &headers).await?;
    let cookie = auth::clear_cookie(state.config.cookie_secure);
    Ok(([(header::SET_COOKIE, cookie)], Json(json!({ "ok": true }))).into_response())
}

#[derive(Deserialize)]
pub struct MeBody {
    display_name: Option<String>,
}

pub async fn update_me(
    State(state): State<AppState>,
    mut user: CurrentUser,
    Json(body): Json<MeBody>,
) -> AppResult<Json<CurrentUser>> {
    if let Some(name) = &body.display_name {
        user.display_name = auth::clean_display_name(name)?;
    }
    sqlx::query("UPDATE users SET display_name = ? WHERE id = ?")
        .bind(&user.display_name)
        .bind(user.id)
        .execute(&state.db)
        .await?;
    Ok(Json(user))
}

#[derive(Deserialize)]
pub struct PasswordBody {
    current_password: String,
    new_password: String,
}

pub async fn change_password(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    Json(body): Json<PasswordBody>,
) -> AppResult<Json<Value>> {
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?")
        .bind(user.id)
        .fetch_one(&state.db)
        .await?;
    if !auth::verify_password(body.current_password, hash).await {
        return Err(AppError::bad("目前的密碼不對"));
    }
    auth::set_password(&state.db, user.id, &body.new_password).await?;
    // 其他裝置上的登入全部失效，這個瀏覽器留著
    let current = auth::read_cookie(&headers, auth::COOKIE_NAME);
    auth::delete_sessions_of(&state.db, user.id, current.as_deref()).await?;
    Ok(Json(json!({ "ok": true })))
}
