use axum::{
    body::{to_bytes, Body},
    extract::ConnectInfo,
    http::{HeaderMap, Request, StatusCode},
    Router,
};
use lite_cdp::{app, auth, build_state, Config};
use serde_json::{json, Value};
use tower::ServiceExt;
async fn req(
    app: &Router,
    method: &str,
    path: &str,
    cookie: &str,
    body: Value,
) -> (StatusCode, Value, HeaderMap) {
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .extension(ConnectInfo(
                    "127.0.0.1:43210".parse::<std::net::SocketAddr>().unwrap(),
                ))
                .header("content-type", "application/json")
                .header("origin", "http://127.0.0.1:9999")
                .header("cookie", cookie)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let s = r.status();
    let h = r.headers().clone();
    let b = to_bytes(r.into_body(), 100000).await.unwrap();
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null), h)
}
async fn fixture() -> (tempfile::TempDir, lite_cdp::AppState, Router, String) {
    let dir = tempfile::TempDir::new().unwrap();
    let mut c = Config::from_env().unwrap();
    c.database_path = dir.path().join("test.db");
    c.static_dir = dir.path().join("web");
    c.public_origin = Some("http://127.0.0.1:9999".into());
    c.public_url = Some("http://127.0.0.1:9999".into());
    let state = build_state(c).await.unwrap();
    let router = app(state.clone());
    let (s, p, _) = req(
        &router,
        "GET",
        "/api/auth/registration-policy",
        "",
        Value::Null,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        p,
        json!({"mode":"closed","password_enabled":false,"providers":[],"setup_required":true})
    );
    let (s, _, h) = req(
        &router,
        "POST",
        "/api/auth/setup",
        "",
        json!({"username":"admin","password":"test-password-123"}),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let cookie = h["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    (dir, state, router, cookie)
}
#[tokio::test]
async fn registration_review_lifecycle_and_active_gates() {
    let (_dir, state, router, admin) = fixture().await;
    assert_eq!(
        req(
            &router,
            "POST",
            "/api/auth/register",
            "",
            json!({"username":"amy","password":"test-password-123"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        req(
            &router,
            "PATCH",
            "/api/auth/registration-policy",
            &admin,
            json!({"mode":"approval"})
        )
        .await
        .0,
        StatusCode::OK
    );
    let (s, v, h) = req(
        &router,
        "POST",
        "/api/auth/register",
        "",
        json!({"username":"amy","display_name":" Amy ","password":"test-password-123"}),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    assert_eq!(v["status"], "pending");
    assert!(!h.contains_key("set-cookie"));
    let id = v["user_id"].as_i64().unwrap();
    assert!(auth::create_session(&state.db, id).await.is_err());
    assert_eq!(
        req(
            &router,
            "POST",
            "/api/auth/login",
            "",
            json!({"username":"amy","password":"test-password-123"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    // Administrative enable/disable cannot replace review.
    sqlx::query("UPDATE users SET disabled=0 WHERE id=?")
        .bind(id)
        .execute(&state.db)
        .await
        .unwrap();
    assert!(auth::create_session(&state.db, id).await.is_err());
    let (s, v, _) = req(
        &router,
        "GET",
        "/api/auth/registrations?limit=1",
        &admin,
        Value::Null,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["items"][0]["display_name"], "Amy");
    let application = v["items"][0]["id"].as_i64().unwrap();
    let path = format!("/api/auth/registrations/{application}/review");
    let (a, b) = tokio::join!(
        req(
            &router,
            "POST",
            &path,
            &admin,
            json!({"decision":"approve"})
        ),
        req(
            &router,
            "POST",
            &path,
            &admin,
            json!({"decision":"reject","reason":"Concurrent review"})
        )
    );
    let statuses = [a.0, b.0];
    assert!(statuses.contains(&StatusCode::OK));
    assert!(statuses.contains(&StatusCode::CONFLICT));
    let status: String = sqlx::query_scalar("SELECT account_status FROM users WHERE id=?")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    if status == "active" {
        assert_eq!(
            req(
                &router,
                "POST",
                "/api/auth/login",
                "",
                json!({"username":"amy","password":"test-password-123"})
            )
            .await
            .0,
            StatusCode::OK
        )
    } else {
        assert!(auth::create_session(&state.db, id).await.is_err());
    }
    let (s, events, _) = req(
        &router,
        "GET",
        "/api/auth/registration-events",
        &admin,
        Value::Null,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(events["items"].as_array().unwrap().len(), 3);
    let text = events.to_string();
    assert!(!text.contains("test-password"));
    assert!(!text.contains("password_hash"));
    assert_eq!(
        req(
            &router,
            "PATCH",
            "/api/auth/registration-policy",
            &admin,
            json!({"mode":"open"})
        )
        .await
        .0,
        StatusCode::OK
    );
    let (s, v, h) = req(
        &router,
        "POST",
        "/api/auth/register",
        "",
        json!({"username":"bob","password":"test-password-123"}),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    assert_eq!(v["status"], "active");
    let cookie = h["set-cookie"].to_str().unwrap().split(';').next().unwrap();
    let bob = v["user_id"].as_i64().unwrap();
    sqlx::query("UPDATE users SET account_status='rejected' WHERE id=?")
        .bind(bob)
        .execute(&state.db)
        .await
        .unwrap();
    assert_eq!(
        req(&router, "GET", "/api/access/overview", cookie, Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        req(
            &router,
            "PATCH",
            "/api/auth/me",
            cookie,
            json!({"display_name":"bad"})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}
#[tokio::test]
async fn registration_validation_origins_limits_and_rejection() {
    let (_dir, state, router, admin) = fixture().await;
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/auth/registration-policy")
                .header("content-type", "application/json")
                .header("origin", "https://evil.example")
                .header("cookie", &admin)
                .body(Body::from(json!({"mode":"open"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    req(
        &router,
        "PATCH",
        "/api/auth/registration-policy",
        &admin,
        json!({"mode":"approval"}),
    )
    .await;
    assert_eq!(
        req(
            &router,
            "POST",
            "/api/auth/register",
            "",
            json!({"username":"eve","password":"test-password-123","role":"admin"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        req(
            &router,
            "POST",
            "/api/auth/register",
            "",
            json!({"username":"eve","password":"short"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (s, v, _) = req(
        &router,
        "POST",
        "/api/auth/register",
        "",
        json!({"username":"eve","password":"test-password-123"}),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    let id = v["user_id"].as_i64().unwrap();
    let application: i64 =
        sqlx::query_scalar("SELECT id FROM access_registrations WHERE user_id=?")
            .bind(id)
            .fetch_one(&state.db)
            .await
            .unwrap();
    let path = format!("/api/auth/registrations/{application}/review");
    assert_eq!(
        req(
            &router,
            "POST",
            &path,
            &admin,
            json!({"decision":"reject","reason":"  "})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (s, v, _) = req(
        &router,
        "POST",
        &path,
        &admin,
        json!({"decision":"reject","reason":" Not eligible "}),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["rejection_reason"], "Not eligible");
    assert!(auth::create_session(&state.db, id).await.is_err());
    assert_eq!(
        req(
            &router,
            "POST",
            "/api/auth/register",
            "",
            json!({"username":"eve","password":"test-password-123"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        req(
            &router,
            "POST",
            "/api/auth/register",
            "",
            json!({"username":"another","password":"test-password-123"})
        )
        .await
        .0,
        StatusCode::CREATED
    );
    assert_eq!(
        req(
            &router,
            "POST",
            "/api/auth/register",
            "",
            json!({"username":"sixth","password":"test-password-123"})
        )
        .await
        .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        req(
            &router,
            "GET",
            "/api/auth/registrations?limit=0",
            &admin,
            Value::Null
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (s, v, _) = req(
        &router,
        "GET",
        "/api/auth/registrations?status=rejected&limit=1",
        &admin,
        Value::Null,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["items"][0]["user_id"], id);
}
#[tokio::test]
async fn upgrading_existing_accounts_preserves_identity_and_activation() {
    let dir = tempfile::TempDir::new().unwrap();
    let old = dir.path().join("old-migrations");
    std::fs::create_dir(&old).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut files = std::fs::read_dir(source)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    files.sort();
    files.pop();
    for file in files {
        std::fs::copy(&file, old.join(file.file_name().unwrap())).unwrap();
    }
    let filename = dir.path().join("existing.db");
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&filename)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::migrate::Migrator::new(old.as_path())
        .await
        .unwrap()
        .run(&db)
        .await
        .unwrap();
    let hash = auth::hash_password("test-password-123".into())
        .await
        .unwrap();
    sqlx::query("INSERT INTO users(id,username,display_name,password_hash,role,created_at)VALUES(42,'legacy','Legacy',?,'admin','2026-01-01T00:00:00Z')").bind(hash).execute(&db).await.unwrap();
    db.close().await;
    let mut c = Config::from_env().unwrap();
    c.database_path = filename;
    c.static_dir = dir.path().join("web");
    c.public_origin = Some("http://127.0.0.1:9999".into());
    c.public_url = Some("http://127.0.0.1:9999".into());
    let state = build_state(c).await.unwrap();
    let row: (i64, String, bool) = sqlx::query_as(
        "SELECT id,account_status,password_login_enabled FROM users WHERE username='legacy'",
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert_eq!(row, (42, "active".into(), true));
    assert!(auth::create_session(&state.db, 42).await.is_ok());
    let router = app(state);
    assert_eq!(
        req(
            &router,
            "POST",
            "/api/auth/login",
            "",
            json!({"username":"legacy","password":"test-password-123"})
        )
        .await
        .0,
        StatusCode::OK
    );
}
#[tokio::test]
async fn maximum_registration_credentials_can_log_out_and_log_back_in() {
    let (_dir, _state, router, admin) = fixture().await;
    assert_eq!(
        req(
            &router,
            "PATCH",
            "/api/auth/registration-policy",
            &admin,
            json!({"mode":"open"})
        )
        .await
        .0,
        StatusCode::OK
    );
    let username = "A".repeat(64);
    let password = "p".repeat(256);
    let display_name = "D".repeat(100);
    let (s, v, h) = req(
        &router,
        "POST",
        "/api/auth/register",
        "",
        json!({"username":username,"password":password,"display_name":display_name}),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    let cookie = h["set-cookie"].to_str().unwrap().split(';').next().unwrap();
    let (s, v, _) = req(
        &router,
        "PATCH",
        "/api/auth/me",
        cookie,
        json!({"display_name":display_name}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["display_name"], display_name);
    let (s, v, _) = req(
        &router,
        "PATCH",
        &format!("/api/users/{}", v["id"].as_i64().unwrap()),
        &admin,
        json!({"display_name":display_name}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");

    assert_eq!(
        req(&router, "POST", "/api/auth/logout", cookie, json!({}))
            .await
            .0,
        StatusCode::OK
    );
    let (s, v, h) = req(
        &router,
        "POST",
        "/api/auth/login",
        "",
        json!({"username":username,"password":password}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["username"], username.to_lowercase());
    assert!(h.contains_key("set-cookie"));
}
