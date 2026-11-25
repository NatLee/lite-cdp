//! 分群：存起來的名單條件。看得到這個網站的人都可以改。

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    auth::CurrentUser,
    db,
    error::{AppError, AppResult},
    routes::{
        people::{Filter, DEFAULT_ORDER},
        sites::ensure_visible,
    },
    segments::{Rules, KINDS},
    state::AppState,
};

#[derive(sqlx::FromRow)]
struct SegmentRow {
    id: i64,
    name: String,
    description: String,
    kind: String,
    rules: String,
    created_by_name: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
pub struct Segment {
    id: i64,
    name: String,
    description: String,
    /// `all`：所有人；`member`：知道 email 的；`anonymous`：還不知道是誰的
    kind: String,
    rules: Rules,
    /// 現在符合的人數
    count: i64,
    created_by_name: Option<String>,
    created_at: String,
    updated_at: String,
}

const SEGMENT_SELECT: &str =
    "SELECT s.id, s.name, s.description, s.kind, s.rules, u.display_name AS created_by_name,
            s.created_at, s.updated_at
     FROM segments s LEFT JOIN users u ON u.id = s.created_by";

async fn present(state: &AppState, site_id: i64, row: SegmentRow) -> AppResult<Segment> {
    let rules: Rules = serde_json::from_str(&row.rules)?;
    let count = Filter::for_rules(site_id, &row.kind, rules.clone())
        .count(state)
        .await?;
    Ok(Segment {
        id: row.id,
        name: row.name,
        description: row.description,
        kind: row.kind,
        rules,
        count,
        created_by_name: row.created_by_name,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

pub async fn list(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(site_id): Path<i64>,
) -> AppResult<Json<Vec<Segment>>> {
    ensure_visible(&state, &user, site_id).await?;
    let rows: Vec<SegmentRow> = sqlx::query_as(&format!(
        "{SEGMENT_SELECT} WHERE s.site_id = ? ORDER BY s.name"
    ))
    .bind(site_id)
    .fetch_all(&state.db)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(present(&state, site_id, row).await?);
    }
    Ok(Json(out))
}

async fn one(state: &AppState, site_id: i64, id: i64) -> AppResult<Segment> {
    let row: Option<SegmentRow> = sqlx::query_as(&format!(
        "{SEGMENT_SELECT} WHERE s.site_id = ? AND s.id = ?"
    ))
    .bind(site_id)
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let row = row.ok_or_else(|| AppError::not_found("找不到這個分群"))?;
    present(state, site_id, row).await
}

pub async fn detail(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((site_id, id)): Path<(i64, i64)>,
) -> AppResult<Json<Segment>> {
    ensure_visible(&state, &user, site_id).await?;
    Ok(Json(one(&state, site_id, id).await?))
}

#[derive(Deserialize, Default)]
pub struct SegmentBody {
    name: Option<String>,
    description: Option<String>,
    kind: Option<String>,
    rules: Option<Rules>,
}

fn clean_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(AppError::bad("分群名稱要 1 到 60 個字元"));
    }
    Ok(name.to_owned())
}

fn clean_description(text: &str) -> AppResult<String> {
    let text = text.trim();
    if text.chars().count() > 200 {
        return Err(AppError::bad("說明最多 200 個字元"));
    }
    Ok(text.to_owned())
}

fn clean_kind(kind: &str) -> AppResult<String> {
    if !KINDS.contains(&kind) {
        return Err(AppError::bad("對象只能是 all、member 或 anonymous"));
    }
    Ok(kind.to_owned())
}

/// 整理條件，並確認用到的標籤都是這個網站的。
async fn clean_rules(state: &AppState, site_id: i64, kind: &str, rules: Rules) -> AppResult<Rules> {
    let rules = rules.clean().map_err(AppError::bad)?;
    if kind == "all" && rules.is_empty() {
        return Err(AppError::bad("至少要有一個條件"));
    }
    for label_id in rules.label_ids() {
        let found: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM labels WHERE site_id = ? AND id = ?")
                .bind(site_id)
                .bind(label_id)
                .fetch_optional(&state.db)
                .await?;
        if found.is_none() {
            return Err(AppError::bad(format!("找不到標籤 {label_id}")));
        }
    }
    Ok(rules)
}

async fn name_taken(state: &AppState, site_id: i64, name: &str, except: i64) -> AppResult<bool> {
    let taken: Option<i64> =
        sqlx::query_scalar("SELECT id FROM segments WHERE site_id = ? AND name = ? AND id != ?")
            .bind(site_id)
            .bind(name)
            .bind(except)
            .fetch_optional(&state.db)
            .await?;
    Ok(taken.is_some())
}

/// 還沒存之前先看看這組條件會選到多少人。
pub async fn preview(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(site_id): Path<i64>,
    Json(body): Json<SegmentBody>,
) -> AppResult<Json<Value>> {
    ensure_visible(&state, &user, site_id).await?;
    let kind = clean_kind(body.kind.as_deref().unwrap_or("all"))?;
    let rules = clean_rules(&state, site_id, &kind, body.rules.unwrap_or_default()).await?;
    let filter = Filter::for_rules(site_id, &kind, rules);
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM people WHERE site_id = ?")
        .bind(site_id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(json!({
        "count": filter.count(&state).await?,
        "total": total,
        "sample": filter.fetch(&state, DEFAULT_ORDER, 5, 0).await?,
    })))
}

pub async fn create(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(site_id): Path<i64>,
    Json(body): Json<SegmentBody>,
) -> AppResult<Json<Segment>> {
    ensure_visible(&state, &user, site_id).await?;
    let name = clean_name(body.name.as_deref().unwrap_or_default())?;
    if name_taken(&state, site_id, &name, 0).await? {
        return Err(AppError::conflict("已經有同名的分群"));
    }
    let description = clean_description(body.description.as_deref().unwrap_or_default())?;
    let kind = clean_kind(body.kind.as_deref().unwrap_or("all"))?;
    let rules = clean_rules(&state, site_id, &kind, body.rules.unwrap_or_default()).await?;
    let now = db::now();
    let id = sqlx::query(
        "INSERT INTO segments (site_id, name, description, kind, rules, created_by, created_at,
            updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(site_id)
    .bind(name)
    .bind(description)
    .bind(kind)
    .bind(serde_json::to_string(&rules)?)
    .bind(user.id)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await?
    .last_insert_rowid();
    Ok(Json(one(&state, site_id, id).await?))
}

pub async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((site_id, id)): Path<(i64, i64)>,
    Json(body): Json<SegmentBody>,
) -> AppResult<Json<Segment>> {
    ensure_visible(&state, &user, site_id).await?;
    let current = one(&state, site_id, id).await?;
    let name = match &body.name {
        Some(name) => clean_name(name)?,
        None => current.name,
    };
    if name_taken(&state, site_id, &name, id).await? {
        return Err(AppError::conflict("已經有同名的分群"));
    }
    let description = match &body.description {
        Some(text) => clean_description(text)?,
        None => current.description,
    };
    let kind = match &body.kind {
        Some(kind) => clean_kind(kind)?,
        None => current.kind,
    };
    let rules = clean_rules(&state, site_id, &kind, body.rules.unwrap_or(current.rules)).await?;
    sqlx::query(
        "UPDATE segments SET name = ?, description = ?, kind = ?, rules = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(name)
    .bind(description)
    .bind(kind)
    .bind(serde_json::to_string(&rules)?)
    .bind(db::now())
    .bind(id)
    .execute(&state.db)
    .await?;
    Ok(Json(one(&state, site_id, id).await?))
}

pub async fn remove(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((site_id, id)): Path<(i64, i64)>,
) -> AppResult<Json<Value>> {
    ensure_visible(&state, &user, site_id).await?;
    let done = sqlx::query("DELETE FROM segments WHERE site_id = ? AND id = ?")
        .bind(site_id)
        .bind(id)
        .execute(&state.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::not_found("找不到這個分群"));
    }
    Ok(Json(json!({ "ok": true })))
}
