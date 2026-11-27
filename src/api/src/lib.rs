//! 輕量的顧客資料平台：網站貼一段追蹤程式碼，就能看到每個訪客看了什麼、
//! 認出誰是誰，再用標籤和分群把人整理成名單。

pub mod access;
pub mod access_registry;
pub mod auth;
pub mod config;
pub mod db;
pub mod demo;
pub mod error;
pub mod ingest;
pub mod labels;
pub mod logging;
pub mod policy;
pub mod ratelimit;
pub mod retention;
pub mod routes;
pub mod segments;
pub mod state;

use std::sync::Arc;

use anyhow::Result;
use axum::Router;

pub use config::Config;
pub use state::{AppState, Inner};

/// 開資料庫（必要時建立並套用 migration）。
pub async fn build_state(config: Config) -> Result<AppState> {
    let db = db::connect(&config.database_path).await?;
    let state = Arc::new(Inner::new(db, config));
    access::initialize(&state).await?;
    Ok(state)
}

pub fn app(state: AppState) -> Router {
    let core = state.access.get().expect("initialized access kit").router();
    routes::router(state).merge(core)
}
