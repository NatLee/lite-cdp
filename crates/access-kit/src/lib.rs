//! Shared access protocol and audit core; hosts retain all domain authorization.
mod config;
mod crypto;
mod error;
mod types;
pub use config::AccessConfig;
pub use error::AccessError;
pub use types::{
    CookieSession, DomainOperation, Host, HostFuture, HostPaths, Invocation, Principal, Role,
    Transport, TrustedPrincipal,
};
pub const SCHEMA_SQL: &str = concat!(
    include_str!("../migrations/0001_access.sql"),
    include_str!("../migrations/0002_client_registration.sql")
);
pub const SCHEMA_VERSION: u32 = 2;
mod audit;
mod authorization;
mod clients;
mod credentials;
mod dispatch;
mod http;
mod mcp;
mod providers;
mod tokens;
use axum::Router;
use serde_json::Value;
use sqlx::SqlitePool;
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
};
#[derive(Clone)]
pub struct AccessKit(pub(crate) Arc<Inner>);
pub(crate) struct Registered {
    pub operation: DomainOperation,
    pub validator: jsonschema::Validator,
}
pub(crate) struct Inner {
    pub db: SqlitePool,
    pub config: AccessConfig,
    pub host: Arc<dyn Host>,
    pub operations: BTreeMap<&'static str, Registered>,
    pub limits: Mutex<HashMap<String, (i64, u32)>>,
    pub public_limits: Mutex<HashMap<String, (i64, u32)>>,
    pub http: reqwest::Client,
}
impl AccessKit {
    pub async fn new(
        pool: SqlitePool,
        config: AccessConfig,
        host: Arc<dyn Host>,
    ) -> Result<Self, AccessError> {
        let version: i64 = sqlx::query_scalar("SELECT version FROM access_meta")
            .fetch_one(&pool)
            .await?;
        if version != i64::from(SCHEMA_VERSION) {
            return Err(AccessError::unavailable());
        }
        let mut operations = BTreeMap::new();
        for op in host.operations() {
            if !op.read_only
                || op.name.is_empty()
                || op.name.len() > 64
                || !op
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                || op.required_scope.is_empty()
                || op.required_scope.len() > 64
                || !op
                    .required_scope
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b':' || b == b'-')
                || !op.input_schema.is_object()
                || op.input_schema.to_string().len() > 32768
                || has_remote_ref(&op.input_schema)
                || operations.contains_key(op.name)
                || operations.len() >= 128
            {
                return Err(AccessError::bad(
                    "invalid_registry",
                    "Invalid operation registry",
                ));
            }
            let validator = jsonschema::validator_for(&op.input_schema)
                .map_err(|_| AccessError::bad("invalid_registry", "Invalid input schema"))?;
            operations.insert(
                op.name,
                Registered {
                    operation: op,
                    validator,
                },
            );
        }
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("access-kit/0.1")
            .build()
            .map_err(|_| AccessError::unavailable())?;
        audit::maintain(&pool).await?;
        let kit = Self(Arc::new(Inner {
            db: pool,
            config,
            host,
            operations,
            limits: Mutex::new(HashMap::new()),
            public_limits: Mutex::new(HashMap::new()),
            http,
        }));
        let weak = Arc::downgrade(&kit.0);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                let Some(inner) = weak.upgrade() else { break };
                let _ = audit::maintain(&inner.db).await;
            }
        });
        Ok(kit)
    }
    pub fn router(&self) -> Router {
        http::router(self.clone()).merge(mcp::router(self.clone()))
    }
    pub(crate) fn scopes(&self) -> Vec<String> {
        let mut scopes = self
            .0
            .operations
            .values()
            .map(|r| r.operation.required_scope.to_string())
            .collect::<Vec<_>>();
        scopes.sort();
        scopes.dedup();
        scopes
    }
    pub(crate) fn validate_scopes(&self, scopes: &[String]) -> Result<(), AccessError> {
        let allowed = self.scopes();
        if scopes.is_empty() || scopes.len() > 32 || scopes.iter().any(|v| !allowed.contains(v)) {
            return Err(AccessError::bad("invalid_scope", "Unknown or empty scopes"));
        }
        Ok(())
    }
    pub(crate) fn rate(&self, key: String, max: u32) -> Result<(), AccessError> {
        Self::rate_in(&self.0.limits, key, max, None)
    }
    pub(crate) fn public_rate(&self, key: String, max: u32) -> Result<(), AccessError> {
        let namespace = key.split(':').next().unwrap_or("").to_owned();
        Self::rate_in(&self.0.public_limits, key, max, Some(&namespace))
    }
    fn rate_in(
        buckets: &Mutex<HashMap<String, (i64, u32)>>,
        key: String,
        max: u32,
        namespace: Option<&str>,
    ) -> Result<(), AccessError> {
        let now = crypto::now();
        let mut limits = buckets.lock().map_err(|_| AccessError::unavailable())?;
        limits.retain(|_, (at, _)| now - *at < 60);
        let occupied = match namespace {
            Some(prefix) => limits
                .keys()
                .filter(|key| key.split(':').next() == Some(prefix))
                .count(),
            None => limits.len(),
        };
        if !limits.contains_key(&key) && occupied >= 4096 {
            return Err(AccessError::new(
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "Rate limit reached",
            ));
        }
        let (_, count) = limits.entry(key).or_insert((now, 0));
        if *count >= max {
            return Err(AccessError::new(
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "Rate limit reached",
            ));
        }
        *count += 1;
        Ok(())
    }
}
fn has_remote_ref(value: &Value) -> bool {
    match value {
        Value::Object(o) => o.iter().any(|(key, v)| {
            key == "$ref" && v.as_str().is_some_and(|s| !s.starts_with('#')) || has_remote_ref(v)
        }),
        Value::Array(a) => a.iter().any(has_remote_ref),
        _ => false,
    }
}
