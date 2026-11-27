use crate::AccessError;
use axum::http::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{future::Future, pin::Pin};
pub type HostFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, AccessError>> + Send + 'a>>;
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Member,
}
#[derive(Clone, Serialize)]
pub struct Principal {
    pub user_id: i64,
    pub username: String,
    pub display_name: String,
    pub role: Role,
}
pub struct CookieSession {
    pub principal: Principal,
    pub session_binding: [u8; 32],
}
pub struct HostPaths {
    pub login: &'static str,
    pub connections: &'static str,
    pub consent: &'static str,
}
#[derive(Clone)]
pub struct DomainOperation {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    pub required_scope: &'static str,
    pub read_only: bool,
}
pub struct RegistrationUser {
    pub username: String,
    pub display_name: String,
    pub password_hash: String,
    pub account_status: String,
    pub password_login_enabled: bool,
}
pub struct RegistrationAccount {
    pub status: String,
    pub password_login_enabled: bool,
}
pub trait Host: Send + Sync + 'static {
    fn registration_admin<'a>(
        &'a self,
        connection: &'a mut sqlx::SqliteConnection,
        headers: &'a HeaderMap,
        user_id: i64,
    ) -> HostFuture<'a, bool> {
        let _ = (connection, headers, user_id);
        Box::pin(async { Err(AccessError::unavailable()) })
    }
    fn registration_count<'a>(
        &'a self,
        connection: &'a mut sqlx::SqliteConnection,
    ) -> HostFuture<'a, i64> {
        let _ = connection;
        Box::pin(async { Err(AccessError::unavailable()) })
    }
    fn registration_create<'a>(
        &'a self,
        connection: &'a mut sqlx::SqliteConnection,
        user: RegistrationUser,
    ) -> HostFuture<'a, i64> {
        let _ = (connection, user);
        Box::pin(async { Err(AccessError::unavailable()) })
    }
    fn registration_account<'a>(
        &'a self,
        connection: &'a mut sqlx::SqliteConnection,
        user_id: i64,
    ) -> HostFuture<'a, Option<RegistrationAccount>> {
        let _ = (connection, user_id);
        Box::pin(async { Err(AccessError::unavailable()) })
    }
    fn registration_set_status<'a>(
        &'a self,
        connection: &'a mut sqlx::SqliteConnection,
        user_id: i64,
        status: &'a str,
    ) -> HostFuture<'a, bool> {
        let _ = (connection, user_id, status);
        Box::pin(async { Err(AccessError::unavailable()) })
    }

    fn cookie_principal<'a>(
        &'a self,
        headers: &'a HeaderMap,
    ) -> HostFuture<'a, Option<CookieSession>>;
    fn active_principal(&self, user_id: i64) -> HostFuture<'_, Option<Principal>>;
    fn create_session(&self, user_id: i64) -> HostFuture<'_, HeaderValue>;
    fn operations(&self) -> Vec<DomainOperation>;
    fn execute(&self, invocation: Invocation) -> HostFuture<'_, Value>;
    fn paths(&self) -> HostPaths;
}
#[derive(Clone)]
pub struct TrustedPrincipal {
    pub(crate) user_id: i64,
}
impl TrustedPrincipal {
    pub fn user_id(&self) -> i64 {
        self.user_id
    }
}
pub struct Invocation {
    pub(crate) principal: Principal,
    pub(crate) operation: String,
    pub(crate) arguments: Value,
}
impl Invocation {
    pub fn principal(&self) -> &Principal {
        &self.principal
    }
    pub fn operation(&self) -> &str {
        &self.operation
    }
    pub fn arguments(&self) -> &Value {
        &self.arguments
    }
    pub fn trusted_principal(&self) -> TrustedPrincipal {
        TrustedPrincipal {
            user_id: self.principal.user_id,
        }
    }
}
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Api,
    Mcp,
}
impl Transport {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Mcp => "mcp",
        }
    }
}
#[derive(Clone)]
pub(crate) struct AuditTicket {
    pub id: String,
    pub started: std::time::Instant,
    pub domain_status: std::sync::Arc<std::sync::atomic::AtomicU16>,
}
#[derive(Clone)]
pub(crate) struct AuthContext {
    pub principal: Principal,
    pub scopes: Vec<String>,
    pub credential_id: Option<String>,
    pub client_id: Option<String>,
    pub grant_id: Option<String>,
    pub request_id: String,
    pub audit_ticket: Option<AuditTicket>,
}
