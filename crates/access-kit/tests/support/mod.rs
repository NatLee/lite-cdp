use access_kit::*;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    Arc,
};
pub struct TestHost {
    pub active: AtomicBool,
    pub permission: AtomicBool,
    pub calls: AtomicUsize,
    pub admin: AtomicBool,
    pub generation: AtomicUsize,
    pub delay_ms: AtomicU64,
    pub variant: AtomicUsize,
}
impl TestHost {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            active: AtomicBool::new(true),
            permission: AtomicBool::new(true),
            calls: AtomicUsize::new(0),
            admin: AtomicBool::new(true),
            generation: AtomicUsize::new(1),
            delay_ms: AtomicU64::new(0),
            variant: AtomicUsize::new(0),
        })
    }
    fn principal(&self) -> Principal {
        Principal {
            user_id: 1,
            username: "demo".into(),
            display_name: "Demo".into(),
            role: if self.admin.load(Ordering::SeqCst) {
                Role::Admin
            } else {
                Role::Member
            },
        }
    }
}
impl Host for TestHost {
    fn cookie_principal<'a>(&'a self, h: &'a HeaderMap) -> HostFuture<'a, Option<CookieSession>> {
        Box::pin(async move {
            Ok((h
                .get("cookie")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v.split(';').any(|p| p.trim() == "session=owner"))
                && self.active.load(Ordering::SeqCst))
            .then(|| CookieSession {
                principal: self.principal(),
                session_binding: [self.generation.load(Ordering::SeqCst) as u8; 32],
            }))
        })
    }
    fn active_principal(&self, id: i64) -> HostFuture<'_, Option<Principal>> {
        Box::pin(async move {
            Ok((id == 1 && self.active.load(Ordering::SeqCst)).then(|| self.principal()))
        })
    }
    fn create_session(&self, _: i64) -> HostFuture<'_, HeaderValue> {
        Box::pin(async {
            Ok(HeaderValue::from_static(
                "session=owner; HttpOnly; SameSite=Lax; Path=/",
            ))
        })
    }
    fn operations(&self) -> Vec<DomainOperation> {
        let mut ops = vec![
            DomainOperation {
                name: "read_report",
                description: "Read the authorized report",
                input_schema: json!({"type":"object","properties":{"value":{"type":"integer"}},"additionalProperties":false}),
                required_scope: "reports:read",
                read_only: true,
            },
            DomainOperation {
                name: "read_people",
                description: "Read authorized people",
                input_schema: json!({"type":"object","additionalProperties":false}),
                required_scope: "people:read",
                read_only: true,
            },
        ];
        match self.variant.load(Ordering::SeqCst) {
            1 => ops.push(ops[0].clone()),
            2 => ops[0].read_only = false,
            3 => ops[0].input_schema = json!({"$ref":"https://untrusted.invalid/schema"}),
            4 => ops[0].input_schema["properties"]["value"]["type"] = json!("string"),
            _ => {}
        }
        ops
    }
    fn execute(&self, call: Invocation) -> HostFuture<'_, Value> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(std::time::Duration::from_millis(
                self.delay_ms.load(Ordering::SeqCst),
            ))
            .await;
            if !self.permission.load(Ordering::SeqCst) {
                return Err(AccessError::new(
                    StatusCode::FORBIDDEN,
                    "access_denied",
                    "Access denied",
                ));
            }
            Ok(json!({"user_id":call.principal().user_id,"args":call.arguments()}))
        })
    }
    fn paths(&self) -> HostPaths {
        HostPaths {
            login: "/login",
            connections: "/settings/access",
            consent: "/access/authorize",
        }
    }
}
pub async fn database() -> sqlx::SqlitePool {
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(SCHEMA_SQL).execute(&db).await.unwrap();
    db
}
