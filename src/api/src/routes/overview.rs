//! 一個網站的總覽：有多少人、最近的流量、熱門頁面和來源。

use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Duration, NaiveTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    auth::CurrentUser, db, error::AppResult, routes::sites::ensure_visible, state::AppState,
};

const DAYS: i64 = 30;

#[derive(Deserialize)]
pub struct OverviewQuery {
    /// 瀏覽器的時區比 UTC 快幾分鐘（台灣是 480）；每天的數字照這個時區切
    tz_offset: Option<i64>,
}

#[derive(Serialize)]
struct Totals {
    people: i64,
    identified: i64,
    anonymous: i64,
}

#[derive(Serialize)]
struct Window {
    visitors: i64,
    pageviews: i64,
    /// 第一次來的人
    new_people: i64,
    /// 這段時間被認出來的人
    identified: i64,
}

#[derive(Serialize)]
struct Day {
    date: String,
    pageviews: i64,
    visitors: i64,
}

#[derive(Serialize, sqlx::FromRow)]
struct TopPage {
    path: String,
    title: String,
    pageviews: i64,
    visitors: i64,
}

#[derive(Serialize, sqlx::FromRow)]
struct TopReferrer {
    host: String,
    visitors: i64,
}

#[derive(Serialize, sqlx::FromRow)]
struct LabelCount {
    id: i64,
    name: String,
    color: String,
    people: i64,
}

#[derive(Serialize, sqlx::FromRow)]
struct RecentPerson {
    id: i64,
    email: String,
    name: String,
    identified_at: Option<String>,
}

#[derive(Serialize)]
pub struct Overview {
    totals: Totals,
    last_7_days: Window,
    previous_7_days: Window,
    /// 最近 30 天，從最舊到今天，沒有資料的日子也有一筆
    daily: Vec<Day>,
    top_pages: Vec<TopPage>,
    top_referrers: Vec<TopReferrer>,
    labels: Vec<LabelCount>,
    recent_identified: Vec<RecentPerson>,
}

async fn window(
    state: &AppState,
    site_id: i64,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> AppResult<Window> {
    let (from, to) = (db::format_time(from), db::format_time(to));
    let (visitors, pageviews): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(DISTINCT person_id), COALESCE(SUM(type = 'pageview'), 0) FROM events
         WHERE site_id = ? AND created_at >= ? AND created_at < ?",
    )
    .bind(site_id)
    .bind(&from)
    .bind(&to)
    .fetch_one(&state.db)
    .await?;
    let (new_people, identified): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(first_seen >= ?1 AND first_seen < ?2), 0),
                COALESCE(SUM(identified_at >= ?1 AND identified_at < ?2), 0)
         FROM people WHERE site_id = ?3",
    )
    .bind(&from)
    .bind(&to)
    .bind(site_id)
    .fetch_one(&state.db)
    .await?;
    Ok(Window {
        visitors,
        pageviews,
        new_people,
        identified,
    })
}

pub async fn overview(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(site_id): Path<i64>,
    Query(query): Query<OverviewQuery>,
) -> AppResult<Json<Overview>> {
    ensure_visible(&state, &user, site_id).await?;
    let now = Utc::now();
    let offset = Duration::minutes(query.tz_offset.unwrap_or(0).clamp(-840, 840));

    let (people, identified): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(email != ''), 0) FROM people WHERE site_id = ?",
    )
    .bind(site_id)
    .fetch_one(&state.db)
    .await?;

    // 30 天前那一天的半夜（照瀏覽器的時區）
    let today = (now + offset).date_naive();
    let first_day = today - Duration::days(DAYS - 1);
    let since = db::format_time(first_day.and_time(NaiveTime::MIN).and_utc() - offset);

    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT date(created_at, ?) AS day, COALESCE(SUM(type = 'pageview'), 0),
                COUNT(DISTINCT person_id)
         FROM events WHERE site_id = ? AND created_at >= ? GROUP BY day",
    )
    .bind(format!("{:+} minutes", offset.num_minutes()))
    .bind(site_id)
    .bind(&since)
    .fetch_all(&state.db)
    .await?;
    let by_day: HashMap<String, (i64, i64)> = rows
        .into_iter()
        .map(|(day, pageviews, visitors)| (day, (pageviews, visitors)))
        .collect();
    let daily = (0..DAYS)
        .map(|i| {
            let date = (first_day + Duration::days(i)).to_string();
            let (pageviews, visitors) = by_day.get(&date).copied().unwrap_or_default();
            Day {
                date,
                pageviews,
                visitors,
            }
        })
        .collect();

    // 同一個路徑的標題取最新的那一筆（SQLite：只有一個 MAX 時，其他欄位跟著那一列）
    let top_pages: Vec<TopPage> = sqlx::query_as(
        "SELECT path, title, COUNT(*) AS pageviews, COUNT(DISTINCT person_id) AS visitors,
                MAX(created_at) AS last_at
         FROM events WHERE site_id = ? AND type = 'pageview' AND created_at >= ?
         GROUP BY path ORDER BY pageviews DESC, path LIMIT 8",
    )
    .bind(site_id)
    .bind(&since)
    .fetch_all(&state.db)
    .await?;

    let top_referrers: Vec<TopReferrer> = sqlx::query_as(
        "SELECT referrer_host AS host, COUNT(DISTINCT person_id) AS visitors
         FROM events WHERE site_id = ? AND referrer_host != '' AND created_at >= ?
         GROUP BY referrer_host ORDER BY visitors DESC, host LIMIT 8",
    )
    .bind(site_id)
    .bind(&since)
    .fetch_all(&state.db)
    .await?;

    let labels: Vec<LabelCount> = sqlx::query_as(
        "SELECT l.id, l.name, l.color,
            (SELECT COUNT(*) FROM person_labels pl
              WHERE pl.label_id = l.id AND pl.hits >= l.min_hits) AS people
         FROM labels l WHERE l.site_id = ? ORDER BY people DESC, l.name",
    )
    .bind(site_id)
    .fetch_all(&state.db)
    .await?;

    let recent_identified: Vec<RecentPerson> = sqlx::query_as(
        "SELECT id, email, name, identified_at FROM people
         WHERE site_id = ? AND email != '' ORDER BY identified_at DESC, id DESC LIMIT 6",
    )
    .bind(site_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(Overview {
        totals: Totals {
            people,
            identified,
            anonymous: people - identified,
        },
        last_7_days: window(
            &state,
            site_id,
            now - Duration::days(7),
            now + Duration::seconds(1),
        )
        .await?,
        previous_7_days: window(
            &state,
            site_id,
            now - Duration::days(14),
            now - Duration::days(7),
        )
        .await?,
        daily,
        top_pages,
        top_referrers,
        labels,
        recent_identified,
    }))
}
