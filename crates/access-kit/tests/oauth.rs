mod support;
use access_kit::*;
use axum::{
    body::{to_bytes, Body},
    http::{HeaderMap, Request, StatusCode},
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use url::Url;
async fn req(
    app: &Router,
    method: &str,
    path: &str,
    cookie: &str,
    content: &str,
    body: String,
) -> (StatusCode, HeaderMap, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("cookie", cookie)
                .header("content-type", content)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn client(app: &Router) -> String {
    let (s, _, v) = req(
        app,
        "POST",
        "/api/access/clients",
        "session=owner",
        "application/json",
        json!({"client_name":"test","redirect_uris":["http://127.0.0.1:7777/callback"]})
            .to_string(),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    v["client_id"].as_str().unwrap().to_owned()
}
async fn code(app: &Router, client: &str) -> String {
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest("a".repeat(43)));
    let mut url = Url::parse("http://127.0.0.1:9999/oauth/authorize").unwrap();
    url.query_pairs_mut().extend_pairs([
        ("client_id", client),
        ("redirect_uri", "http://127.0.0.1:7777/callback"),
        ("response_type", "code"),
        ("state", "same-state"),
        ("scope", "reports:read"),
        ("resource", "http://127.0.0.1:9999/api/v1"),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
    ]);
    let (s, h, v) = req(
        app,
        "GET",
        &format!("{}?{}", url.path(), url.query().unwrap()),
        "session=owner",
        "application/json",
        String::new(),
    )
    .await;
    assert_eq!(s, StatusCode::SEE_OTHER, "{v}");
    let location = Url::parse(&format!(
        "http://127.0.0.1:9999{}",
        h["location"].to_str().unwrap()
    ))
    .unwrap();
    let id = location
        .query_pairs()
        .find(|(k, _)| k == "request_id")
        .unwrap()
        .1
        .to_string();
    let browser = h["set-cookie"].to_str().unwrap().split(';').next().unwrap();
    let (s, h, v) = req(
        app,
        "POST",
        &format!("/oauth/consent/{id}"),
        &format!("session=owner; {browser}"),
        "application/x-www-form-urlencoded",
        "decision=approve".into(),
    )
    .await;
    assert_eq!(s, StatusCode::SEE_OTHER, "{v}");
    let target = Url::parse(h["location"].to_str().unwrap()).unwrap();
    assert!(target
        .query_pairs()
        .any(|(k, v)| k == "state" && v == "same-state"));
    target
        .query_pairs()
        .find(|(k, _)| k == "code")
        .unwrap()
        .1
        .to_string()
}
async fn token(app: &Router, params: &[(&str, &str)]) -> (StatusCode, Value) {
    let (s, h, v) = req(
        app,
        "POST",
        "/oauth/token",
        "",
        "application/x-www-form-urlencoded",
        serde_urlencoded::to_string(params).unwrap(),
    )
    .await;
    assert_eq!(
        h.get("cache-control").and_then(|v| v.to_str().ok()),
        Some("no-store")
    );
    (s, v)
}
#[tokio::test]
async fn pkce_code_single_use_refresh_rotation_and_family_reuse() {
    let db = support::database().await;
    let host = support::TestHost::new();
    let kit = AccessKit::new(
        db.clone(),
        AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
        host,
    )
    .await
    .unwrap();
    let app = kit.router();
    let client = client(&app).await;
    let code = code(&app, &client).await;
    let verifier = "a".repeat(43);
    let params = [
        ("grant_type", "authorization_code"),
        ("client_id", client.as_str()),
        ("code", code.as_str()),
        ("redirect_uri", "http://127.0.0.1:7777/callback"),
        ("resource", "http://127.0.0.1:9999/api/v1"),
        ("code_verifier", verifier.as_str()),
    ];
    let mut wrong = params;
    wrong[5].1 = "wrong";
    assert_eq!(token(&app, &wrong).await.0, StatusCode::BAD_REQUEST);
    let (s, t) = token(&app, &params).await;
    assert_eq!(s, StatusCode::OK, "{t}");
    assert_eq!(token(&app, &params).await.0, StatusCode::BAD_REQUEST);
    let old = t["refresh_token"].as_str().unwrap();
    let (s, t2) = token(
        &app,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", &client),
            ("refresh_token", old),
        ],
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{t2}");
    assert_ne!(old, t2["refresh_token"]);
    assert_eq!(
        token(
            &app,
            &[
                ("grant_type", "refresh_token"),
                ("client_id", &client),
                ("refresh_token", old)
            ]
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let grants: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM access_grants WHERE revoked_at IS NOT NULL")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(grants, 1);
    let secret_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM access_tokens WHERE token_hash=?")
            .bind(t2["access_token"].as_str().unwrap())
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(secret_count, 0);
}
#[tokio::test]
async fn unsafe_redirect_and_unregistered_error_never_redirect() {
    let db = support::database().await;
    let kit = AccessKit::new(
        db,
        AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
        support::TestHost::new(),
    )
    .await
    .unwrap();
    let app = kit.router();
    for uri in [
        "https://user@example.com/cb",
        "https://example.com/cb#fragment",
        "http://evil.example/cb",
        "https://*.example.com/cb",
    ] {
        let (s, _, _) = req(
            &app,
            "POST",
            "/api/access/clients",
            "session=owner",
            "application/json",
            json!({"client_name":"bad","redirect_uris":[uri]}).to_string(),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{uri}");
    }
    let (s, h, _) = req(
        &app,
        "GET",
        "/oauth/authorize?client_id=unknown&redirect_uri=https://evil.example/cb",
        "session=owner",
        "application/json",
        String::new(),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(!h.contains_key("location"));
}
#[tokio::test]
async fn code_expiry_concurrent_redemption_and_resource_binding() {
    let db = support::database().await;
    let kit = AccessKit::new(
        db.clone(),
        AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
        support::TestHost::new(),
    )
    .await
    .unwrap();
    let app = kit.router();
    let client = client(&app).await;
    let code = code(&app, &client).await;
    let verifier = "a".repeat(43);
    let mut p = [
        ("grant_type", "authorization_code"),
        ("client_id", client.as_str()),
        ("code", code.as_str()),
        ("redirect_uri", "http://127.0.0.1:7777/callback"),
        ("resource", "http://127.0.0.1:9999/mcp"),
        ("code_verifier", verifier.as_str()),
    ];
    assert_eq!(token(&app, &p).await.0, StatusCode::BAD_REQUEST);
    p[4].1 = "http://127.0.0.1:9999/api/v1";
    let (a, b) = tokio::join!(token(&app, &p), token(&app, &p));
    assert_eq!(
        [a.0, b.0].iter().filter(|s| **s == StatusCode::OK).count(),
        1
    );
    let response = if a.0 == StatusCode::OK { a.1 } else { b.1 };
    let refresh = response["refresh_token"].as_str().unwrap();
    assert_eq!(
        token(
            &app,
            &[
                ("grant_type", "refresh_token"),
                ("client_id", &client),
                ("refresh_token", refresh),
                ("scope", "reports:read people:read")
            ]
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        token(
            &app,
            &[
                ("grant_type", "refresh_token"),
                ("client_id", &client),
                ("refresh_token", refresh),
                ("resource", "http://127.0.0.1:9999/mcp")
            ]
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let c = crate::code(&app, &client).await;
    sqlx::query("UPDATE access_codes SET expires_at=0 WHERE used_at IS NULL")
        .execute(&db)
        .await
        .unwrap();
    p[2].1 = &c;
    assert_eq!(token(&app, &p).await.0, StatusCode::BAD_REQUEST);
}
#[tokio::test]
async fn foreign_client_cannot_revoke_other_grant_and_revocation_is_immediate() {
    let db = support::database().await;
    let kit = AccessKit::new(
        db.clone(),
        AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
        support::TestHost::new(),
    )
    .await
    .unwrap();
    let app = kit.router();
    let c1 = client(&app).await;
    let c2 = client(&app).await;
    let code = code(&app, &c1).await;
    let verifier = "a".repeat(43);
    let (_, tokens) = token(
        &app,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", &c1),
            ("code", &code),
            ("redirect_uri", "http://127.0.0.1:7777/callback"),
            ("resource", "http://127.0.0.1:9999/api/v1"),
            ("code_verifier", &verifier),
        ],
    )
    .await;
    let raw = tokens["access_token"].as_str().unwrap();
    for (client, expected) in [(&c2, 0i64), (&c1, 1i64)] {
        let (s, _, _) = req(
            &app,
            "POST",
            "/oauth/revoke",
            "",
            "application/x-www-form-urlencoded",
            serde_urlencoded::to_string([("client_id", client.as_str()), ("token", raw)]).unwrap(),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM access_grants WHERE revoked_at IS NOT NULL")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(count, expected);
    }
}

#[tokio::test]
async fn invalid_requests_for_a_known_client_preserve_valid_code_exchange() {
    let kit = AccessKit::new(
        support::database().await,
        AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
        support::TestHost::new(),
    )
    .await
    .unwrap();
    let app = kit.router();
    let client = client(&app).await;
    let code = code(&app, &client).await;
    for _ in 0..31 {
        assert_eq!(
            token(
                &app,
                &[
                    ("grant_type", "authorization_code"),
                    ("client_id", &client),
                    ("code", "invalid"),
                    ("redirect_uri", "http://127.0.0.1:7777/callback"),
                    ("resource", "http://127.0.0.1:9999/api/v1"),
                    ("code_verifier", &"a".repeat(43))
                ]
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        token(
            &app,
            &[
                ("grant_type", "authorization_code"),
                ("client_id", &client),
                ("code", &code),
                ("redirect_uri", "http://127.0.0.1:7777/callback"),
                ("resource", "http://127.0.0.1:9999/api/v1"),
                ("code_verifier", &"a".repeat(43))
            ]
        )
        .await
        .0,
        StatusCode::OK
    );
}
