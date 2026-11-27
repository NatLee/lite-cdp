//! 網站的設定。管理員看得到全部，其他人只看得到被加進去的。

use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    auth::{self, Admin, CurrentUser},
    config::origin_of,
    db,
    error::{AppError, AppResult},
    policy::CollectionPolicy,
    state::AppState,
};

#[derive(sqlx::FromRow)]
struct SiteRow {
    id: i64,
    name: String,
    url: String,
    key: String,
    allowed_origins: String,
    collection_policy: String,
    active: bool,
    created_at: String,
    people: i64,
    identified: i64,
    last_event_at: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct Member {
    id: i64,
    username: String,
    display_name: String,
}

#[derive(Serialize)]
pub struct Site {
    id: i64,
    name: String,
    url: String,
    /// 追蹤程式碼用的金鑰；會出現在網頁原始碼裡，不是秘密
    key: String,
    allowed_origins: Vec<String>,
    collection_policy: CollectionPolicy,
    active: bool,
    created_at: String,
    /// 總共幾個人
    people: i64,
    /// 其中知道 email 的
    identified: i64,
    /// 最後一次收到事件的時間；`null` 表示還沒收到過
    last_event_at: Option<String>,
    members: Vec<Member>,
}

const SITE_SELECT: &str =
    "SELECT s.id, s.name, s.url, s.key, s.allowed_origins, s.collection_policy, s.active, s.created_at,
        (SELECT COUNT(*) FROM people p WHERE p.site_id = s.id) AS people,
        (SELECT COUNT(*) FROM people p WHERE p.site_id = s.id AND (p.email != '' OR p.user_id != '')) AS identified,
        (SELECT MAX(e.created_at) FROM events e WHERE e.site_id = s.id) AS last_event_at
     FROM sites s";

pub fn parse_list(json: &str) -> Vec<String> {
    serde_json::from_str(json).unwrap_or_default()
}

async fn members_by_site(state: &AppState) -> AppResult<HashMap<i64, Vec<Member>>> {
    let rows: Vec<(i64, i64, String, String)> = sqlx::query_as(
        "SELECT m.site_id, u.id, u.username, u.display_name
         FROM site_members m JOIN users u ON u.id = m.user_id
         WHERE u.disabled = 0 ORDER BY u.display_name",
    )
    .fetch_all(&state.db)
    .await?;
    let mut map: HashMap<i64, Vec<Member>> = HashMap::new();
    for (site_id, id, username, display_name) in rows {
        map.entry(site_id).or_default().push(Member {
            id,
            username,
            display_name,
        });
    }
    Ok(map)
}

fn present(row: SiteRow, members: Vec<Member>) -> Site {
    Site {
        id: row.id,
        name: row.name,
        url: row.url,
        key: row.key,
        allowed_origins: parse_list(&row.allowed_origins),
        collection_policy: CollectionPolicy::parse(&row.collection_policy),
        active: row.active,
        created_at: row.created_at,
        people: row.people,
        identified: row.identified,
        last_event_at: row.last_event_at,
        members,
    }
}

/// 這個人看不看得到這個網站的資料；網站不存在也是回「找不到」。
pub async fn ensure_visible(state: &AppState, user: &CurrentUser, site_id: i64) -> AppResult<()> {
    let visible: Option<i64> = if user.is_admin() {
        sqlx::query_scalar("SELECT 1 FROM sites WHERE id = ?")
            .bind(site_id)
            .fetch_optional(&state.db)
            .await?
    } else {
        sqlx::query_scalar("SELECT 1 FROM site_members WHERE site_id = ? AND user_id = ?")
            .bind(site_id)
            .bind(user.id)
            .fetch_optional(&state.db)
            .await?
    };
    match visible {
        Some(_) => Ok(()),
        // 不說「沒有權限」，免得別人用編號去猜有哪些網站
        None => Err(AppError::not_found("找不到這個網站")),
    }
}

pub async fn list(State(state): State<AppState>, user: CurrentUser) -> AppResult<Json<Vec<Site>>> {
    let rows: Vec<SiteRow> = if user.is_admin() {
        sqlx::query_as(&format!("{SITE_SELECT} ORDER BY s.name"))
            .fetch_all(&state.db)
            .await?
    } else {
        sqlx::query_as(&format!(
            "{SITE_SELECT} WHERE s.id IN (SELECT site_id FROM site_members WHERE user_id = ?)
             ORDER BY s.name"
        ))
        .bind(user.id)
        .fetch_all(&state.db)
        .await?
    };
    let mut members = members_by_site(&state).await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| {
                let m = members.remove(&row.id).unwrap_or_default();
                present(row, m)
            })
            .collect(),
    ))
}

async fn one(state: &AppState, user: &CurrentUser, id: i64) -> AppResult<Site> {
    ensure_visible(state, user, id).await?;
    let row: SiteRow = sqlx::query_as(&format!("{SITE_SELECT} WHERE s.id = ?"))
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    let members = members_by_site(state)
        .await?
        .remove(&id)
        .unwrap_or_default();
    Ok(present(row, members))
}

pub async fn detail(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> AppResult<Json<Site>> {
    Ok(Json(one(&state, &user, id).await?))
}

#[derive(Deserialize, Default)]
pub struct SiteBody {
    name: Option<String>,
    url: Option<String>,
    allowed_origins: Option<Vec<String>>,
    collection_policy: Option<CollectionPolicy>,
    active: Option<bool>,
    member_ids: Option<Vec<i64>>,
}

fn clean_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(AppError::bad("網站名稱要 1 到 60 個字元"));
    }
    Ok(name.to_owned())
}

/// 可以留白；有填的話要是 http(s) 網址。
fn clean_url(url: &str) -> AppResult<String> {
    let url = url.trim();
    if url.is_empty() {
        return Ok(String::new());
    }
    if url.len() > 500 || origin_of(url).is_none() {
        return Err(AppError::bad("網址要是 http:// 或 https:// 開頭"));
    }
    Ok(url.to_owned())
}

/// `https://www.example.com/contact` 也收，只留 origin。
fn clean_origins(origins: &[String]) -> AppResult<String> {
    let mut out: Vec<String> = Vec::new();
    for origin in origins.iter().filter(|o| !o.trim().is_empty()) {
        let origin = origin_of(origin).ok_or_else(|| {
            AppError::bad(format!(
                "「{}」不是合法的來源，要像 https://www.example.com",
                origin.trim()
            ))
        })?;
        if !out.contains(&origin) {
            out.push(origin);
        }
    }
    if out.len() > 20 {
        return Err(AppError::bad("允許的來源最多 20 個"));
    }
    Ok(serde_json::to_string(&out)?)
}

async fn name_taken(state: &AppState, name: &str, except: i64) -> AppResult<bool> {
    let taken: Option<i64> = sqlx::query_scalar("SELECT id FROM sites WHERE name = ? AND id != ?")
        .bind(name)
        .bind(except)
        .fetch_optional(&state.db)
        .await?;
    Ok(taken.is_some())
}

async fn set_members(state: &AppState, site_id: i64, member_ids: &[i64]) -> AppResult<()> {
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM site_members WHERE site_id = ?")
        .bind(site_id)
        .execute(&mut *tx)
        .await?;
    let mut ids = member_ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    for user_id in ids {
        let added = sqlx::query(
            "INSERT INTO site_members (site_id, user_id) SELECT ?, id FROM users WHERE id = ?",
        )
        .bind(site_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        if added.rows_affected() == 0 {
            return Err(AppError::bad(format!("找不到帳號 {user_id}")));
        }
    }
    tx.commit().await?;
    Ok(())
}

pub async fn create(
    State(state): State<AppState>,
    Admin(admin): Admin,
    Json(body): Json<SiteBody>,
) -> AppResult<Json<Site>> {
    let name = clean_name(body.name.as_deref().unwrap_or_default())?;
    if name_taken(&state, &name, 0).await? {
        return Err(AppError::conflict("已經有同名的網站"));
    }
    let url = clean_url(body.url.as_deref().unwrap_or_default())?;
    let origins = clean_origins(body.allowed_origins.as_deref().unwrap_or_default())?;
    let policy = body.collection_policy.clone().unwrap_or_default();
    policy.validate().map_err(AppError::bad)?;
    let id = sqlx::query(
        "INSERT INTO sites (name, url, key, allowed_origins, active, created_at, collection_policy, identity_secret)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&name)
    .bind(url)
    .bind(auth::random_hex(12))
    .bind(origins)
    .bind(body.active.unwrap_or(true))
    .bind(db::now())
    .bind(serde_json::to_string(&policy)?)
    .bind(auth::random_hex(32))
    .execute(&state.db)
    .await?
    .last_insert_rowid();
    set_members(&state, id, &body.member_ids.unwrap_or_default()).await?;
    Ok(Json(one(&state, &admin, id).await?))
}

pub async fn update(
    State(state): State<AppState>,
    Admin(admin): Admin,
    Path(id): Path<i64>,
    Json(body): Json<SiteBody>,
) -> AppResult<Json<Site>> {
    let current = one(&state, &admin, id).await?;
    let name = match &body.name {
        Some(name) => clean_name(name)?,
        None => current.name.clone(),
    };
    if name_taken(&state, &name, id).await? {
        return Err(AppError::conflict("已經有同名的網站"));
    }
    let url = match &body.url {
        Some(url) => clean_url(url)?,
        None => current.url.clone(),
    };
    let origins = clean_origins(
        body.allowed_origins
            .as_deref()
            .unwrap_or(&current.allowed_origins),
    )?;
    let policy = body.collection_policy.unwrap_or(current.collection_policy);
    policy.validate().map_err(AppError::bad)?;
    sqlx::query("UPDATE sites SET name = ?, url = ?, allowed_origins = ?, active = ?, collection_policy = ? WHERE id = ?")
        .bind(name)
        .bind(url)
        .bind(origins)
        .bind(body.active.unwrap_or(current.active))
        .bind(serde_json::to_string(&policy)?)
        .bind(id)
        .execute(&state.db)
        .await?;
    if let Some(member_ids) = &body.member_ids {
        set_members(&state, id, member_ids).await?;
    }
    Ok(Json(one(&state, &admin, id).await?))
}

/// 刪掉網站，它的訪客、事件、標籤和分群也一起刪。
pub async fn remove(
    State(state): State<AppState>,
    _admin: Admin,
    Path(id): Path<i64>,
) -> AppResult<Json<Value>> {
    let done = sqlx::query("DELETE FROM sites WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::not_found("找不到這個網站"));
    }
    Ok(Json(json!({ "ok": true })))
}

/// 換一個新的追蹤金鑰；舊的馬上失效，網站上的追蹤程式碼要跟著換。
pub async fn rotate_key(
    State(state): State<AppState>,
    Admin(admin): Admin,
    Path(id): Path<i64>,
) -> AppResult<Json<Site>> {
    let done = sqlx::query("UPDATE sites SET key = ? WHERE id = ?")
        .bind(auth::random_hex(12))
        .bind(id)
        .execute(&state.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::not_found("找不到這個網站"));
    }
    Ok(Json(one(&state, &admin, id).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn origins_are_normalised() {
        let cleaned = clean_origins(&strings(&[
            "https://WWW.example.com/contact",
            "https://www.example.com",
            " ",
            "http://localhost:5173",
        ]))
        .unwrap();
        assert_eq!(
            cleaned,
            r#"["https://www.example.com","http://localhost:5173"]"#
        );
        assert!(clean_origins(&strings(&["www.example.com"])).is_err());
    }

    #[test]
    fn urls() {
        assert_eq!(clean_url("  ").unwrap(), "");
        assert!(clean_url("https://example.com/shop").is_ok());
        assert!(clean_url("javascript:alert(1)").is_err());
        assert!(clean_url("/shop").is_err());
    }
}
