//! Host adapter: existing sessions and domain authorization remain authoritative.
use crate::{access_registry, auth, AppState, Inner};
use access_kit::{
    AccessConfig, AccessError, AccessKit, CookieSession, DomainOperation, Host, HostFuture,
    HostPaths, Invocation, Principal, Role,
};
use axum::{
    body::{to_bytes, Body},
    http::{HeaderMap, HeaderValue, Request, StatusCode},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Weak};
use tower::ServiceExt;

struct HostAdapter {
    state: Weak<Inner>,
    secure: bool,
}
fn unavailable() -> AccessError {
    AccessError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "host_unavailable",
        "Host service is unavailable",
    )
}
fn principal(user: auth::CurrentUser) -> Principal {
    Principal {
        user_id: user.id,
        username: user.username,
        display_name: user.display_name,
        role: if user.role == "admin" {
            Role::Admin
        } else {
            Role::Member
        },
    }
}
impl HostAdapter {
    fn state(&self) -> Result<AppState, AccessError> {
        self.state.upgrade().ok_or_else(unavailable)
    }
}
impl Host for HostAdapter {
    fn registration_admin<'a>(
        &'a self,
        connection: &'a mut sqlx::SqliteConnection,
        headers: &'a HeaderMap,
        id: i64,
    ) -> HostFuture<'a, bool> {
        Box::pin(async move {
            let Some(token) = auth::read_cookie(headers, auth::COOKIE_NAME) else {
                return Ok(false);
            };
            let hash = Sha256::digest(token.as_bytes())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.token_hash=? AND s.user_id=? AND s.expires_at>? AND u.disabled=0 AND u.account_status='active' AND u.role='admin')").bind(hash).bind(id).bind(crate::db::now()).fetch_one(connection).await?)
        })
    }

    fn registration_count<'a>(
        &'a self,
        connection: &'a mut sqlx::SqliteConnection,
    ) -> HostFuture<'a, i64> {
        Box::pin(async move {
            Ok(sqlx::query_scalar("SELECT COUNT(*) FROM users")
                .fetch_one(connection)
                .await?)
        })
    }
    fn registration_create<'a>(
        &'a self,
        connection: &'a mut sqlx::SqliteConnection,
        user: access_kit::RegistrationUser,
    ) -> HostFuture<'a, i64> {
        Box::pin(async move {
            let id=sqlx::query("INSERT INTO users(username,display_name,password_hash,role,created_at,account_status,password_login_enabled) VALUES(?,?,?,'member',?,?,?)")
 .bind(&user.username).bind(&user.display_name).bind(&user.password_hash).bind(crate::db::now()).bind(&user.account_status).bind(user.password_login_enabled).execute(connection).await.map_err(|e| if e.as_database_error().is_some_and(|e|e.is_unique_violation()){AccessError::new(StatusCode::CONFLICT,"username_taken","Username is already in use")}else{unavailable()})?.last_insert_rowid();
            Ok(id)
        })
    }
    fn registration_account<'a>(
        &'a self,
        connection: &'a mut sqlx::SqliteConnection,
        id: i64,
    ) -> HostFuture<'a, Option<access_kit::RegistrationAccount>> {
        Box::pin(async move {
            let row: Option<(String, bool)> = sqlx::query_as(
                "SELECT account_status,password_login_enabled FROM users WHERE id=?",
            )
            .bind(id)
            .fetch_optional(connection)
            .await?;
            Ok(row.map(
                |(status, password_login_enabled)| access_kit::RegistrationAccount {
                    status,
                    password_login_enabled,
                },
            ))
        })
    }
    fn registration_set_status<'a>(
        &'a self,
        connection: &'a mut sqlx::SqliteConnection,
        id: i64,
        status: &'a str,
    ) -> HostFuture<'a, bool> {
        Box::pin(async move {
            Ok(sqlx::query("UPDATE users SET account_status=? WHERE id=? AND account_status='pending' AND role='member'").bind(status).bind(id).execute(connection).await?.rows_affected()==1)
        })
    }

    fn cookie_principal<'a>(
        &'a self,
        headers: &'a HeaderMap,
    ) -> HostFuture<'a, Option<CookieSession>> {
        Box::pin(async move {
            let state = self.state()?;
            let Some(user) = auth::optional_user(&state.db, headers)
                .await
                .map_err(|_| unavailable())?
            else {
                return Ok(None);
            };
            let Some(token) = auth::read_cookie(headers, auth::COOKIE_NAME) else {
                return Ok(None);
            };
            Ok(Some(CookieSession {
                principal: principal(user),
                session_binding: Sha256::digest(token.as_bytes()).into(),
            }))
        })
    }
    fn active_principal(&self, user_id: i64) -> HostFuture<'_, Option<Principal>> {
        Box::pin(async move {
            let state = self.state()?;
            Ok(auth::active_user(&state.db, user_id)
                .await
                .map_err(|_| unavailable())?
                .map(principal))
        })
    }
    fn create_session(&self, user_id: i64) -> HostFuture<'_, HeaderValue> {
        Box::pin(async move {
            let state = self.state()?;
            if auth::active_user(&state.db, user_id)
                .await
                .map_err(|_| unavailable())?
                .is_none()
            {
                return Err(AccessError::new(
                    StatusCode::UNAUTHORIZED,
                    "account_unavailable",
                    "Account is unavailable",
                ));
            }
            let token = auth::create_session(&state.db, user_id)
                .await
                .map_err(|_| unavailable())?;
            HeaderValue::from_str(&auth::session_cookie(&token, self.secure))
                .map_err(|_| unavailable())
        })
    }
    fn operations(&self) -> Vec<DomainOperation> {
        access_registry::operations()
            .into_iter()
            .map(|op| DomainOperation {
                name: op.name,
                description: op.description,
                input_schema: op.schema(),
                required_scope: op.scope,
                read_only: true,
            })
            .collect()
    }

    fn execute(&self, invocation: Invocation) -> HostFuture<'_, Value> {
        Box::pin(async move {
            let state = self.state()?;
            let path =
                access_registry::request_path(invocation.operation(), invocation.arguments())
                    .map_err(|_| {
                        AccessError::new(
                            StatusCode::BAD_REQUEST,
                            "invalid_arguments",
                            "Invalid domain arguments",
                        )
                    })?;
            let request = Request::builder()
                .uri(path)
                .extension(invocation.trusted_principal())
                .body(Body::empty())
                .map_err(|_| unavailable())?;
            let response = crate::routes::domain_router(state)
                .oneshot(request)
                .await
                .map_err(|_| unavailable())?;
            let status = response.status();
            if !status.is_success() {
                let status = match status {
                    StatusCode::BAD_REQUEST
                    | StatusCode::UNAUTHORIZED
                    | StatusCode::FORBIDDEN
                    | StatusCode::NOT_FOUND
                    | StatusCode::TOO_MANY_REQUESTS
                    | StatusCode::SERVICE_UNAVAILABLE => status,
                    _ => StatusCode::SERVICE_UNAVAILABLE,
                };
                return Err(AccessError::new(
                    status,
                    "domain_denied",
                    "Domain request could not be completed with the current account and arguments",
                ));
            }
            let bytes = to_bytes(response.into_body(), 1024 * 1024)
                .await
                .map_err(|_| unavailable())?;
            let value = serde_json::from_slice(&bytes).map_err(|_| unavailable())?;
            access_registry::project(invocation.operation(), value).map_err(|_| unavailable())
        })
    }
    fn paths(&self) -> HostPaths {
        HostPaths {
            login: "/",
            connections: "/connections",
            consent: "/access/authorize",
        }
    }
}
pub async fn initialize(state: &AppState) -> anyhow::Result<()> {
    let default_origin = state
        .config
        .public_url
        .as_deref()
        .unwrap_or("http://127.0.0.1:8585");
    let config = AccessConfig::from_env(default_origin)?;
    let host = Arc::new(HostAdapter {
        state: Arc::downgrade(state),
        secure: config.secure_cookies(),
    });
    let kit = AccessKit::new(state.db.clone(), config, host).await?;
    state
        .access
        .set(kit)
        .map_err(|_| anyhow::anyhow!("access kit already initialized"))?;
    Ok(())
}
