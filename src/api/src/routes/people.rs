//! 訪客名單、單一訪客的資料和他做過的事。

use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    http::header,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{QueryBuilder, Sqlite};

use crate::{
    auth::{Admin, CurrentUser},
    error::{AppError, AppResult},
    ingest::describe_user_agent,
    routes::{labels::LabelChip, sites::ensure_visible},
    segments::{self, Rules},
    state::AppState,
};

/// 匯出最多幾個人。
const EXPORT_LIMIT: i64 = 50_000;

#[derive(Deserialize, Default)]
pub struct ListQuery {
    /// `member`：知道 email 的；`anonymous`：還不知道是誰的
    kind: Option<String>,
    /// email、名稱或帳號含有這段文字
    q: Option<String>,
    label_id: Option<i64>,
    segment_id: Option<i64>,
    /// `last_seen`（預設）、`first_seen`、`pageviews`
    sort: Option<String>,
    page: Option<i64>,
    per_page: Option<i64>,
}

/// 名單的篩選條件。
pub struct Filter {
    site_id: i64,
    kind: String,
    q: Option<String>,
    label_id: Option<i64>,
    /// 分群的對象和條件
    segment: Option<(String, Rules)>,
    now: DateTime<Utc>,
}

impl Filter {
    /// 只用一組分群條件篩（預覽分群時用）。
    pub fn for_rules(site_id: i64, kind: &str, rules: Rules) -> Self {
        Self {
            site_id,
            kind: "all".into(),
            q: None,
            label_id: None,
            segment: Some((kind.to_owned(), rules)),
            now: Utc::now(),
        }
    }

    async fn from_query(state: &AppState, site_id: i64, query: &ListQuery) -> AppResult<Self> {
        let kind = query.kind.as_deref().unwrap_or("all");
        if !segments::KINDS.contains(&kind) {
            return Err(AppError::bad("kind 只能是 all、member 或 anonymous"));
        }
        let segment = match query.segment_id {
            Some(id) => {
                let row: Option<(String, String)> =
                    sqlx::query_as("SELECT kind, rules FROM segments WHERE site_id = ? AND id = ?")
                        .bind(site_id)
                        .bind(id)
                        .fetch_optional(&state.db)
                        .await?;
                let (kind, rules) = row.ok_or_else(|| AppError::not_found("找不到這個分群"))?;
                Some((kind, serde_json::from_str(&rules)?))
            }
            None => None,
        };
        Ok(Self {
            site_id,
            kind: kind.to_owned(),
            q: query
                .q
                .as_deref()
                .map(|q| q.trim().to_ascii_lowercase())
                .filter(|q| !q.is_empty()),
            label_id: query.label_id,
            segment,
            now: Utc::now(),
        })
    }

    /// ` FROM people p WHERE ...`
    fn push(&self, qb: &mut QueryBuilder<'_, Sqlite>) {
        qb.push(" FROM people p WHERE p.site_id = ")
            .push_bind(self.site_id);
        match self.kind.as_str() {
            "member" => {
                qb.push(" AND p.email != ''");
            }
            "anonymous" => {
                qb.push(" AND p.email = ''");
            }
            _ => {}
        }
        if let Some(q) = &self.q {
            qb.push(" AND (instr(lower(p.email), ")
                .push_bind(q.clone())
                .push(") > 0 OR instr(lower(p.name), ")
                .push_bind(q.clone())
                .push(") > 0 OR instr(lower(p.account), ")
                .push_bind(q.clone())
                .push(") > 0)");
        }
        if let Some(label_id) = self.label_id {
            qb.push(
                " AND EXISTS (SELECT 1 FROM person_labels pl JOIN labels l ON l.id = pl.label_id
                   WHERE pl.person_id = p.id AND pl.hits >= l.min_hits AND pl.label_id = ",
            )
            .push_bind(label_id)
            .push(")");
        }
        if let Some((kind, rules)) = &self.segment {
            rules.push_where(qb, kind, self.now);
        }
    }

    pub async fn count(&self, state: &AppState) -> AppResult<i64> {
        let mut qb = QueryBuilder::new("SELECT COUNT(*)");
        self.push(&mut qb);
        Ok(qb.build_query_scalar().fetch_one(&state.db).await?)
    }

    /// 一頁的人。`order` 是寫死的 SQL，不是使用者給的字串。
    pub async fn fetch(
        &self,
        state: &AppState,
        order: &'static str,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<Person>> {
        let mut qb = QueryBuilder::new(
            "SELECT p.id, p.email, p.name, p.account, p.pageviews, p.first_seen, p.last_seen,
                    p.identified_at",
        );
        self.push(&mut qb);
        qb.push(" ORDER BY ")
            .push(order)
            .push(" LIMIT ")
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind(offset);
        let rows: Vec<PersonRow> = qb.build_query_as().fetch_all(&state.db).await?;
        let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
        let mut labels = labels_of(state, &ids).await?;
        Ok(rows
            .into_iter()
            .map(|row| Person {
                labels: labels.remove(&row.id).unwrap_or_default(),
                id: row.id,
                email: row.email,
                name: row.name,
                account: row.account,
                pageviews: row.pageviews,
                first_seen: row.first_seen,
                last_seen: row.last_seen,
                identified_at: row.identified_at,
            })
            .collect())
    }
}

pub const DEFAULT_ORDER: &str = "p.last_seen DESC, p.id DESC";

#[derive(sqlx::FromRow)]
struct PersonRow {
    id: i64,
    email: String,
    name: String,
    account: String,
    pageviews: i64,
    first_seen: String,
    last_seen: String,
    identified_at: Option<String>,
}

#[derive(Serialize)]
pub struct Person {
    id: i64,
    /// 空字串是還不知道是誰的匿名訪客
    email: String,
    name: String,
    account: String,
    pageviews: i64,
    first_seen: String,
    last_seen: String,
    identified_at: Option<String>,
    labels: Vec<LabelChip>,
}

/// 這些人各有哪些標籤（次數到了門檻的才算）。
async fn labels_of(state: &AppState, ids: &[i64]) -> AppResult<HashMap<i64, Vec<LabelChip>>> {
    let mut map: HashMap<i64, Vec<LabelChip>> = HashMap::new();
    // SQLite 一個查詢的參數有上限，分批查
    for chunk in ids.chunks(500) {
        let mut qb = QueryBuilder::new(
            "SELECT pl.person_id, l.id, l.name, l.color
             FROM person_labels pl JOIN labels l ON l.id = pl.label_id
             WHERE pl.hits >= l.min_hits AND pl.person_id IN (",
        );
        let mut list = qb.separated(", ");
        for id in chunk {
            list.push_bind(*id);
        }
        qb.push(") ORDER BY l.name");
        let rows: Vec<(i64, i64, String, String)> =
            qb.build_query_as().fetch_all(&state.db).await?;
        for (person_id, id, name, color) in rows {
            map.entry(person_id)
                .or_default()
                .push(LabelChip { id, name, color });
        }
    }
    Ok(map)
}

fn order_of(sort: Option<&str>) -> AppResult<&'static str> {
    match sort.unwrap_or("last_seen") {
        "last_seen" => Ok(DEFAULT_ORDER),
        "first_seen" => Ok("p.first_seen DESC, p.id DESC"),
        "pageviews" => Ok("p.pageviews DESC, p.id DESC"),
        _ => Err(AppError::bad(
            "sort 只能是 last_seen、first_seen 或 pageviews",
        )),
    }
}

pub async fn list(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(site_id): Path<i64>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    ensure_visible(&state, &user, site_id).await?;
    let filter = Filter::from_query(&state, site_id, &query).await?;
    let order = order_of(query.sort.as_deref())?;
    let per_page = query.per_page.unwrap_or(25).clamp(1, 100);
    let page = query.page.unwrap_or(1).max(1);
    let total = filter.count(&state).await?;
    let items = filter
        .fetch(&state, order, per_page, (page - 1) * per_page)
        .await?;
    Ok(Json(json!({
        "items": items,
        "total": total,
        "page": page,
        "per_page": per_page,
    })))
}

/// 試算表會把 `=`、`+`、`-`、`@` 開頭的儲存格當公式；前面加一個單引號擋掉。
fn csv_safe(text: &str) -> String {
    if text.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{text}")
    } else {
        text.to_owned()
    }
}

pub async fn export(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(site_id): Path<i64>,
    Query(query): Query<ListQuery>,
) -> AppResult<Response> {
    ensure_visible(&state, &user, site_id).await?;
    let filter = Filter::from_query(&state, site_id, &query).await?;
    let order = order_of(query.sort.as_deref())?;
    let people = filter.fetch(&state, order, EXPORT_LIMIT, 0).await?;

    let mut writer = csv::Writer::from_writer(Vec::new());
    writer.write_record([
        "email",
        "名稱",
        "帳號",
        "瀏覽頁數",
        "第一次來",
        "最後一次來",
        "認出來的時間",
        "標籤",
    ])?;
    for person in people {
        let labels: Vec<&str> = person.labels.iter().map(|l| l.name.as_str()).collect();
        writer.write_record([
            csv_safe(&person.email),
            csv_safe(&person.name),
            csv_safe(&person.account),
            person.pageviews.to_string(),
            person.first_seen,
            person.last_seen,
            person.identified_at.unwrap_or_default(),
            csv_safe(&labels.join("、")),
        ])?;
    }
    let rows = writer.into_inner().map_err(|e| anyhow::anyhow!("{e}"))?;
    // 開頭加 BOM，Excel 才會用 UTF-8 開
    let mut body = b"\xEF\xBB\xBF".to_vec();
    body.extend(rows);
    let filename = format!("people-{}.csv", Utc::now().format("%Y%m%d"));
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        body,
    )
        .into_response())
}

#[derive(sqlx::FromRow)]
struct ProfileRow {
    id: i64,
    email: String,
    name: String,
    account: String,
    traits: String,
    pageviews: i64,
    first_seen: String,
    last_seen: String,
    identified_at: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
struct PersonLabel {
    id: i64,
    name: String,
    color: String,
    /// 符合規則的事件有幾次
    hits: i64,
    min_hits: i64,
    last_at: String,
}

#[derive(Serialize)]
struct Device {
    id: i64,
    browser: &'static str,
    os: &'static str,
    first_seen: String,
    last_seen: String,
}

#[derive(Serialize, sqlx::FromRow)]
struct Referrer {
    host: String,
    /// 從這個來源進來了幾次
    visits: i64,
}

#[derive(Serialize, sqlx::FromRow)]
struct TopPage {
    path: String,
    title: String,
    pageviews: i64,
}

async fn ensure_person(state: &AppState, site_id: i64, id: i64) -> AppResult<()> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM people WHERE site_id = ? AND id = ?")
            .bind(site_id)
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
    found
        .map(|_| ())
        .ok_or_else(|| AppError::not_found("找不到這個人"))
}

pub async fn detail(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((site_id, id)): Path<(i64, i64)>,
) -> AppResult<Json<Value>> {
    ensure_visible(&state, &user, site_id).await?;
    let row: Option<ProfileRow> = sqlx::query_as(
        "SELECT id, email, name, account, traits, pageviews, first_seen, last_seen, identified_at
         FROM people WHERE site_id = ? AND id = ?",
    )
    .bind(site_id)
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let row = row.ok_or_else(|| AppError::not_found("找不到這個人"))?;

    let (events, sessions): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COUNT(DISTINCT NULLIF(session_id, '')) FROM events WHERE person_id = ?",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    // 還沒到門檻的也列出來，畫面上看得到「還差幾次」
    let labels: Vec<PersonLabel> = sqlx::query_as(
        "SELECT l.id, l.name, l.color, pl.hits, l.min_hits, pl.last_at
         FROM person_labels pl JOIN labels l ON l.id = pl.label_id
         WHERE pl.person_id = ? ORDER BY pl.hits >= l.min_hits DESC, l.name",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    let devices: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT id, user_agent, first_seen, last_seen FROM devices
         WHERE person_id = ? ORDER BY last_seen DESC",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    let devices: Vec<Device> = devices
        .into_iter()
        .map(|(id, user_agent, first_seen, last_seen)| {
            let (browser, os) = describe_user_agent(&user_agent);
            Device {
                id,
                browser,
                os,
                first_seen,
                last_seen,
            }
        })
        .collect();
    let referrers: Vec<Referrer> = sqlx::query_as(
        "SELECT referrer_host AS host, COUNT(*) AS visits FROM events
         WHERE person_id = ? AND referrer_host != ''
         GROUP BY referrer_host ORDER BY visits DESC, host LIMIT 10",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    let top_pages: Vec<TopPage> = sqlx::query_as(
        "SELECT path, title, COUNT(*) AS pageviews, MAX(created_at) AS last_at FROM events
         WHERE person_id = ? AND type = 'pageview'
         GROUP BY path ORDER BY pageviews DESC, path LIMIT 8",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(json!({
        "id": row.id,
        "email": row.email,
        "name": row.name,
        "account": row.account,
        "traits": serde_json::from_str::<Value>(&row.traits).unwrap_or_else(|_| json!({})),
        "pageviews": row.pageviews,
        "first_seen": row.first_seen,
        "last_seen": row.last_seen,
        "identified_at": row.identified_at,
        "events": events,
        "sessions": sessions,
        "labels": labels,
        "devices": devices,
        "referrers": referrers,
        "top_pages": top_pages,
    })))
}

#[derive(Deserialize)]
pub struct EventsQuery {
    /// 只要編號比這個小的（上一頁最後一筆的編號）
    before: Option<i64>,
    limit: Option<i64>,
    #[serde(rename = "type")]
    kind: Option<String>,
}

#[derive(sqlx::FromRow)]
struct EventRow {
    id: i64,
    #[sqlx(rename = "type")]
    kind: String,
    name: String,
    url: String,
    path: String,
    title: String,
    referrer_host: String,
    props: String,
    session_id: String,
    created_at: String,
}

/// 一個人做過的事，從最新的開始。
pub async fn events(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((site_id, id)): Path<(i64, i64)>,
    Query(query): Query<EventsQuery>,
) -> AppResult<Json<Value>> {
    ensure_visible(&state, &user, site_id).await?;
    ensure_person(&state, site_id, id).await?;
    let limit = query.limit.unwrap_or(50).clamp(1, 200);

    let mut qb = QueryBuilder::new(
        "SELECT id, type, name, url, path, title, referrer_host, props, session_id, created_at
         FROM events WHERE person_id = ",
    );
    qb.push_bind(id);
    if let Some(before) = query.before {
        qb.push(" AND id < ").push_bind(before);
    }
    if let Some(kind) = query.kind.as_deref().filter(|k| !k.is_empty()) {
        if !["pageview", "click", "scroll", "custom", "identify"].contains(&kind) {
            return Err(AppError::bad("沒有這種事件"));
        }
        qb.push(" AND type = ").push_bind(kind.to_owned());
    }
    // 多拿一筆，用來判斷後面還有沒有
    qb.push(" ORDER BY id DESC LIMIT ").push_bind(limit + 1);
    let mut rows: Vec<EventRow> = qb.build_query_as().fetch_all(&state.db).await?;
    let more = rows.len() as i64 > limit;
    rows.truncate(limit as usize);
    let next_before = if more {
        rows.last().map(|r| r.id)
    } else {
        None
    };

    let items: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            json!({
                "id": row.id,
                "type": row.kind,
                "name": row.name,
                "url": row.url,
                "path": row.path,
                "title": row.title,
                "referrer_host": row.referrer_host,
                "props": serde_json::from_str::<Value>(&row.props).unwrap_or_else(|_| json!({})),
                "session_id": row.session_id,
                "created_at": row.created_at,
            })
        })
        .collect();
    Ok(Json(json!({ "items": items, "next_before": next_before })))
}

/// 刪掉一個人和他所有的事件（對方要求刪除個人資料時用）。
/// 他的瀏覽器下次再來，會被當成新的匿名訪客。
pub async fn remove(
    State(state): State<AppState>,
    _admin: Admin,
    Path((site_id, id)): Path<(i64, i64)>,
) -> AppResult<Json<Value>> {
    let done = sqlx::query("DELETE FROM people WHERE site_id = ? AND id = ?")
        .bind(site_id)
        .bind(id)
        .execute(&state.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::not_found("找不到這個人"));
    }
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formulas_are_defused() {
        assert_eq!(csv_safe("=1+1"), "'=1+1");
        assert_eq!(csv_safe("@amy"), "'@amy");
        assert_eq!(csv_safe("amy@example.com"), "amy@example.com");
        assert_eq!(csv_safe(""), "");
    }
}
