use std::{path::Path, time::Duration};

use anyhow::{Context, Result};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};

pub async fn connect(path: &Path) -> Result<SqlitePool> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("建立資料夾失敗：{}", parent.display()))?;
        }
    }
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(10));
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
        .with_context(|| format!("開不了資料庫：{}", path.display()))?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("資料庫 migration 失敗")?;
    Ok(pool)
}

/// 現在的 UTC 時間（RFC 3339，到秒）。
pub fn now() -> String {
    format_time(chrono::Utc::now())
}

pub fn format_time(t: chrono::DateTime<chrono::Utc>) -> String {
    t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
