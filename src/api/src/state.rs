use std::{sync::Arc, time::Duration};

use sqlx::SqlitePool;

use crate::{auth::LoginLimiter, config::Config, ratelimit::RateLimiter};

pub type AppState = Arc<Inner>;

pub struct Inner {
    pub access: std::sync::OnceLock<access_kit::AccessKit>,
    pub db: SqlitePool,
    pub config: Config,
    pub limiter: LoginLimiter,
    /// 同一個 IP 一分鐘能送幾次追蹤資料
    pub collects: RateLimiter,
}

impl Inner {
    pub fn new(db: SqlitePool, config: Config) -> Self {
        Self {
            access: std::sync::OnceLock::new(),
            db,
            collects: RateLimiter::new(config.rate_limit, Duration::from_secs(60)),
            config,
            limiter: LoginLimiter::default(),
        }
    }
}
