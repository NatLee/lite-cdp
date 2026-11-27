use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use lite_cdp::{app, auth, build_state, AppState, Config};
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::ServiceExt;
struct Fixture {
    app: Router,
    state: AppState,
    admin: String,
    member: String,
    _dir: TempDir,
}
impl Fixture {
    async fn new(origin: Option<String>) -> Self {
        let dir = TempDir::new().unwrap();
        let config = Config {
            bind: "127.0.0.1:0".into(),
            database_path: dir.path().join("access.db"),
            static_dir: dir.path().join("web"),
            public_url: Some(
                origin
                    .clone()
                    .unwrap_or_else(|| "https://service.example.invalid".into()),
            ),
            public_origin: Some(origin.unwrap_or_else(|| "https://service.example.invalid".into())),
            cookie_secure: false,
            trust_proxy: false,
            rate_limit: 0,
            retention_days: 0,
        };
        let state = build_state(config).await.unwrap();
        let admin = auth::create_user(&state.db, "admin", "Admin", "test-password-123", "admin")
            .await
            .unwrap();
        let member =
            auth::create_user(&state.db, "member", "Member", "test-password-123", "member")
                .await
                .unwrap();
        let admin = format!(
            "{}={}",
            auth::COOKIE_NAME,
            auth::create_session(&state.db, admin).await.unwrap()
        );
        let member = format!(
            "{}={}",
            auth::COOKIE_NAME,
            auth::create_session(&state.db, member).await.unwrap()
        );
        sqlx::query("INSERT INTO sites(id,name,url,key,created_at)VALUES(1,'Visible','https://one.example.invalid','SITE-KEY-CANARY',?),(2,'Private','https://two.example.invalid','OTHER-KEY-CANARY',?)").bind(lite_cdp::db::now()).bind(lite_cdp::db::now()).execute(&state.db).await.unwrap();
        sqlx::query("INSERT INTO site_members(site_id,user_id)VALUES(1,2)")
            .execute(&state.db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO people(id,site_id,email,name,first_seen,last_seen,identified_at)VALUES(1,1,'PRIVATE-EMAIL-CANARY@example.invalid','Private Name',?,?,?)").bind(lite_cdp::db::now()).bind(lite_cdp::db::now()).bind(lite_cdp::db::now()).execute(&state.db).await.unwrap();
        Self {
            app: app(state.clone()),
            state,
            admin,
            member,
            _dir: dir,
        }
    }
    async fn call(
        &self,
        method: &str,
        path: &str,
        cookie: Option<&str>,
        bearer: Option<&str>,
        body: Value,
    ) -> (StatusCode, Value) {
        let mut r = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json");
        if let Some(v) = cookie {
            r = r.header("cookie", v);
        }
        if let Some(v) = bearer {
            r = r.header("authorization", format!("Bearer {v}"));
        }
        let response = self
            .app
            .clone()
            .oneshot(r.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let s = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (s, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }
    async fn key(&self, scopes: Value) -> Value {
        let (s, v) = self
            .call(
                "POST",
                "/api/access/credentials",
                Some(&self.member),
                None,
                json!({"name":"test-client","scopes":scopes}),
            )
            .await;
        assert_eq!(s, StatusCode::CREATED);
        v
    }
}
#[tokio::test]
async fn api_key_reuses_membership_counts_once_and_revokes() {
    let f = Fixture::new(None).await;
    let key = f.key(json!(["reports:read"])).await;
    let token = key["token"].as_str().unwrap();
    let (s, v) = f
        .call(
            "POST",
            "/api/v1/get_site_overview",
            None,
            Some(token),
            json!({"site_id":1}),
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert!(v.get("totals").is_some());
    assert!(v.get("recent_identified").is_none());
    assert!(!v.to_string().contains("PRIVATE-EMAIL-CANARY"));
    let (_, usage) = f
        .call(
            "GET",
            "/api/access/usage?transport=api",
            Some(&f.member),
            None,
            Value::Null,
        )
        .await;
    assert_eq!(usage["totals"]["attempts"], 1);
    assert_eq!(usage["totals"]["successes"], 1);
    let (s, _) = f
        .call(
            "POST",
            "/api/v1/get_site_overview",
            None,
            Some(token),
            json!({"site_id":2}),
        )
        .await;
    assert!(matches!(s, StatusCode::NOT_FOUND | StatusCode::FORBIDDEN));
    sqlx::query("DELETE FROM site_members WHERE site_id=1 AND user_id=2")
        .execute(&f.state.db)
        .await
        .unwrap();
    let (s, _) = f
        .call(
            "POST",
            "/api/v1/get_site_overview",
            None,
            Some(token),
            json!({"site_id":1}),
        )
        .await;
    assert!(matches!(s, StatusCode::NOT_FOUND | StatusCode::FORBIDDEN));
    let (s, _) = f
        .call(
            "DELETE",
            &format!(
                "/api/access/credentials/{}",
                key["credential"]["id"].as_str().unwrap()
            ),
            Some(&f.member),
            None,
            Value::Null,
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    assert_eq!(
        f.call(
            "POST",
            "/api/v1/get_site_overview",
            None,
            Some(token),
            json!({"site_id":1})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}
#[tokio::test]
async fn disabled_account_and_forged_trusted_header_do_not_authenticate() {
    let f = Fixture::new(None).await;
    let key = f.key(json!(["reports:read"])).await;
    sqlx::query("UPDATE users SET disabled=1 WHERE id=2")
        .execute(&f.state.db)
        .await
        .unwrap();
    assert_eq!(
        f.call(
            "POST",
            "/api/v1/get_site_overview",
            None,
            Some(key["token"].as_str().unwrap()),
            json!({"site_id":1})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let r = f
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/sites/1/overview")
                .header("x-trusted-principal", "1")
                .header("x-user-id", "1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_site_projection_never_exposes_collection_secrets() {
    let f = Fixture::new(None).await;
    let (status, key) = f
        .call(
            "POST",
            "/api/access/credentials",
            Some(&f.admin),
            None,
            json!({"name":"admin reader","scopes":["reports:read"]}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, value) = f
        .call(
            "POST",
            "/api/v1/list_sites",
            None,
            Some(key["token"].as_str().unwrap()),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value.as_array().unwrap().len(), 2);
    let text = value.to_string();
    for canary in [
        "SITE-KEY-CANARY",
        "OTHER-KEY-CANARY",
        "members",
        "webhook_url",
        "notify_email",
    ] {
        assert!(!text.contains(canary));
    }
}
#[tokio::test]
async fn real_official_sdk_discovers_calls_counts_and_rejects_revocation() {
    use rmcp::{
        model::{CallToolRequestParams, ProtocolVersion},
        transport::{
            streamable_http_client::StreamableHttpClientTransportConfig,
            StreamableHttpClientTransport,
        },
        ClientLifecycleMode, ClientServiceExt,
    };
    use std::future::IntoFuture;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let f = Fixture::new(Some(origin.clone())).await;
    let server = tokio::spawn(axum::serve(listener, f.app.clone()).into_future());
    let (status, key) = f
        .call(
            "POST",
            "/api/access/credentials",
            Some(&f.member),
            None,
            json!({"name":"SDK reader","scopes":["reports:read"],"audiences":["mcp"]}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let token = key["token"].as_str().unwrap();
    assert_eq!(
        f.call(
            "POST",
            "/api/v1/get_site_overview",
            None,
            Some(token),
            json!({"site_id":1})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let transport = StreamableHttpClientTransport::from_config(
        StreamableHttpClientTransportConfig::with_uri(format!("{origin}/mcp")).auth_header(token),
    );
    let client = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        ().serve_with_lifecycle(
            transport,
            ClientLifecycleMode::Discover {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            },
        ),
    )
    .await
    .unwrap()
    .unwrap();
    let tools = client.list_tools(None).await.unwrap();
    assert_eq!(tools.tools.len(), 4);
    let (_, usage) = f
        .call(
            "GET",
            "/api/access/usage?transport=mcp",
            Some(&f.member),
            None,
            Value::Null,
        )
        .await;
    assert_eq!(usage["totals"]["attempts"], 0);
    let result = client
        .call_tool(
            CallToolRequestParams::new("get_site_overview")
                .with_arguments(json!({"site_id":1}).as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    assert_ne!(serde_json::to_value(result).unwrap()["isError"], true);
    let (_, usage) = f
        .call(
            "GET",
            "/api/access/usage?transport=mcp",
            Some(&f.member),
            None,
            Value::Null,
        )
        .await;
    assert_eq!(usage["totals"]["attempts"], 1);
    assert_eq!(usage["totals"]["successes"], 1);
    assert_eq!(
        f.call(
            "DELETE",
            &format!(
                "/api/access/credentials/{}",
                key["credential"]["id"].as_str().unwrap()
            ),
            Some(&f.member),
            None,
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert!(client
        .call_tool(
            CallToolRequestParams::new("get_site_overview")
                .with_arguments(json!({"site_id":1}).as_object().unwrap().clone())
        )
        .await
        .is_err());
    server.abort();
}

#[tokio::test]
async fn personal_events_require_explicit_people_scope_and_current_site_membership() {
    let f = Fixture::new(None).await;
    let reports = f.key(json!(["reports:read"])).await;
    assert_eq!(
        f.call(
            "POST",
            "/api/v1/get_person_events",
            None,
            Some(reports["token"].as_str().unwrap()),
            json!({"site_id":1,"person_id":1})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let people = f.key(json!(["people:read"])).await;
    assert_eq!(
        f.call(
            "POST",
            "/api/v1/get_person_events",
            None,
            Some(people["token"].as_str().unwrap()),
            json!({"site_id":1,"person_id":1})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        f.call(
            "POST",
            "/api/v1/get_person_events",
            None,
            Some(people["token"].as_str().unwrap()),
            json!({"site_id":2,"person_id":1})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[test]
fn access_schema_is_the_frozen_snapshot() {
    assert!(
        include_str!("../migrations/0004_registration.sql").starts_with(include_str!(
            "../../../crates/access-kit/migrations/0003_registration.sql"
        ))
    );
    assert_eq!(
        concat!(
            include_str!("../migrations/0002_access.sql"),
            include_str!("../migrations/0003_access_client_registration.sql"),
            include_str!("../../../crates/access-kit/migrations/0003_registration.sql")
        ),
        access_kit::SCHEMA_SQL
    );
}
