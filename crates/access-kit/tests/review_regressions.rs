mod support;
use access_kit::*;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;
async fn req(
    app: &Router,
    path: &str,
    content: &str,
    cookie: &str,
    body: String,
) -> (StatusCode, Value) {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", content)
                .header("cookie", cookie)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let body = to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}
#[tokio::test]
async fn invalid_token_requests_do_not_block_authenticated_management() {
    let db = support::database().await;
    let kit = AccessKit::new(
        db,
        AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
        support::TestHost::new(),
    )
    .await
    .unwrap();
    let app = kit.router();
    for n in 0..4096 {
        let (s, _) = req(
            &app,
            "/oauth/token",
            "application/x-www-form-urlencoded",
            "",
            format!("client_id=attacker{n}&grant_type=invalid"),
        )
        .await;
        assert!(matches!(
            s,
            StatusCode::BAD_REQUEST | StatusCode::TOO_MANY_REQUESTS
        ));
    }
    let (s, v) = req(
        &app,
        "/api/access/credentials",
        "application/json",
        "session=owner",
        json!({"name":"legitimate","scopes":["reports:read"]}).to_string(),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
}
#[tokio::test]
async fn public_registration_quota_does_not_block_admin_registration() {
    let db = support::database().await;
    let kit = AccessKit::new(
        db.clone(),
        AccessConfig::for_origin("http://127.0.0.1:9999")
            .unwrap()
            .with_dynamic_client_registration(true),
        support::TestHost::new(),
    )
    .await
    .unwrap();
    let app = kit.router();
    for n in 0..256 {
        let request = Request::builder()
            .method("POST")
            .uri("/oauth/register")
            .header("content-type", "application/json")
            .extension(axum::extract::ConnectInfo(std::net::SocketAddr::from((
                [10, 0, (n / 256) as u8, (n % 256) as u8],
                10000,
            ))))
            .body(Body::from(
                json!({"client_name":"attacker","redirect_uris":["https://example.com/callback"]})
                    .to_string(),
            ))
            .unwrap();
        let res = app.clone().oneshot(request).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
    }
    drop(app);
    drop(kit);
    let kit = AccessKit::new(
        db.clone(),
        AccessConfig::for_origin("http://127.0.0.1:9999")
            .unwrap()
            .with_dynamic_client_registration(true),
        support::TestHost::new(),
    )
    .await
    .unwrap();
    let (s, v) = req(
        &kit.router(),
        "/api/access/clients",
        "application/json",
        "session=owner",
        json!({"client_name":"legitimate","redirect_uris":["https://example.com/legitimate"]})
            .to_string(),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    sqlx::query(
        "UPDATE access_clients SET created_at=created_at-86401 WHERE public_registration=1",
    )
    .execute(&db)
    .await
    .unwrap();
    let (status, _) = req(
        &kit.router(),
        "/oauth/register",
        "application/json",
        "",
        json!({"client_name":"new pending","redirect_uris":["https://example.com/callback"]})
            .to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let counts: (i64, i64) =
        sqlx::query_as("SELECT COUNT(*),SUM(public_registration) FROM access_clients")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(counts, (2, 1)); // Trusted client retained; unused public registrations reclaimed.
}

struct LargeHost(std::sync::Arc<support::TestHost>);
impl Host for LargeHost {
    fn cookie_principal<'a>(
        &'a self,
        h: &'a axum::http::HeaderMap,
    ) -> HostFuture<'a, Option<CookieSession>> {
        self.0.cookie_principal(h)
    }
    fn active_principal(&self, id: i64) -> HostFuture<'_, Option<Principal>> {
        self.0.active_principal(id)
    }
    fn create_session(&self, id: i64) -> HostFuture<'_, axum::http::HeaderValue> {
        self.0.create_session(id)
    }
    fn operations(&self) -> Vec<DomainOperation> {
        self.0.operations()
    }
    fn paths(&self) -> HostPaths {
        self.0.paths()
    }
    fn execute(&self, _: Invocation) -> HostFuture<'_, Value> {
        Box::pin(async { Ok(json!({"text":"x".repeat(2200000)})) })
    }
}
#[tokio::test]
async fn mcp_bounded_output_failure_is_audited_as_failure() {
    let db = support::database().await;
    let kit = AccessKit::new(
        db.clone(),
        AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
        std::sync::Arc::new(LargeHost(support::TestHost::new())),
    )
    .await
    .unwrap();
    let app = kit.router();
    let (_, key) = req(
        &app,
        "/api/access/credentials",
        "application/json",
        "session=owner",
        json!({"name":"large output","scopes":["reports:read"]}).to_string(),
    )
    .await;
    let request=Request::builder().method("POST").uri("/mcp").header("host","127.0.0.1:9999").header("authorization",format!("Bearer {}",key["token"].as_str().unwrap())).header("content-type","application/json").header("MCP-Protocol-Version","2026-07-28").header("Mcp-Method","tools/call").header("Mcp-Name","read_report").header("Accept","application/json, text/event-stream").body(Body::from(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"read_report","arguments":{},"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}).to_string())).unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 5000000).await.unwrap();
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "{}",
        String::from_utf8_lossy(&body[..body.len().min(500)])
    );
    let outcome: String =
        sqlx::query_scalar("SELECT outcome FROM access_events WHERE kind='operation'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(outcome, "failure");
}

#[tokio::test]
async fn concurrent_wal_calls_finalize_every_audit_once() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(tmp.path())
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                .busy_timeout(std::time::Duration::from_secs(5)),
        )
        .await
        .unwrap();
    sqlx::raw_sql(SCHEMA_SQL).execute(&db).await.unwrap();
    let host = support::TestHost::new();
    host.delay_ms.store(5, std::sync::atomic::Ordering::SeqCst);
    let kit = AccessKit::new(
        db.clone(),
        AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
        host.clone(),
    )
    .await
    .unwrap();
    let app = kit.router();
    let (_, key) = req(
        &app,
        "/api/access/credentials",
        "application/json",
        "session=owner",
        json!({"name":"parallel","scopes":["reports:read"]}).to_string(),
    )
    .await;
    let mut tasks = Vec::new();
    for _ in 0..30 {
        let app = app.clone();
        let token = key["token"].as_str().unwrap().to_string();
        tasks.push(tokio::spawn(async move {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/read_report")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
        }));
    }
    let mut failures = 0;
    for task in tasks {
        if task.await.unwrap() == StatusCode::SERVICE_UNAVAILABLE {
            failures += 1;
        }
    }
    let pending: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM access_events WHERE outcome='pending'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(failures, 0);
    assert_eq!(pending, 0);
    assert_eq!(host.calls.load(std::sync::atomic::Ordering::SeqCst), 30);
}

#[tokio::test]
async fn public_registration_is_disabled_by_default_and_not_advertised() {
    let kit = AccessKit::new(
        support::database().await,
        AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
        support::TestHost::new(),
    )
    .await
    .unwrap();
    let (status, value) = req(
        &kit.router(),
        "/oauth/register",
        "application/json",
        "",
        json!({"client_name":"public","redirect_uris":["https://example.com/callback"]})
            .to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["error"], "registration_disabled");
    let response = kit
        .router()
        .oneshot(
            Request::builder()
                .uri("/.well-known/oauth-authorization-server")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100000).await.unwrap()).unwrap();
    assert!(value.get("registration_endpoint").is_none());
}

#[tokio::test]
async fn schema_upgrade_preserves_managed_clients() {
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../migrations/0001_access.sql"))
        .execute(&db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO access_clients VALUES('managed','Managed','[\"https://example.com/cb\"]',1)",
    )
    .execute(&db)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!("../migrations/0002_client_registration.sql"))
        .execute(&db)
        .await
        .unwrap();
    let _kit = AccessKit::new(
        db.clone(),
        AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
        support::TestHost::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT public_registration FROM access_clients WHERE client_id='managed'"
        )
        .fetch_one(&db)
        .await
        .unwrap(),
        0
    );
}
