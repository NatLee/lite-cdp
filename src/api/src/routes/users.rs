//! 帳號的管理（管理員）。

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{self, Admin},
    error::{AppError, AppResult},
    state::AppState,
};

#[derive(Serialize, sqlx::FromRow)]
pub struct UserRow {
    id: i64,
    username: String,
    display_name: String,
    role: String,
    disabled: bool,
    account_status: String,
    created_at: String,
    /// 看得到幾個網站
    site_count: i64,
}

const USER_SELECT: &str =
    "SELECT u.id, u.username, u.display_name, u.role, u.disabled, u.account_status, u.created_at,
        (SELECT COUNT(*) FROM site_members m WHERE m.user_id = u.id) AS site_count
     FROM users u";

pub async fn list(State(state): State<AppState>, _admin: Admin) -> AppResult<Json<Vec<UserRow>>> {
    let users = sqlx::query_as::<_, UserRow>(&format!("{USER_SELECT} ORDER BY u.id"))
        .fetch_all(&state.db)
        .await?;
    Ok(Json(users))
}

async fn one(state: &AppState, id: i64) -> AppResult<UserRow> {
    sqlx::query_as::<_, UserRow>(&format!("{USER_SELECT} WHERE u.id = ?"))
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("找不到這個帳號"))
}

#[derive(Deserialize)]
pub struct CreateUser {
    username: String,
    display_name: Option<String>,
    password: String,
    role: Option<String>,
}

pub async fn create(
    State(state): State<AppState>,
    _admin: Admin,
    Json(body): Json<CreateUser>,
) -> AppResult<Json<UserRow>> {
    let username = body.username.trim().to_lowercase();
    let display_name = body
        .display_name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| username.clone());
    let role = body.role.as_deref().unwrap_or("member");
    let id = auth::create_user(&state.db, &username, &display_name, &body.password, role).await?;
    Ok(Json(one(&state, id).await?))
}

#[derive(Deserialize)]
pub struct UpdateUser {
    display_name: Option<String>,
    role: Option<String>,
    disabled: Option<bool>,
    password: Option<String>,
}

pub async fn update(
    State(state): State<AppState>,
    Admin(admin): Admin,
    Path(id): Path<i64>,
    Json(body): Json<UpdateUser>,
) -> AppResult<Json<UserRow>> {
    let current = one(&state, id).await?;
    // 先全部檢查過再寫，才不會改到一半
    let display_name = match &body.display_name {
        Some(name) => auth::clean_display_name(name)?,
        None => current.display_name.clone(),
    };
    let role = body.role.as_deref().unwrap_or(&current.role);
    let disabled = body.disabled.unwrap_or(current.disabled);
    if !matches!(role, "admin" | "member") {
        return Err(AppError::bad("角色只能是 admin 或 member"));
    }
    let access_changed = role != current.role || disabled != current.disabled;
    if access_changed && id == admin.id {
        return Err(AppError::bad("不能改自己的角色或停用自己"));
    }
    if let Some(password) = &body.password {
        auth::validate_password(password)?;
    }

    sqlx::query("UPDATE users SET display_name = ?, role = ?, disabled = ? WHERE id = ?")
        .bind(display_name)
        .bind(role)
        .bind(disabled)
        .bind(id)
        .execute(&state.db)
        .await?;
    if disabled && !current.disabled {
        auth::delete_sessions_of(&state.db, id, None).await?;
    }
    if let Some(password) = &body.password {
        auth::set_password(&state.db, id, password).await?;
        if id != admin.id {
            auth::delete_sessions_of(&state.db, id, None).await?;
        }
    }
    Ok(Json(one(&state, id).await?))
}
