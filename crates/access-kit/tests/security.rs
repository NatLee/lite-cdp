use access_kit::AccessConfig;
#[test]
fn canonical_origin_rejects_host_path_userinfo_fragment() {
    for value in [
        "http://remote.example",
        "https://user@safe.example",
        "https://safe.example/path",
        "https://safe.example/#fragment",
        "https://safe.example/?q=1",
    ] {
        assert!(AccessConfig::for_origin(value).is_err(), "{value}");
    }
    assert!(AccessConfig::for_origin("https://safe.example").is_ok());
    assert!(AccessConfig::for_origin("http://127.0.0.1:9999").is_ok());
}
mod support;
use access_kit::*;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use tower::ServiceExt;
async fn request(
    app: axum::Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    cookie: bool,
    body: Value,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    if cookie {
        req = req.header("cookie", "session=owner");
    }
    let response = app
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let b = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&b).unwrap_or(Value::Null))
}
#[tokio::test]
async fn key_audit_revoke_and_live_permission_contract() {
    let db = support::database().await;
    let host = support::TestHost::new();
    let kit = AccessKit::new(
        db.clone(),
        AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
        host.clone(),
    )
    .await
    .unwrap();
    let app = kit.router();
    let (status, key) = request(
        app.clone(),
        "POST",
        "/api/access/credentials",
        None,
        true,
        json!({"name":"test","scopes":["reports:read"]}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{key}");
    let token = key["token"].as_str().unwrap();
    let id = key["credential"]["id"].as_str().unwrap();
    let (_, keys) = request(
        app.clone(),
        "GET",
        "/api/access/credentials",
        None,
        true,
        Value::Null,
    )
    .await;
    assert!(!keys.to_string().contains(token));
    assert_eq!(
        request(
            app.clone(),
            "POST",
            "/api/v1/read_report",
            Some(token),
            false,
            json!({"value":7})
        )
        .await
        .0,
        StatusCode::OK
    );
    host.permission.store(false, Ordering::SeqCst);
    assert_eq!(
        request(
            app.clone(),
            "POST",
            "/api/v1/read_report",
            Some(token),
            false,
            json!({})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (_, usage) = request(
        app.clone(),
        "GET",
        "/api/access/usage?days=7&transport=api",
        None,
        true,
        Value::Null,
    )
    .await;
    assert_eq!(
        usage["totals"],
        json!({"attempts":2,"successes":1,"failures":1,"pending":0})
    );
    assert_eq!(
        request(
            app.clone(),
            "DELETE",
            &format!("/api/access/credentials/{id}"),
            None,
            true,
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(
            app.clone(),
            "POST",
            "/api/v1/read_report",
            Some(token),
            false,
            json!({})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(host.calls.load(Ordering::SeqCst), 2);
}
#[test]
fn empty_public_url_uses_literal_loopback_default() {
    let previous = std::env::var_os("ACCESS_PUBLIC_URL");
    std::env::set_var("ACCESS_PUBLIC_URL", "  ");
    let result = AccessConfig::from_env("http://127.0.0.1:9999");
    match previous {
        Some(v) => std::env::set_var("ACCESS_PUBLIC_URL", v),
        None => std::env::remove_var("ACCESS_PUBLIC_URL"),
    };
    assert!(result.is_ok());
}
async fn fixture() -> (
    sqlx::SqlitePool,
    std::sync::Arc<support::TestHost>,
    axum::Router,
) {
    let db = support::database().await;
    let host = support::TestHost::new();
    let kit = AccessKit::new(
        db.clone(),
        AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
        host.clone(),
    )
    .await
    .unwrap();
    (db, host, kit.router())
}
async fn key(app: &axum::Router, audiences: Value) -> Value {
    let (s, v) = request(
        app.clone(),
        "POST",
        "/api/access/credentials",
        None,
        true,
        json!({"name":"test","scopes":["reports:read"],"audiences":audiences}),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    v
}
#[tokio::test]
async fn wrong_audience_scope_expiry_disabled_and_cookie_boundaries() {
    let (db, host, app) = fixture().await;
    let k = key(&app, json!(["mcp"])).await;
    let t = k["token"].as_str().unwrap();
    assert_eq!(
        request(
            app.clone(),
            "POST",
            "/api/v1/read_report",
            Some(t),
            false,
            json!({})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let k = key(&app, json!(["api"])).await;
    let t = k["token"].as_str().unwrap();
    assert_eq!(
        request(
            app.clone(),
            "POST",
            "/api/v1/read_people",
            Some(t),
            false,
            json!({})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            app.clone(),
            "GET",
            "/api/access/credentials",
            Some(t),
            false,
            Value::Null
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            app.clone(),
            "POST",
            "/api/v1/read_report",
            None,
            true,
            json!({})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    host.active.store(false, Ordering::SeqCst);
    assert_eq!(
        request(
            app.clone(),
            "POST",
            "/api/v1/read_report",
            Some(t),
            false,
            json!({})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    host.active.store(true, Ordering::SeqCst);
    sqlx::query("UPDATE access_credentials SET expires_at=0")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        request(
            app.clone(),
            "POST",
            "/api/v1/read_report",
            Some(t),
            false,
            json!({})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(host.calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn invalid_scopes_audiences_expiry_and_cookie_csrf_are_rejected() {
    let (_, _, app) = fixture().await;
    for b in [
        json!({"name":"x","scopes":["admin:*"],"audiences":["api"]}),
        json!({"name":"x","scopes":["reports:read"],"audiences":[]}),
        json!({"name":"x","scopes":["reports:read"],"expires_in_days":91}),
    ] {
        assert_eq!(
            request(
                app.clone(),
                "POST",
                "/api/access/credentials",
                None,
                true,
                b
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    let r = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/access/credentials")
                .header("cookie", "session=owner")
                .header("origin", "https://evil.example")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name":"x","scopes":["reports:read"]}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}
#[tokio::test]
async fn audit_reserve_and_finalize_fail_closed_then_restart_recovers() {
    let (db, host, app) = fixture().await;
    let k = key(&app, json!(["api"])).await;
    let t = k["token"].as_str().unwrap();
    sqlx::query("CREATE TRIGGER fail_finalize BEFORE UPDATE OF outcome ON access_events WHEN NEW.outcome='success' BEGIN SELECT RAISE(FAIL,'test fault'); END").execute(&db).await.unwrap();
    assert_eq!(
        request(
            app.clone(),
            "POST",
            "/api/v1/read_report",
            Some(t),
            false,
            json!({})
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    let pending: i64 = sqlx::query_scalar("SELECT pending FROM access_daily")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(pending, 1);
    sqlx::query("DROP TRIGGER fail_finalize")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("UPDATE access_events SET at=at-601")
        .execute(&db)
        .await
        .unwrap();
    let kit = AccessKit::new(
        db.clone(),
        AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
        host.clone(),
    )
    .await
    .unwrap();
    let (_, usage) = request(
        kit.router(),
        "GET",
        "/api/access/usage",
        None,
        true,
        Value::Null,
    )
    .await;
    assert_eq!(
        usage["totals"],
        json!({"attempts":1,"successes":0,"failures":1,"pending":0})
    );
    sqlx::query("DROP TABLE access_daily")
        .execute(&db)
        .await
        .unwrap();
    let calls = host.calls.load(Ordering::SeqCst);
    assert_eq!(
        request(
            app,
            "POST",
            "/api/v1/read_report",
            Some(t),
            false,
            json!({})
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(host.calls.load(Ordering::SeqCst), calls);
}
#[tokio::test]
async fn admin_only_invalid_token_events_and_foreign_user_filters() {
    let (db, host, app) = fixture().await;
    let _ = request(
        app.clone(),
        "POST",
        "/api/v1/not_a_registered_secret",
        Some("invalid-secret-token"),
        false,
        json!({"private":"CANARY-PAYLOAD"}),
    )
    .await;
    host.admin.store(false, Ordering::SeqCst);
    assert_eq!(
        request(
            app.clone(),
            "GET",
            "/api/access/events?user_id=all",
            None,
            true,
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (_, own) = request(
        app.clone(),
        "GET",
        "/api/access/events",
        None,
        true,
        Value::Null,
    )
    .await;
    assert_eq!(own["items"], json!([]));
    host.admin.store(true, Ordering::SeqCst);
    let (_, events) = request(
        app.clone(),
        "GET",
        "/api/access/events?user_id=all",
        None,
        true,
        Value::Null,
    )
    .await;
    assert_eq!(events["items"].as_array().unwrap().len(), 1);
    let raw = events.to_string();
    assert!(
        !raw.contains("invalid-secret-token")
            && !raw.contains("CANARY-PAYLOAD")
            && !raw.contains("not_a_registered_secret")
    );
    let ops: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM access_daily")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(ops, 0);
}
#[tokio::test]
async fn malformed_authenticated_arguments_count_once_and_401_challenges() {
    let (_, _, app) = fixture().await;
    let k = key(&app, json!(["api"])).await;
    let token = k["token"].as_str().unwrap();
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/read_report")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from("{not json"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let (_, usage) = request(
        app.clone(),
        "GET",
        "/api/access/usage",
        None,
        true,
        Value::Null,
    )
    .await;
    assert_eq!(usage["totals"]["attempts"], 1);
    assert_eq!(usage["totals"]["failures"], 1);
    let r = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/read_report")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    assert!(r.headers().get("www-authenticate").is_some());
}
#[tokio::test]
async fn interrupted_call_recovery_and_utc_zero_days() {
    let (db, host, app) = fixture().await;
    let k = key(&app, json!(["api"])).await;
    let token = k["token"].as_str().unwrap().to_owned();
    host.delay_ms.store(10_000, Ordering::SeqCst);
    let clone = app.clone();
    let task = tokio::spawn(async move {
        request(
            clone,
            "POST",
            "/api/v1/read_report",
            Some(&token),
            false,
            json!({}),
        )
        .await
    });
    while host.calls.load(Ordering::SeqCst) == 0 {
        tokio::task::yield_now().await;
    }
    task.abort();
    let _ = task.await;
    sqlx::query("UPDATE access_events SET at=at-601")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("UPDATE access_daily SET day=(SELECT strftime('%Y-%m-%d',at,'unixepoch') FROM access_events LIMIT 1)").execute(&db).await.unwrap();
    let kit = AccessKit::new(
        db,
        AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
        host,
    )
    .await
    .unwrap();
    let (_, v) = request(
        kit.router(),
        "GET",
        "/api/access/usage?days=30",
        None,
        true,
        Value::Null,
    )
    .await;
    assert_eq!(v["daily"].as_array().unwrap().len(), 30);
    assert_eq!(v["timezone"], "UTC");
    assert_eq!(
        v["totals"],
        json!({"attempts":1,"successes":0,"failures":1,"pending":0})
    );
}
#[tokio::test]
async fn invalid_registries_are_startup_errors() {
    for variant in [1, 2, 3] {
        let db = support::database().await;
        let host = support::TestHost::new();
        host.variant.store(variant, Ordering::SeqCst);
        assert!(AccessKit::new(
            db,
            AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
            host
        )
        .await
        .is_err());
    }
}
