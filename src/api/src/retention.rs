//! 定期刪掉太舊的事件（`CDP_RETENTION_DAYS`）。人的資料會留著，只是看不到那麼久以前做了什麼。

use std::time::Duration;

use anyhow::Result;
use sqlx::SqlitePool;

use crate::{db, labels, state::AppState};

/// 刪掉比 `days` 天更早的事件，回傳刪了幾筆。
pub async fn purge(pool: &SqlitePool, days: u32) -> Result<u64> {
    let cutoff = db::format_time(chrono::Utc::now() - chrono::Duration::days(i64::from(days)));
    let mut tx = pool.begin().await?;
    let deleted = sqlx::query("DELETE FROM events WHERE created_at < ?")
        .bind(cutoff)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if deleted > 0 {
        // 瀏覽頁數和標籤都是從事件算出來的，跟著重算
        sqlx::query(
            "UPDATE people SET pageviews =
                (SELECT COUNT(*) FROM events e WHERE e.person_id = people.id AND e.type = 'pageview')",
        )
        .execute(&mut *tx)
        .await?;
        let sites: Vec<i64> = sqlx::query_scalar("SELECT id FROM sites")
            .fetch_all(&mut *tx)
            .await?;
        for site_id in sites {
            labels::recompute_site(&mut tx, site_id).await?;
        }
    }
    tx.commit().await?;
    Ok(deleted)
}

/// 啟動時跑一次，之後每 6 小時一次。
pub fn spawn(state: AppState) {
    let days = state.config.retention_days;
    if days == 0 {
        return;
    }
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(Duration::from_secs(6 * 3600));
        loop {
            timer.tick().await;
            match purge(&state.db, days).await {
                Ok(0) => {}
                Ok(deleted) => tracing::info!("刪掉了 {deleted} 筆超過 {days} 天的事件"),
                Err(err) => tracing::error!("清舊事件失敗：{err:#}"),
            }
        }
    });
}
