//! 網站上有哪些頁面被看過；訂標籤規則時拿來對照。

use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::QueryBuilder;

use crate::{
    auth::CurrentUser,
    db,
    error::AppResult,
    ingest::Event,
    labels,
    routes::{labels::LabelChip, sites::ensure_visible},
    state::AppState,
};

#[derive(Deserialize)]
pub struct PagesQuery {
    /// 看最近幾天，預設 30
    days: Option<i64>,
    /// 路徑或標題含有這段文字
    q: Option<String>,
}

#[derive(sqlx::FromRow)]
struct PageRow {
    path: String,
    title: String,
    url: String,
    pageviews: i64,
    visitors: i64,
    last_at: String,
}

#[derive(Serialize)]
pub struct Page {
    path: String,
    /// 最近一次看到的標題
    title: String,
    /// 最近一次看到的完整網址
    url: String,
    pageviews: i64,
    visitors: i64,
    last_at: String,
    /// 看這一頁會符合哪些標籤
    labels: Vec<LabelChip>,
}

pub async fn list(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(site_id): Path<i64>,
    Query(query): Query<PagesQuery>,
) -> AppResult<Json<Vec<Page>>> {
    ensure_visible(&state, &user, site_id).await?;
    let days = query.days.unwrap_or(30).clamp(1, 3650);
    let since = db::format_time(Utc::now() - Duration::days(days));

    let mut qb = QueryBuilder::new(
        "SELECT path, title, url, COUNT(*) AS pageviews, COUNT(DISTINCT person_id) AS visitors,
                MAX(created_at) AS last_at
         FROM events WHERE type = 'pageview' AND site_id = ",
    );
    qb.push_bind(site_id)
        .push(" AND created_at >= ")
        .push_bind(since);
    if let Some(q) = query.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        let q = q.to_ascii_lowercase();
        qb.push(" AND (instr(lower(path), ")
            .push_bind(q.clone())
            .push(") > 0 OR instr(lower(title), ")
            .push_bind(q)
            .push(") > 0)");
    }
    qb.push(" GROUP BY path ORDER BY pageviews DESC, path LIMIT 200");
    let rows: Vec<PageRow> = qb.build_query_as().fetch_all(&state.db).await?;

    let mut conn = state.db.acquire().await?;
    let rules = labels::site_rules(&mut conn, site_id).await?;
    let chips = LabelChip::of_site(&state, site_id).await?;
    let pages = rows
        .into_iter()
        .map(|row| {
            let event = Event {
                kind: "pageview",
                name: String::new(),
                url: row.url.clone(),
                path: row.path.clone(),
                title: row.title.clone(),
                referrer: String::new(),
                referrer_host: String::new(),
                props: String::new(),
            };
            let ids = labels::matching(&rules, &event);
            Page {
                path: row.path,
                title: row.title,
                url: row.url,
                pageviews: row.pageviews,
                visitors: row.visitors,
                last_at: row.last_at,
                labels: chips
                    .iter()
                    .filter(|chip| ids.contains(&chip.id))
                    .cloned()
                    .collect(),
            }
        })
        .collect();
    Ok(Json(pages))
}
