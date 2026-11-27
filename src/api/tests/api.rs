//! 整合測試：每個測試用一個全新的暫存資料庫，直接對 router 送請求。

use axum::{
    body::Body,
    http::{header, HeaderMap, Method, Request, StatusCode},
    Router,
};
use chrono::{Duration, Utc};
use http_body_util::BodyExt;
use lite_cdp::{app, build_state, db, demo, ingest, retention, AppState, Config};
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::ServiceExt;

const PASSWORD: &str = "test-password-123";
const CHROME: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
const IPHONE: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";

struct TestApp {
    router: Router,
    state: AppState,
    _dir: TempDir,
}

/// 某個人登入之後的 cookie。
#[derive(Clone)]
struct Session(String);

impl TestApp {
    async fn new() -> Self {
        Self::with_rate_limit(0).await
    }

    async fn with_rate_limit(rate_limit: u32) -> Self {
        let dir = TempDir::new().unwrap();
        let config = Config {
            bind: "127.0.0.1:0".into(),
            database_path: dir.path().join("test.db"),
            static_dir: dir.path().join("web"),
            public_url: Some("https://cdp.example.com".into()),
            public_origin: Some("https://cdp.example.com".into()),
            cookie_secure: false,
            trust_proxy: true,
            rate_limit,
            retention_days: 0,
        };
        let state = build_state(config).await.unwrap();
        Self {
            router: app(state.clone()),
            state,
            _dir: dir,
        }
    }

    async fn send(&self, req: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
        let resp = self.router.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = resp
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec();
        (status, headers, bytes)
    }

    async fn call(
        &self,
        session: Option<&Session>,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(path);
        if let Some(Session(cookie)) = session {
            req = req.header(header::COOKIE, cookie);
        }
        let req = match body {
            Some(b) => req
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(b.to_string()))
                .unwrap(),
            None => req.body(Body::empty()).unwrap(),
        };
        let (status, _, bytes) = self.send(req).await;
        let value = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
        (status, value)
    }

    /// 送出登入或設定請求，回傳 cookie。
    async fn sign_in(&self, path: &str, body: Value) -> Session {
        let req = Request::builder()
            .method(Method::POST)
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let (status, headers, bytes) = self.send(req).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(&bytes)
        );
        let cookie = headers[header::SET_COOKIE].to_str().unwrap();
        assert!(cookie.starts_with("cdp_session=") && cookie.contains("HttpOnly"));
        Session(cookie.split(';').next().unwrap().to_owned())
    }

    /// 建立管理員 `admin` 並登入。
    async fn admin(&self) -> Session {
        self.sign_in(
            "/api/auth/setup",
            json!({ "username": "admin", "display_name": "管理員", "password": PASSWORD }),
        )
        .await
    }

    /// 由管理員建立一個成員並登入，回傳（編號、session）。
    async fn member(&self, admin: &Session, username: &str) -> (i64, Session) {
        let (status, user) = self
            .call(
                Some(admin),
                Method::POST,
                "/api/users",
                Some(json!({ "username": username, "password": PASSWORD })),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{user}");
        let session = self
            .sign_in(
                "/api/auth/login",
                json!({ "username": username, "password": PASSWORD }),
            )
            .await;
        (user["id"].as_i64().unwrap(), session)
    }

    async fn get(&self, session: &Session, path: &str) -> Value {
        let (status, value) = self.call(Some(session), Method::GET, path, None).await;
        assert_eq!(status, StatusCode::OK, "{path}: {value}");
        value
    }

    async fn ok(&self, session: &Session, method: Method, path: &str, body: Value) -> Value {
        let (status, value) = self.call(Some(session), method, path, Some(body)).await;
        assert_eq!(status, StatusCode::OK, "{path}: {value}");
        value
    }

    /// 建立一個網站，回傳（編號、追蹤金鑰）。
    async fn site(&self, admin: &Session, body: Value) -> (i64, String) {
        let site = self.ok(admin, Method::POST, "/api/sites", body).await;
        (
            site["id"].as_i64().unwrap(),
            site["key"].as_str().unwrap().to_owned(),
        )
    }

    /// 像追蹤程式碼那樣送資料進來：text/plain、帶瀏覽器的 User-Agent。
    async fn collect_as(
        &self,
        body: &Value,
        user_agent: &str,
        origin: Option<&str>,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder()
            .method(Method::POST)
            .uri("/collect")
            .header(header::CONTENT_TYPE, "text/plain;charset=UTF-8")
            .header(header::USER_AGENT, user_agent)
            .header("x-forwarded-for", "203.0.113.9");
        if let Some(origin) = origin {
            req = req.header(header::ORIGIN, origin);
        }
        let (status, headers, bytes) = self
            .send(req.body(Body::from(body.to_string())).unwrap())
            .await;
        assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn collect(&self, body: Value) {
        let (status, error) = self.collect_as(&body, CHROME, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{error}");
    }

    async fn count(&self, table: &str) -> i64 {
        sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&self.state.db)
            .await
            .unwrap()
    }

    /// 直接寫一筆過去的瀏覽紀錄（`days_ago` 天前）。
    async fn past_view(&self, site_id: i64, anon_id: &str, path: &str, days_ago: i64) {
        let batch = serde_json::from_value::<ingest::Payload>(json!({
            "key": "-",
            "anon_id": anon_id,
            "events": [view(path)],
        }))
        .unwrap()
        .clean()
        .unwrap();
        let at = db::format_time(Utc::now() - Duration::days(days_ago));
        ingest::store(&self.state.db, site_id, &batch, CHROME, &at)
            .await
            .unwrap();
    }
}

fn view(path: &str) -> Value {
    json!({ "type": "pageview", "url": format!("https://www.example.com{path}"), "title": format!("標題 {path}") })
}

fn view_from(path: &str, referrer: &str) -> Value {
    json!({ "type": "pageview", "url": format!("https://www.example.com{path}"), "referrer": referrer })
}

fn custom(name: &str) -> Value {
    json!({ "type": "custom", "name": name, "url": "https://www.example.com/cart", "props": { "sku": "A-001" } })
}

fn batch(key: &str, anon_id: &str, events: Vec<Value>) -> Value {
    json!({ "key": key, "anon_id": anon_id, "session_id": "s1", "events": events })
}

fn identify(key: &str, anon_id: &str, who: Value) -> Value {
    json!({ "key": key, "anon_id": anon_id, "identify": who })
}

fn emails(list: &Value) -> Vec<&str> {
    list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["email"].as_str().unwrap())
        .collect()
}

fn names(list: &Value) -> Vec<&str> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn collection_policy_is_persisted_public_and_enforced() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (id, key) = t.site(&admin, json!({ "name": "policy", "collection_policy": {
        "consent_required": true, "auto_click": false, "query_mode": "allowlist",
        "query_allowlist": ["campaign"], "referrer_scope": "origin", "allowed_traits": ["plan"]
    }})).await;
    let site = t.get(&admin, &format!("/api/sites/{id}")).await;
    assert_eq!(site["collection_policy"]["consent_required"], true);
    let (status, config) = t
        .call(None, Method::GET, &format!("/tracker-config/{key}"), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(config["policy"]["auto_click"], false);
    assert!(config.get("identity_secret").is_none());
    let mut payload = batch(
        &key,
        "policy-anon-001",
        vec![view("/?email=private&campaign=spring#secret")],
    );
    assert_eq!(
        t.collect_as(&payload, CHROME, None).await.0,
        StatusCode::FORBIDDEN
    );
    payload["consent"] = json!(true);
    payload["identify"] =
        json!({ "user_id": "customer-1", "traits": { "plan": "pro", "secret": "omit" } });
    t.collect(payload).await;
    let (url,): (String,) = sqlx::query_as("SELECT url FROM events WHERE type='pageview'")
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert_eq!(url, "https://www.example.com/?campaign=spring");
    let traits: String = sqlx::query_scalar("SELECT traits FROM people")
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&traits).unwrap(),
        json!({"plan":"pro"})
    );
}

#[tokio::test]
async fn stable_user_ids_merge_devices_but_never_distinct_subjects_by_email() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (id, key) = t.site(&admin, json!({ "name": "subjects" })).await;
    for (anon, user) in [
        ("browser-one", "member-1"),
        ("browser-two", "member-2"),
        ("browser-three", "member-1"),
    ] {
        let mut payload = batch(&key, anon, vec![view("/")]);
        payload["identify"] = json!({"user_id":user,"email":"shared@example.com"});
        t.collect(payload).await;
    }
    let people = t
        .get(&admin, &format!("/api/sites/{id}/people?kind=member"))
        .await;
    assert_eq!(people["total"], 2);
    assert_eq!(t.count("devices").await, 3);
    let one = people["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["user_id"] == "member-1")
        .unwrap();
    assert_eq!(one["pageviews"], 2);
    let found = t
        .get(&admin, &format!("/api/sites/{id}/people?q=member-1"))
        .await;
    assert_eq!(found["total"], 1);
}

#[tokio::test]
async fn replayed_event_ids_do_not_inflate_events_labels_or_pageviews() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (id, key) = t.site(&admin, json!({ "name": "replay" })).await;
    t.ok(
        &admin,
        Method::POST,
        &format!("/api/sites/{id}/labels"),
        json!({"name":"Pricing","rules":[{"kind":"path_prefix","value":"/pricing"}]}),
    )
    .await;
    let mut event = view("/pricing");
    event["event_id"] = json!("event-unique-123");
    event["happened_at"] = json!(db::now());
    let payload = batch(&key, "browser-replay", vec![event]);
    t.collect(payload.clone()).await;
    t.collect(payload).await;
    assert_eq!(t.count("events").await, 1);
    let views: i64 = sqlx::query_scalar("SELECT pageviews FROM people")
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert_eq!(views, 1);
    let hits: i64 = sqlx::query_scalar("SELECT hits FROM person_labels")
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert_eq!(hits, 1);
}

#[tokio::test]
async fn installation_receipts_are_site_scoped_and_never_enter_analytics() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (id, key) = t.site(&admin, json!({ "name": "install" })).await;
    let test = t
        .ok(
            &admin,
            Method::POST,
            &format!("/api/sites/{id}/installation-tests"),
            json!({}),
        )
        .await;
    let token = test["token"].as_str().unwrap();
    let mut payload = batch(&key, "browser-install", vec![view("/")]);
    payload["test_token"] = json!(token);
    t.collect(payload).await;
    assert_eq!(t.count("events").await, 0);
    assert_eq!(t.count("people").await, 0);
    let receipt = t
        .get(
            &admin,
            &format!("/api/sites/{id}/installation-tests/{token}"),
        )
        .await;
    assert_eq!(receipt["receipt"]["events"][0]["type"], "pageview");
    let (_, member) = t.member(&admin, "outsider").await;
    assert_eq!(
        t.call(
            Some(&member),
            Method::GET,
            &format!("/api/sites/{id}/installation-tests/{token}"),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn first_visitor_creates_the_admin_and_there_is_no_default_account() {
    let t = TestApp::new().await;
    let (status, state) = t.call(None, Method::GET, "/api/auth/state", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(state, json!({ "setup_required": true, "user": null }));

    // 還沒登入：什麼都讀不到
    for path in [
        "/api/sites",
        "/api/users",
        "/api/sites/1/people",
        "/api/sites/1/overview",
    ] {
        let (status, _) = t.call(None, Method::GET, path, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
    }
    let (status, _) = t
        .call(
            None,
            Method::POST,
            "/api/auth/login",
            Some(json!({ "username": "admin", "password": "admin" })),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, body) = t
        .call(
            None,
            Method::POST,
            "/api/auth/setup",
            Some(json!({ "username": "admin", "password": "short" })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let admin = t.admin().await;
    let state = t.get(&admin, "/api/auth/state").await;
    assert_eq!(state["setup_required"], false);
    assert_eq!(state["user"]["role"], "admin");
    assert_eq!(state["user"]["display_name"], "管理員");

    // 設定只能做一次
    let (status, _) = t
        .call(
            None,
            Method::POST,
            "/api/auth/setup",
            Some(json!({ "username": "other", "password": PASSWORD })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let me = t
        .ok(
            &admin,
            Method::PATCH,
            "/api/auth/me",
            json!({ "display_name": " 王小明 " }),
        )
        .await;
    assert_eq!(me["display_name"], "王小明");

    // 登出之後 cookie 失效
    let (status, _) = t
        .call(Some(&admin), Method::POST, "/api/auth/logout", None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = t.call(Some(&admin), Method::GET, "/api/sites", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // 密碼沒有被存成明文
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users")
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert!(hash.starts_with("$argon2") && !hash.contains(PASSWORD));
}

#[tokio::test]
async fn api_writes_from_another_site_are_rejected_but_tracking_is_not() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let request = |origin: &'static str| {
        Request::builder()
            .method(Method::POST)
            .uri("/api/sites")
            .header(header::HOST, "localhost:8585")
            .header(header::ORIGIN, origin)
            .header(header::COOKIE, &admin.0)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "name": origin }).to_string()))
            .unwrap()
    };
    let (status, _, _) = t.send(request("https://evil.example")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = t.send(request("http://localhost:8585")).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = t.send(request("https://cdp.example.com")).await;
    assert_eq!(status, StatusCode::OK);

    // 追蹤資料本來就是從別的網站送過來的
    let (_, key) = t.site(&admin, json!({ "name": "官網" })).await;
    let (status, _) = t
        .collect_as(
            &batch(&key, "anon-00000001", vec![view("/")]),
            CHROME,
            Some("https://www.example.com"),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let req = Request::builder()
        .method(Method::OPTIONS)
        .uri("/collect")
        .header(header::ORIGIN, "https://www.example.com")
        .header("access-control-request-method", "POST")
        .body(Body::empty())
        .unwrap();
    let (status, headers, _) = t.send(req).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
    assert!(headers[header::ACCESS_CONTROL_ALLOW_METHODS]
        .to_str()
        .unwrap()
        .contains("POST"));
}

#[tokio::test]
async fn the_tracker_script_is_served() {
    let t = TestApp::new().await;
    let req = Request::builder().uri("/t.js").body(Body::empty()).unwrap();
    let (status, headers, bytes) = t.send(req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .starts_with("application/javascript"));
    assert!(headers[header::CACHE_CONTROL]
        .to_str()
        .unwrap()
        .contains("max-age"));
    let script = String::from_utf8(bytes).unwrap();
    assert!(script.contains("cdp_anon_id") && script.contains("/collect"));
}

#[tokio::test]
async fn tracking_data_is_validated_before_it_is_stored() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (site_id, key) = t.site(&admin, json!({ "name": "官網" })).await;

    let (status, body) = t
        .collect_as(
            &batch("nope", "anon-00000001", vec![view("/")]),
            CHROME,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");

    let bad = [
        batch(&key, "short", vec![view("/")]),
        batch(&key, "anon-00000001", vec![]),
        batch(
            &key,
            "anon-00000001",
            vec![json!({ "type": "pageview", "url": "/pricing" })],
        ),
        batch(
            &key,
            "anon-00000001",
            vec![json!({ "type": "purchase", "name": "x" })],
        ),
        batch(&key, "anon-00000001", vec![view("/"); 51]),
        identify(&key, "anon-00000001", json!({ "email": "not-an-email" })),
    ];
    for body in bad {
        let (status, error) = t.collect_as(&body, CHROME, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(error["error"].is_string());
    }
    let req = Request::builder()
        .method(Method::POST)
        .uri("/collect")
        .body(Body::from("not json"))
        .unwrap();
    assert_eq!(t.send(req).await.0, StatusCode::BAD_REQUEST);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/collect")
        .body(Body::from(
            batch(
                &key,
                "anon-00000001",
                vec![
                    json!({ "type": "custom", "name": "x", "props": { "a": "x".repeat(70_000) } }),
                ],
            )
            .to_string(),
        ))
        .unwrap();
    assert_eq!(t.send(req).await.0, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(t.count("events").await, 0);
    assert_eq!(t.count("people").await, 0);

    // 爬蟲：回成功，但不存
    let (status, _) = t
        .collect_as(
            &batch(&key, "anon-00000001", vec![view("/")]),
            "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(t.count("events").await, 0);

    t.collect(batch(
        &key,
        "anon-00000001",
        vec![
            view_from("/pricing/?plan=team#faq", "https://www.google.com/search?q=cdp"),
            json!({ "type": "scroll", "name": "50%", "url": "https://www.example.com/pricing/", "props": { "depth": 50 } }),
            custom("add_to_cart"),
        ],
    ))
    .await;
    let people = t.get(&admin, &format!("/api/sites/{site_id}/people")).await;
    assert_eq!(people["total"], 1);
    let person = &people["items"][0];
    assert_eq!(person["email"], "");
    assert_eq!(person["pageviews"], 1);
    let pid = person["id"].as_i64().unwrap();

    let events = t
        .get(&admin, &format!("/api/sites/{site_id}/people/{pid}/events"))
        .await;
    let items = events["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    // 從最新的開始
    assert_eq!(items[0]["type"], "custom");
    assert_eq!(items[0]["props"], json!({ "sku": "A-001" }));
    assert_eq!(items[2]["type"], "pageview");
    assert_eq!(items[2]["path"], "/pricing");
    assert_eq!(items[2]["referrer_host"], "google.com");
    assert_eq!(events["next_before"], Value::Null);

    // IP 沒有被存在任何地方
    for table in ["events", "devices", "people"] {
        let columns: Vec<String> =
            sqlx::query_scalar(&format!("SELECT name FROM pragma_table_info('{table}')"))
                .fetch_all(&t.state.db)
                .await
                .unwrap();
        assert!(!columns.iter().any(|c| c.contains("ip")), "{table}");
    }

    // 停收之後不再收
    t.ok(
        &admin,
        Method::PATCH,
        &format!("/api/sites/{site_id}"),
        json!({ "active": false }),
    )
    .await;
    let (status, _) = t
        .collect_as(&batch(&key, "anon-00000001", vec![view("/")]), CHROME, None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // 換金鑰之後舊的失效
    t.ok(
        &admin,
        Method::PATCH,
        &format!("/api/sites/{site_id}"),
        json!({ "active": true }),
    )
    .await;
    let rotated = t
        .ok(
            &admin,
            Method::POST,
            &format!("/api/sites/{site_id}/rotate-key"),
            json!({}),
        )
        .await;
    assert_ne!(rotated["key"], key);
    let (status, _) = t
        .collect_as(&batch(&key, "anon-00000001", vec![view("/")]), CHROME, None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn allowed_origins_limit_which_sites_can_send_data() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (site_id, key) = t
        .site(
            &admin,
            json!({ "name": "官網", "allowed_origins": ["https://www.example.com/contact"] }),
        )
        .await;
    let site = t.get(&admin, &format!("/api/sites/{site_id}")).await;
    assert_eq!(site["allowed_origins"], json!(["https://www.example.com"]));

    let body = batch(&key, "anon-00000001", vec![view("/")]);
    let (status, _) = t
        .collect_as(&body, CHROME, Some("https://www.example.com"))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, error) = t
        .collect_as(&body, CHROME, Some("https://evil.example"))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{error}");
    // 沒有 Origin 的不是瀏覽器的跨站請求
    let (status, _) = t.collect_as(&body, CHROME, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(t.count("events").await, 2);

    let (status, _) = t
        .call(
            Some(&admin),
            Method::PATCH,
            &format!("/api/sites/{site_id}"),
            Some(json!({ "allowed_origins": ["www.example.com"] })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn tracking_is_rate_limited_per_address() {
    let t = TestApp::with_rate_limit(3).await;
    let admin = t.admin().await;
    let (_, key) = t.site(&admin, json!({ "name": "官網" })).await;
    let body = batch(&key, "anon-00000001", vec![view("/")]);
    for _ in 0..3 {
        assert_eq!(
            t.collect_as(&body, CHROME, None).await.0,
            StatusCode::NO_CONTENT
        );
    }
    let (status, error) = t.collect_as(&body, CHROME, None).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{error}");
    assert_eq!(t.count("events").await, 3);
}

#[tokio::test]
async fn visitors_become_known_people_when_identified() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (site_id, key) = t.site(&admin, json!({ "name": "官網" })).await;
    let people = |query: &'static str| {
        let path = format!("/api/sites/{site_id}/people{query}");
        let (t, admin) = (&t, &admin);
        async move { t.get(admin, &path).await }
    };

    // 筆電：先匿名逛了兩頁
    t.collect(batch(
        &key,
        "laptop-0001",
        vec![view("/"), view("/pricing")],
    ))
    .await;
    assert_eq!(people("?kind=anonymous").await["total"], 1);
    assert_eq!(people("?kind=member").await["total"], 0);

    // 登入：同一個人變成知道是誰
    let mut body = batch(&key, "laptop-0001", vec![view("/account")]);
    body["identify"] = json!({
        "email": " Amy@Example.com ",
        "name": "陳雅婷",
        "account": "M-001",
        "traits": { "plan": "team", "seats": 5, "nested": { "x": 1 } },
    });
    t.collect(body.clone()).await;
    let members = people("?kind=member").await;
    assert_eq!(members["total"], 1);
    assert_eq!(people("?kind=anonymous").await["total"], 0);
    let amy = &members["items"][0];
    assert_eq!(amy["email"], "amy@example.com");
    assert_eq!(amy["name"], "陳雅婷");
    assert_eq!(amy["pageviews"], 3);
    assert!(amy["identified_at"].is_string());
    let amy_id = amy["id"].as_i64().unwrap();

    // 每一頁都呼叫 identify 也只記一次「認出來」
    t.collect(body).await;
    let events = t
        .get(
            &admin,
            &format!("/api/sites/{site_id}/people/{amy_id}/events?type=identify"),
        )
        .await;
    assert_eq!(events["items"].as_array().unwrap().len(), 1);
    assert_eq!(events["items"][0]["name"], "amy@example.com");

    // 手機：匿名逛了一頁，之後登入同一個 email → 併成同一個人
    let req = batch(&key, "phone-00001", vec![view("/blog")]);
    assert_eq!(
        t.collect_as(&req, IPHONE, None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(people("").await["total"], 2);
    let req = identify(
        &key,
        "phone-00001",
        json!({ "email": "amy@example.com", "traits": { "plan": "business" } }),
    );
    assert_eq!(
        t.collect_as(&req, IPHONE, None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(people("").await["total"], 1);
    let detail = t
        .get(&admin, &format!("/api/sites/{site_id}/people/{amy_id}"))
        .await;
    assert_eq!(detail["pageviews"], 5);
    assert_eq!(detail["name"], "陳雅婷");
    assert_eq!(detail["account"], "M-001");
    // 特徵：新的蓋掉舊的，巢狀的不收
    assert_eq!(detail["traits"], json!({ "plan": "business", "seats": 5 }));
    let devices = detail["devices"].as_array().unwrap();
    assert_eq!(devices.len(), 2);
    let mut seen: Vec<(&str, &str)> = devices
        .iter()
        .map(|d| (d["browser"].as_str().unwrap(), d["os"].as_str().unwrap()))
        .collect();
    seen.sort_unstable();
    assert_eq!(seen, [("Chrome", "macOS"), ("Safari", "iOS")]);
    assert_eq!(detail["events"], 7);
    assert_eq!(detail["top_pages"][0]["path"], "/account");

    // 同一台筆電換另一個人登入（共用電腦）：之後算新的人，之前的不動
    let mut body = batch(&key, "laptop-0001", vec![view("/account")]);
    body["identify"] = json!({ "email": "ben@example.com" });
    t.collect(body).await;
    let all = people("?sort=first_seen").await;
    assert_eq!(all["total"], 2);
    let ben = all["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["email"] == "ben@example.com")
        .unwrap();
    assert_eq!(ben["pageviews"], 1);
    let detail = t
        .get(&admin, &format!("/api/sites/{site_id}/people/{amy_id}"))
        .await;
    assert_eq!(detail["pageviews"], 5);
    assert_eq!(detail["devices"].as_array().unwrap().len(), 1);

    // 搜尋 email、名稱、帳號
    assert_eq!(emails(&people("?q=AMY@").await), ["amy@example.com"]);
    assert_eq!(emails(&people("?q=雅婷").await), ["amy@example.com"]);
    assert_eq!(emails(&people("?q=m-001").await), ["amy@example.com"]);
    assert_eq!(people("?q=nobody").await["total"], 0);
    let (status, _) = t
        .call(
            Some(&admin),
            Method::GET,
            &format!("/api/sites/{site_id}/people?sort=email"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 同一個 email 在另一個網站是另一個人
    let (other_id, other_key) = t.site(&admin, json!({ "name": "商店" })).await;
    t.collect(identify(
        &other_key,
        "laptop-0001",
        json!({ "email": "amy@example.com" }),
    ))
    .await;
    let other = t
        .get(&admin, &format!("/api/sites/{other_id}/people"))
        .await;
    assert_eq!(other["total"], 1);
    assert_ne!(other["items"][0]["id"], amy_id);
    let (status, _) = t
        .call(
            Some(&admin),
            Method::GET,
            &format!("/api/sites/{other_id}/people/{amy_id}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn labels_follow_their_rules() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (site_id, key) = t.site(&admin, json!({ "name": "官網" })).await;
    let labels_path = format!("/api/sites/{site_id}/labels");

    // 先有一些事件，標籤是之後才建的
    t.collect(batch(
        &key,
        "reader-00001",
        vec![view("/blog"), view("/blog/hello"), view("/blogger")],
    ))
    .await;
    t.collect(batch(
        &key,
        "buyer-000001",
        vec![view("/blog"), custom("Add_To_Cart")],
    ))
    .await;

    let bad = [
        json!({ "name": "", "rules": [{ "kind": "path_prefix", "value": "/blog" }] }),
        json!({ "name": "讀者", "rules": [] }),
        json!({ "name": "讀者", "rules": [{ "kind": "path_prefix", "value": "blog" }] }),
        json!({ "name": "讀者", "rules": [{ "kind": "regex", "value": ".*" }] }),
        json!({ "name": "讀者", "color": "#ff0000", "rules": [{ "kind": "event", "value": "x" }] }),
        json!({ "name": "讀者", "min_hits": 0, "rules": [{ "kind": "event", "value": "x" }] }),
    ];
    for body in bad {
        let (status, _) = t
            .call(Some(&admin), Method::POST, &labels_path, Some(body.clone()))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }

    // 建立時會用現有的事件算一次
    let reader = t
        .ok(
            &admin,
            Method::POST,
            &labels_path,
            json!({
                "name": "部落格讀者",
                "color": "sky",
                "min_hits": 2,
                "rules": [{ "kind": "path_prefix", "value": "/blog/" }, { "kind": "path_prefix", "value": "/blog" }],
            }),
        )
        .await;
    assert_eq!(
        reader["rules"],
        json!([{ "kind": "path_prefix", "value": "/blog" }])
    );
    assert_eq!(reader["people"], 1);
    let reader_id = reader["id"].as_i64().unwrap();
    let (status, _) = t
        .call(
            Some(&admin),
            Method::POST,
            &labels_path,
            Some(json!({ "name": "部落格讀者", "rules": [{ "kind": "event", "value": "x" }] })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let cart = t
        .ok(
            &admin,
            Method::POST,
            &labels_path,
            json!({ "name": "加入購物車", "rules": [{ "kind": "event", "value": "add_to_cart" }] }),
        )
        .await;
    assert_eq!(cart["people"], 1);
    assert_eq!(cart["color"], "neutral");

    let people_with = |label_id: i64| {
        let path = format!("/api/sites/{site_id}/people?label_id={label_id}");
        let (t, admin) = (&t, &admin);
        async move { t.get(admin, &path).await }
    };
    let readers = people_with(reader_id).await;
    assert_eq!(readers["total"], 1);
    assert_eq!(names(&readers["items"][0]["labels"]), ["部落格讀者"]);

    // 之後進來的事件會跟著累計：buyer 第二次看部落格就有標籤了
    t.collect(batch(&key, "buyer-000001", vec![view("/blog/second")]))
        .await;
    let readers = people_with(reader_id).await;
    assert_eq!(readers["total"], 2);
    let buyer = readers["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["labels"].as_array().unwrap().len() == 2)
        .expect("buyer 兩個標籤都有");
    assert_eq!(names(&buyer["labels"]), ["加入購物車", "部落格讀者"]);
    let detail = t
        .get(
            &admin,
            &format!("/api/sites/{site_id}/people/{}", buyer["id"]),
        )
        .await;
    let on_person = detail["labels"].as_array().unwrap();
    assert_eq!(on_person.len(), 2);
    assert!(on_person
        .iter()
        .any(|l| l["name"] == "部落格讀者" && l["hits"] == 2 && l["min_hits"] == 2));

    // 門檻調高：不用重算，人數就跟著變
    let updated = t
        .ok(
            &admin,
            Method::PATCH,
            &format!("{labels_path}/{reader_id}"),
            json!({ "min_hits": 3 }),
        )
        .await;
    assert_eq!(updated["people"], 0);
    // 換規則：用現有的事件重算
    let updated = t
        .ok(
            &admin,
            Method::PATCH,
            &format!("{labels_path}/{reader_id}"),
            json!({ "min_hits": 1, "rules": [{ "kind": "title_contains", "value": "/BLOGGER" }] }),
        )
        .await;
    assert_eq!(updated["people"], 1);
    assert_eq!(updated["name"], "部落格讀者");

    // 頁面清單會標出每一頁符合哪些標籤
    let pages = t.get(&admin, &format!("/api/sites/{site_id}/pages")).await;
    let pages = pages.as_array().unwrap();
    assert_eq!(pages[0]["path"], "/blog");
    assert_eq!(pages[0]["pageviews"], 2);
    assert_eq!(pages[0]["visitors"], 2);
    let blogger = pages.iter().find(|p| p["path"] == "/blogger").unwrap();
    assert_eq!(names(&blogger["labels"]), ["部落格讀者"]);
    assert!(pages[0]["labels"].as_array().unwrap().is_empty());
    let found = t
        .get(&admin, &format!("/api/sites/{site_id}/pages?q=SECOND"))
        .await;
    assert_eq!(found.as_array().unwrap().len(), 1);

    // 有分群在用的標籤不能刪
    let segment = t
        .ok(
            &admin,
            Method::POST,
            &format!("/api/sites/{site_id}/segments"),
            json!({ "name": "讀者名單", "rules": { "must": [{ "field": "label", "value": reader_id }] } }),
        )
        .await;
    let (status, error) = t
        .call(
            Some(&admin),
            Method::DELETE,
            &format!("{labels_path}/{reader_id}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(error["error"].as_str().unwrap().contains("讀者名單"));
    let (status, _) = t
        .call(
            Some(&admin),
            Method::DELETE,
            &format!("/api/sites/{site_id}/segments/{}", segment["id"]),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = t
        .call(
            Some(&admin),
            Method::DELETE,
            &format!("{labels_path}/{reader_id}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&t.get(&admin, &labels_path).await), ["加入購物車"]);
    assert_eq!(t.count("person_labels").await, 1);
}

#[tokio::test]
async fn segments_combine_conditions() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (site_id, key) = t.site(&admin, json!({ "name": "官網" })).await;
    let base = format!("/api/sites/{site_id}");
    let label = |name: &'static str, path: &'static str| {
        let (t, admin, url) = (&t, &admin, format!("{base}/labels"));
        async move {
            t.ok(
                admin,
                Method::POST,
                &url,
                json!({ "name": name, "rules": [{ "kind": "path_prefix", "value": path }] }),
            )
            .await["id"]
                .as_i64()
                .unwrap()
        }
    };
    let pricing = label("看過定價", "/pricing").await;
    let enterprise = label("企業方案", "/enterprise").await;
    let careers = label("看過徵才", "/careers").await;

    // amy：會員，定價 + 企業，從 Google 來
    let mut body = batch(
        &key,
        "amy-00000001",
        vec![
            view_from("/pricing", "https://www.google.com/"),
            view("/enterprise"),
        ],
    );
    body["identify"] = json!({ "email": "amy@example.com" });
    t.collect(body).await;
    // ben：會員，定價 + 徵才
    let mut body = batch(
        &key,
        "ben-00000001",
        vec![view("/pricing"), view("/careers")],
    );
    body["identify"] = json!({ "email": "ben@corp.example.org" });
    t.collect(body).await;
    // 匿名：只看過定價，從 Google 來
    t.collect(batch(
        &key,
        "anon-0000001",
        vec![view_from("/pricing", "https://www.google.com.tw/search")],
    ))
    .await;
    // 很久沒來的會員
    t.past_view(site_id, "old-00000001", "/pricing", 40).await;
    t.collect(identify(
        &key,
        "old-00000001",
        json!({ "email": "old@example.com" }),
    ))
    .await;
    sqlx::query("UPDATE people SET last_seen = first_seen WHERE email = 'old@example.com'")
        .execute(&t.state.db)
        .await
        .unwrap();

    let preview = |body: Value| {
        let (t, admin, url) = (&t, &admin, format!("{base}/segments/preview"));
        async move { t.ok(admin, Method::POST, &url, body).await }
    };
    let must_pricing = json!([{ "field": "label", "value": pricing }]);

    let all = preview(json!({ "rules": { "must": must_pricing } })).await;
    assert_eq!(all["count"], 4);
    assert_eq!(all["total"], 4);
    assert_eq!(all["sample"].as_array().unwrap().len(), 4);
    assert_eq!(
        preview(json!({ "kind": "member", "rules": { "must": must_pricing } })).await["count"],
        3
    );
    assert_eq!(
        preview(json!({ "kind": "anonymous", "rules": { "must": must_pricing } })).await["count"],
        1
    );
    // should：至少符合一條
    let either = preview(json!({ "rules": {
        "must": must_pricing,
        "should": [{ "field": "label", "value": enterprise }, { "field": "label", "value": careers }],
    } }))
    .await;
    assert_eq!(either["count"], 2);
    // must_not
    let not_careers = preview(json!({ "kind": "member", "rules": {
        "must": must_pricing,
        "must_not": [{ "field": "label", "value": careers }],
    } }))
    .await;
    assert_eq!(not_careers["count"], 2);
    // 來源、email、最近來過、瀏覽頁數
    let google =
        preview(json!({ "rules": { "must": [{ "field": "referrer", "value": "Google" }] } })).await;
    assert_eq!(google["count"], 2);
    let corp =
        preview(json!({ "rules": { "must": [{ "field": "email", "value": "@CORP." }] } })).await;
    assert_eq!(
        emails(&json!({ "items": corp["sample"] })),
        ["ben@corp.example.org"]
    );
    let recent = preview(
        json!({ "kind": "member", "rules": { "must": [{ "field": "seen_within", "value": 30 }] } }),
    )
    .await;
    assert_eq!(recent["count"], 2);
    let dormant = preview(json!({ "kind": "member", "rules": { "must_not": [{ "field": "seen_within", "value": 30 }] } })).await;
    assert_eq!(
        emails(&json!({ "items": dormant["sample"] })),
        ["old@example.com"]
    );
    let new =
        preview(json!({ "rules": { "must": [{ "field": "first_seen_within", "value": 7 }] } }))
            .await;
    assert_eq!(new["count"], 3);
    let active =
        preview(json!({ "rules": { "must": [{ "field": "min_pageviews", "value": 2 }] } })).await;
    assert_eq!(active["count"], 2);

    // 不合法的條件
    let bad = [
        json!({ "rules": {} }),
        json!({ "kind": "vip", "rules": { "must": must_pricing } }),
        json!({ "rules": { "must": [{ "field": "label", "value": 9999 }] } }),
        json!({ "rules": { "must": [{ "field": "seen_within", "value": 0 }] } }),
    ];
    for body in bad {
        let (status, _) = t
            .call(
                Some(&admin),
                Method::POST,
                &format!("{base}/segments/preview"),
                Some(body.clone()),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
    let (status, _) = t
        .call(
            Some(&admin),
            Method::POST,
            &format!("{base}/segments/preview"),
            Some(json!({ "rules": { "must": [{ "field": "age", "value": 3 }] } })),
        )
        .await;
    assert!(status.is_client_error());

    // 存起來
    let saved = t
        .ok(
            &admin,
            Method::POST,
            &format!("{base}/segments"),
            json!({
                "name": "高意願名單",
                "description": "看過定價和企業方案的會員",
                "kind": "member",
                "rules": { "must": [{ "field": "label", "value": pricing }, { "field": "label", "value": enterprise }] },
            }),
        )
        .await;
    assert_eq!(saved["count"], 1);
    assert_eq!(saved["created_by_name"], "管理員");
    let sid = saved["id"].as_i64().unwrap();
    let (status, _) = t
        .call(
            Some(&admin),
            Method::POST,
            &format!("{base}/segments"),
            Some(json!({ "name": "高意願名單", "kind": "member", "rules": {} })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    // 只限會員的分群可以沒有其他條件
    let members = t
        .ok(
            &admin,
            Method::POST,
            &format!("{base}/segments"),
            json!({ "name": "所有會員", "kind": "member", "rules": {} }),
        )
        .await;
    assert_eq!(members["count"], 3);

    // 名單每次打開時現算：ben 也看了企業方案之後就進來了
    let in_segment = t
        .get(&admin, &format!("{base}/people?segment_id={sid}"))
        .await;
    assert_eq!(emails(&in_segment), ["amy@example.com"]);
    t.collect(batch(
        &key,
        "ben-00000001",
        vec![view("/enterprise/contact")],
    ))
    .await;
    assert_eq!(
        t.get(&admin, &format!("{base}/segments/{sid}")).await["count"],
        2
    );
    // 分群裡再篩一次
    let narrowed = t
        .get(&admin, &format!("{base}/people?segment_id={sid}&q=corp"))
        .await;
    assert_eq!(emails(&narrowed), ["ben@corp.example.org"]);

    let updated = t
        .ok(
            &admin,
            Method::PATCH,
            &format!("{base}/segments/{sid}"),
            json!({ "name": "企業名單", "kind": "all" }),
        )
        .await;
    assert_eq!(updated["name"], "企業名單");
    assert_eq!(updated["rules"]["must"].as_array().unwrap().len(), 2);
    assert_eq!(
        names(&t.get(&admin, &format!("{base}/segments")).await),
        ["企業名單", "所有會員"]
    );
    let (status, _) = t
        .call(
            Some(&admin),
            Method::GET,
            &format!("{base}/people?segment_id=9999"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn overview_summarises_recent_activity() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (site_id, key) = t.site(&admin, json!({ "name": "官網" })).await;
    t.ok(
        &admin,
        Method::POST,
        &format!("/api/sites/{site_id}/labels"),
        json!({ "name": "看過定價", "color": "amber", "rules": [{ "kind": "path_prefix", "value": "/pricing" }] }),
    )
    .await;

    let empty = t
        .get(&admin, &format!("/api/sites/{site_id}/overview"))
        .await;
    assert_eq!(empty["totals"]["people"], 0);
    assert_eq!(empty["daily"].as_array().unwrap().len(), 30);
    assert!(empty["top_pages"].as_array().unwrap().is_empty());

    let mut body = batch(
        &key,
        "amy-00000001",
        vec![
            view_from("/", "https://www.google.com/"),
            view("/pricing"),
            custom("signup"),
        ],
    );
    body["identify"] = json!({ "email": "amy@example.com", "name": "陳雅婷" });
    t.collect(body).await;
    t.collect(batch(
        &key,
        "anon-0000001",
        vec![view_from("/pricing", "https://news.example.org/a")],
    ))
    .await;
    t.past_view(site_id, "old-00000001", "/pricing", 10).await;
    t.past_view(site_id, "older-0000001", "/old-page", 45).await;

    let o = t
        .get(
            &admin,
            &format!("/api/sites/{site_id}/overview?tz_offset=480"),
        )
        .await;
    assert_eq!(
        o["totals"],
        json!({ "people": 4, "identified": 1, "anonymous": 3 })
    );
    assert_eq!(
        o["last_7_days"],
        json!({ "visitors": 2, "pageviews": 3, "new_people": 2, "identified": 1 })
    );
    assert_eq!(
        o["previous_7_days"],
        json!({ "visitors": 1, "pageviews": 1, "new_people": 1, "identified": 0 })
    );
    let daily = o["daily"].as_array().unwrap();
    assert_eq!(daily.len(), 30);
    assert_eq!(daily[29]["pageviews"], 3);
    assert_eq!(daily[29]["visitors"], 2);
    let total: i64 = daily.iter().map(|d| d["pageviews"].as_i64().unwrap()).sum();
    assert_eq!(total, 4, "45 天前的不算");
    assert_eq!(
        o["top_pages"][0],
        json!({ "path": "/pricing", "title": "標題 /pricing", "pageviews": 3, "visitors": 3 })
    );
    let hosts: Vec<&str> = o["top_referrers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["host"].as_str().unwrap())
        .collect();
    assert_eq!(hosts, ["google.com", "news.example.org"]);
    assert_eq!(o["labels"][0]["name"], "看過定價");
    assert_eq!(o["labels"][0]["people"], 3);
    assert_eq!(o["recent_identified"][0]["email"], "amy@example.com");

    let sites = t.get(&admin, "/api/sites").await;
    assert_eq!(sites[0]["people"], 4);
    assert_eq!(sites[0]["identified"], 1);
    assert!(sites[0]["last_event_at"].is_string());
}

#[tokio::test]
async fn timelines_are_paged_and_people_can_be_exported_and_deleted() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (amy_id, amy) = t.member(&admin, "amy").await;
    let (site_id, key) = t
        .site(&admin, json!({ "name": "官網", "member_ids": [amy_id] }))
        .await;
    t.ok(
        &admin,
        Method::POST,
        &format!("/api/sites/{site_id}/labels"),
        json!({ "name": "看過定價", "rules": [{ "kind": "path_prefix", "value": "/pricing" }] }),
    )
    .await;

    let mut body = batch(&key, "formula-0001", vec![view("/pricing")]);
    body["identify"] =
        json!({ "email": "eve@example.com", "name": "=HYPERLINK(\"https://evil.example\")" });
    t.collect(body).await;
    for i in 0..7 {
        t.collect(batch(
            &key,
            "reader-00001",
            vec![view(&format!("/page-{i}"))],
        ))
        .await;
    }

    let people = t
        .get(
            &amy,
            &format!("/api/sites/{site_id}/people?sort=pageviews&per_page=1"),
        )
        .await;
    assert_eq!(people["total"], 2);
    assert_eq!(people["items"].as_array().unwrap().len(), 1);
    assert_eq!(people["items"][0]["pageviews"], 7);
    let reader = people["items"][0]["id"].as_i64().unwrap();
    let page2 = t
        .get(
            &amy,
            &format!("/api/sites/{site_id}/people?sort=pageviews&per_page=1&page=2"),
        )
        .await;
    assert_eq!(emails(&page2), ["eve@example.com"]);
    let eve = page2["items"][0]["id"].as_i64().unwrap();

    // 時間軸一次拿 3 筆，接著往前翻
    let events_path = format!("/api/sites/{site_id}/people/{reader}/events");
    let first = t.get(&amy, &format!("{events_path}?limit=3")).await;
    let paths: Vec<&str> = first["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, ["/page-6", "/page-5", "/page-4"]);
    let before = first["next_before"].as_i64().unwrap();
    let rest = t
        .get(&amy, &format!("{events_path}?limit=10&before={before}"))
        .await;
    assert_eq!(rest["items"].as_array().unwrap().len(), 4);
    assert_eq!(rest["next_before"], Value::Null);
    let (status, _) = t
        .call(
            Some(&amy),
            Method::GET,
            &format!("{events_path}?type=nope"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 匯出：CSV，公式開頭的內容前面會加單引號
    let req = Request::builder()
        .uri(format!("/api/sites/{site_id}/people/export?kind=member"))
        .header(header::COOKIE, &amy.0)
        .body(Body::empty())
        .unwrap();
    let (status, headers, bytes) = t.send(req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .starts_with("text/csv"));
    assert!(headers[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .contains("people-"));
    assert!(bytes.starts_with(b"\xEF\xBB\xBF"));
    let mut reader_csv = csv::Reader::from_reader(&bytes[3..]);
    let rows: Vec<csv::StringRecord> = reader_csv.records().map(Result::unwrap).collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(&rows[0][0], "eve@example.com");
    assert_eq!(&rows[0][1], "'=HYPERLINK(\"https://evil.example\")");
    assert_eq!(&rows[0][3], "1");
    assert_eq!(&rows[0][7], "看過定價");

    // 刪除個人資料只有管理員可以
    let person_path = format!("/api/sites/{site_id}/people/{eve}");
    let (status, _) = t.call(Some(&amy), Method::DELETE, &person_path, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = t
        .call(Some(&admin), Method::DELETE, &person_path, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = t.call(Some(&amy), Method::GET, &person_path, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(t.count("people").await, 1);
    assert_eq!(t.count("events").await, 7);
    assert_eq!(t.count("devices").await, 1);
    assert_eq!(t.count("person_labels").await, 0);

    // 同一個瀏覽器再來，是新的匿名訪客
    t.collect(batch(&key, "formula-0001", vec![view("/")]))
        .await;
    let anonymous = t
        .get(&amy, &format!("/api/sites/{site_id}/people?kind=anonymous"))
        .await;
    assert_eq!(anonymous["total"], 2);
}

#[tokio::test]
async fn members_only_see_the_sites_they_were_added_to() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (amy_id, amy) = t.member(&admin, "amy").await;
    let (_, ben) = t.member(&admin, "ben").await;
    let (site_id, key) = t
        .site(
            &admin,
            json!({ "name": "官網", "url": "https://www.example.com", "member_ids": [amy_id] }),
        )
        .await;
    let (other_id, _) = t.site(&admin, json!({ "name": "商店" })).await;
    t.collect(batch(&key, "anon-0000001", vec![view("/")]))
        .await;

    let sites = t.get(&amy, "/api/sites").await;
    assert_eq!(names(&sites), ["官網"]);
    assert_eq!(sites[0]["members"][0]["username"], "amy");
    assert!(t
        .get(&ben, "/api/sites")
        .await
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        t.get(&admin, "/api/sites").await.as_array().unwrap().len(),
        2
    );

    for path in [
        "",
        "/overview",
        "/people",
        "/people/export",
        "/people/1",
        "/people/1/events",
        "/pages",
        "/labels",
        "/segments",
    ] {
        let url = format!("/api/sites/{site_id}{path}");
        let (status, _) = t.call(Some(&amy), Method::GET, &url, None).await;
        assert_eq!(status, StatusCode::OK, "{url}");
        // 不說「沒有權限」，免得別人用編號去猜有哪些網站
        let (status, _) = t.call(Some(&ben), Method::GET, &url, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{url}");
        let url = format!("/api/sites/{other_id}{path}");
        let (status, _) = t.call(Some(&amy), Method::GET, &url, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{url}");
    }

    // 成員可以整理標籤和分群，但不能動別的網站的
    let label = json!({ "name": "首頁", "rules": [{ "kind": "path_prefix", "value": "/" }] });
    let created = t
        .ok(
            &amy,
            Method::POST,
            &format!("/api/sites/{site_id}/labels"),
            label.clone(),
        )
        .await;
    assert_eq!(created["people"], 1);
    for (session, url) in [
        (&ben, format!("/api/sites/{site_id}/labels")),
        (&amy, format!("/api/sites/{other_id}/labels")),
    ] {
        let (status, _) = t
            .call(Some(session), Method::POST, &url, Some(label.clone()))
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{url}");
    }
    let (status, _) = t
        .call(
            Some(&ben),
            Method::PATCH,
            &format!("/api/sites/{site_id}/labels/{}", created["id"]),
            Some(json!({ "name": "x" })),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // 另一個網站的網址拿不到這個網站的標籤
    let (status, _) = t
        .call(
            Some(&admin),
            Method::PATCH,
            &format!("/api/sites/{other_id}/labels/{}", created["id"]),
            Some(json!({ "name": "x" })),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // 網站和帳號只有管理員能管
    for (method, url, body) in [
        (
            Method::POST,
            "/api/sites".to_owned(),
            json!({ "name": "新的" }),
        ),
        (
            Method::PATCH,
            format!("/api/sites/{site_id}"),
            json!({ "name": "改名" }),
        ),
        (Method::DELETE, format!("/api/sites/{site_id}"), json!({})),
        (
            Method::POST,
            format!("/api/sites/{site_id}/rotate-key"),
            json!({}),
        ),
        (
            Method::POST,
            "/api/users".to_owned(),
            json!({ "username": "eve", "password": PASSWORD }),
        ),
    ] {
        let (status, _) = t.call(Some(&amy), method, &url, Some(body)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{url}");
    }
    let (status, _) = t.call(Some(&amy), Method::GET, "/api/users", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // 網站設定的檢查
    for body in [
        json!({ "name": " " }),
        json!({ "name": "商店" }),
        json!({ "name": "x", "url": "www.example.com" }),
        json!({ "name": "x", "member_ids": [9999] }),
    ] {
        let (status, _) = t
            .call(Some(&admin), Method::POST, "/api/sites", Some(body.clone()))
            .await;
        assert!(
            status == StatusCode::BAD_REQUEST || status == StatusCode::CONFLICT,
            "{body}"
        );
    }

    // 刪掉網站，底下的資料一起刪
    let (status, _) = t
        .call(
            Some(&admin),
            Method::DELETE,
            &format!("/api/sites/{site_id}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    for table in ["people", "events", "devices", "labels", "site_members"] {
        assert_eq!(t.count(table).await, 0, "{table}");
    }
}

#[tokio::test]
async fn accounts_can_be_managed_by_admins() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (amy_id, amy) = t.member(&admin, "amy").await;
    t.site(&admin, json!({ "name": "官網", "member_ids": [amy_id] }))
        .await;

    let users = t.get(&admin, "/api/users").await;
    let row = users
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == "amy")
        .unwrap();
    assert_eq!(row["site_count"], 1);
    assert!(row.get("password_hash").is_none());

    t.ok(
        &admin,
        Method::PATCH,
        &format!("/api/users/{amy_id}"),
        json!({ "display_name": "陳雅婷" }),
    )
    .await;
    assert_eq!(
        t.get(&amy, "/api/auth/state").await["user"]["display_name"],
        "陳雅婷"
    );
    let (status, _) = t
        .call(
            Some(&admin),
            Method::POST,
            "/api/users",
            Some(json!({ "username": "amy", "password": PASSWORD })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // 改密碼要先給對現在的密碼
    let (status, _) = t
        .call(
            Some(&amy),
            Method::POST,
            "/api/auth/password",
            Some(json!({ "current_password": "wrong", "new_password": "another-password-456" })),
        )
        .await;
    assert_ne!(status, StatusCode::OK);

    // 停用之後馬上被登出，也登不進來
    t.ok(
        &admin,
        Method::PATCH,
        &format!("/api/users/{amy_id}"),
        json!({ "disabled": true }),
    )
    .await;
    let (status, _) = t.call(Some(&amy), Method::GET, "/api/sites", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = t
        .call(
            None,
            Method::POST,
            "/api/auth/login",
            Some(json!({ "username": "amy", "password": PASSWORD })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // 管理員不能把自己停用
    let me = t.get(&admin, "/api/auth/state").await["user"]["id"].clone();
    let (status, _) = t
        .call(
            Some(&admin),
            Method::PATCH,
            &format!("/api/users/{me}"),
            Some(json!({ "disabled": true })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn old_events_are_purged_and_counters_follow() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (site_id, key) = t.site(&admin, json!({ "name": "官網" })).await;
    let label = t
        .ok(
            &admin,
            Method::POST,
            &format!("/api/sites/{site_id}/labels"),
            json!({ "name": "看過定價", "rules": [{ "kind": "path_prefix", "value": "/pricing" }] }),
        )
        .await;
    t.past_view(site_id, "old-00000001", "/pricing", 100).await;
    t.past_view(site_id, "both-0000001", "/pricing", 100).await;
    t.collect(batch(
        &key,
        "both-0000001",
        vec![view("/pricing"), view("/")],
    ))
    .await;
    assert_eq!(t.count("events").await, 4);

    assert_eq!(retention::purge(&t.state.db, 365).await.unwrap(), 0);
    assert_eq!(retention::purge(&t.state.db, 90).await.unwrap(), 2);
    assert_eq!(t.count("events").await, 2);
    // 人還在，只是瀏覽頁數和標籤照剩下的事件重算
    let people = t
        .get(
            &admin,
            &format!("/api/sites/{site_id}/people?sort=pageviews"),
        )
        .await;
    assert_eq!(people["total"], 2);
    assert_eq!(people["items"][0]["pageviews"], 2);
    assert_eq!(people["items"][1]["pageviews"], 0);
    let labels = t.get(&admin, &format!("/api/sites/{site_id}/labels")).await;
    assert_eq!(labels[0]["id"], label["id"]);
    assert_eq!(labels[0]["people"], 1);
}

#[tokio::test]
async fn demo_data_fills_an_empty_database_once() {
    let t = TestApp::new().await;
    let summary = demo::seed(&t.state, PASSWORD).await.unwrap();
    assert_eq!(summary.users, 3);
    assert_eq!(summary.sites, 2);
    assert!(summary.people > 300 && summary.events > 3000);
    assert!(demo::seed(&t.state, PASSWORD).await.is_err());

    let admin = t
        .sign_in(
            "/api/auth/login",
            json!({ "username": "demo", "password": PASSWORD }),
        )
        .await;
    let sites = t.get(&admin, "/api/sites").await;
    assert_eq!(names(&sites), ["品牌官網", "線上商店"]);
    for site in sites.as_array().unwrap() {
        let id = site["id"].as_i64().unwrap();
        let o = t.get(&admin, &format!("/api/sites/{id}/overview")).await;
        assert!(o["totals"]["identified"].as_i64().unwrap() > 30);
        assert!(o["totals"]["anonymous"].as_i64().unwrap() > 100);
        assert!(o["last_7_days"]["pageviews"].as_i64().unwrap() > 0);
        assert!(o["previous_7_days"]["pageviews"].as_i64().unwrap() > 0);
        let busy_days = o["daily"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["pageviews"].as_i64().unwrap() > 0)
            .count();
        assert!(busy_days >= 28, "{busy_days}");
        // 每個標籤、每個分群都選得到人，但不是所有人
        let total = o["totals"]["people"].as_i64().unwrap();
        for label in t
            .get(&admin, &format!("/api/sites/{id}/labels"))
            .await
            .as_array()
            .unwrap()
        {
            let people = label["people"].as_i64().unwrap();
            assert!(people > 0 && people < total, "{label}");
        }
        for segment in t
            .get(&admin, &format!("/api/sites/{id}/segments"))
            .await
            .as_array()
            .unwrap()
        {
            let count = segment["count"].as_i64().unwrap();
            assert!(count > 0 && count < total, "{segment}");
        }
    }
    // 累計的數字和直接從事件算出來的一樣
    let drift: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM people p WHERE p.pageviews !=
            (SELECT COUNT(*) FROM events e WHERE e.person_id = p.id AND e.type = 'pageview')",
    )
    .fetch_one(&t.state.db)
    .await
    .unwrap();
    assert_eq!(drift, 0);
    let before: Vec<(i64, i64, i64)> =
        sqlx::query_as("SELECT person_id, label_id, hits FROM person_labels ORDER BY 1, 2")
            .fetch_all(&t.state.db)
            .await
            .unwrap();
    let mut conn = t.state.db.acquire().await.unwrap();
    for site_id in [1, 2] {
        lite_cdp::labels::recompute_site(&mut conn, site_id)
            .await
            .unwrap();
    }
    drop(conn);
    let after: Vec<(i64, i64, i64)> =
        sqlx::query_as("SELECT person_id, label_id, hits FROM person_labels ORDER BY 1, 2")
            .fetch_all(&t.state.db)
            .await
            .unwrap();
    assert_eq!(before, after, "收事件時累計的和整個重算的要一樣");
    let future: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE created_at > ?")
        .bind(db::now())
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert_eq!(future, 0);

    // 成員只看得到自己負責的網站
    let ben = t
        .sign_in(
            "/api/auth/login",
            json!({ "username": demo::MEMBER_USERNAMES[1], "password": PASSWORD }),
        )
        .await;
    assert_eq!(names(&t.get(&ben, "/api/sites").await), ["線上商店"]);
}

#[tokio::test]
async fn meta_and_unknown_api_paths() {
    let t = TestApp::new().await;
    let (status, meta) = t.call(None, Method::GET, "/api/meta", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(meta["public_url"], "https://cdp.example.com");
    assert_eq!(meta["retention_days"], 0);
    let (status, body) = t.call(None, Method::GET, "/api/health", None).await;
    assert_eq!((status, body), (StatusCode::OK, json!("ok")));
    let (status, body) = t.call(None, Method::GET, "/api/nope", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "找不到這個 API");
}

#[tokio::test]
async fn concurrent_setup_creates_exactly_one_administrator() {
    let app = TestApp::new().await;
    let request = |index| {
        app.call(
            None,
            Method::POST,
            "/api/auth/setup",
            Some(json!({ "username": format!("admin{index}"), "password": PASSWORD })),
        )
    };
    let (a, b, c, d, e, f) = tokio::join!(
        request(0),
        request(1),
        request(2),
        request(3),
        request(4),
        request(5)
    );
    let results = [a, b, c, d, e, f];
    assert_eq!(
        results
            .iter()
            .filter(|(status, _)| *status == StatusCode::OK)
            .count(),
        1
    );
    assert!(results
        .iter()
        .all(|(status, _)| matches!(*status, StatusCode::OK | StatusCode::CONFLICT)));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn engagement_cohorts_match_lists_exports_and_site_access() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (uid, member) = t.member(&admin, "atlas").await;
    let (site_id, _) = t
        .site(&admin, json!({"name":"Visible","member_ids":[uid]}))
        .await;
    let (hidden, _) = t.site(&admin, json!({"name":"Hidden"})).await;
    for (i, days) in [0, 3, 8, 20, 31, 60].into_iter().enumerate() {
        t.past_view(site_id, &format!("atlas-person-{i}"), "/", days)
            .await;
    }
    // One member and one anonymous visitor in each disjoint recency bucket.
    sqlx::query("UPDATE people SET email = 'member-' || id || '@example.com' WHERE site_id=? AND id % 2 = 1")
        .bind(site_id).execute(&t.state.db).await.unwrap();
    t.past_view(hidden, "hidden-person", "/", 0).await;
    let overview = t
        .get(&member, &format!("/api/sites/{site_id}/overview"))
        .await;
    let far_page = t
        .get(
            &member,
            &format!("/api/sites/{site_id}/people?page=9223372036854775807"),
        )
        .await;
    assert!(far_page["items"].as_array().unwrap().is_empty());
    assert_eq!(
        overview["engagement"],
        json!([
            {"activity":"recent","member":1,"anonymous":1},
            {"activity":"cooling","member":1,"anonymous":1},
            {"activity":"dormant","member":1,"anonymous":1}
        ])
    );
    for activity in ["recent", "cooling", "dormant"] {
        for kind in ["member", "anonymous"] {
            let query = format!("activity={activity}&kind={kind}");
            let list = t
                .get(&member, &format!("/api/sites/{site_id}/people?{query}"))
                .await;
            assert_eq!(list["total"], 1);
            let csv = t
                .get(
                    &member,
                    &format!("/api/sites/{site_id}/people/export?{query}"),
                )
                .await;
            assert_eq!(csv.as_str().unwrap().lines().count(), 2);
        }
    }
    assert_eq!(
        t.call(
            Some(&member),
            Method::GET,
            &format!("/api/sites/{hidden}/overview"),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    for suffix in ["people?activity=bogus", "people/export?activity=bogus"] {
        assert_eq!(
            t.call(
                Some(&admin),
                Method::GET,
                &format!("/api/sites/{site_id}/{suffix}"),
                None
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
}

#[tokio::test]
async fn signed_subject_requires_server_secret_and_rejects_expiry_or_other_site() {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (id, key) = t
        .site(
            &admin,
            json!({"name":"signed","collection_policy":{"require_signed_identity":true}}),
        )
        .await;
    let secret = t
        .get(&admin, &format!("/api/sites/{id}/identity-secret"))
        .await;
    let expires = Utc::now().timestamp() + 900;
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret["secret"].as_str().unwrap().as_bytes()).unwrap();
    mac.update(format!("{key}\nmember-signed\n{expires}").as_bytes());
    let signature: String = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let mut payload = identify(
        &key,
        "signed-browser",
        json!({"user_id":"member-signed","mutation_id":"signed-mutation-1","revision":100}),
    );
    assert_eq!(
        t.collect_as(&payload, CHROME, None).await.0,
        StatusCode::FORBIDDEN
    );
    payload["identify"]["proof"] = json!({"expires_at":expires,"signature":signature});
    t.collect(payload.clone()).await;
    let people = t
        .get(&admin, &format!("/api/sites/{id}/people?kind=member"))
        .await;
    assert_eq!(people["total"], 1);
    assert_eq!(people["items"][0]["identity_verified"], true);
    let mut referenced = batch(
        &key,
        "signed-browser",
        vec![json!({"type":"custom","event_id":"signed-context-1","name":"after-proof"})],
    );
    referenced["identity_context"] = json!({"mutation_id":"signed-mutation-1","revision":100});
    t.collect(referenced).await;

    payload["identify"]["proof"]["expires_at"] = json!(Utc::now().timestamp() - 1);
    assert_eq!(
        t.collect_as(&payload, CHROME, None).await.0,
        StatusCode::FORBIDDEN
    );
    payload["identify"]["proof"]["expires_at"] = json!(expires);
    let (_, other) = t.site(&admin, json!({"name":"other"})).await;
    payload["key"] = json!(other);
    assert_eq!(
        t.collect_as(&payload, CHROME, None).await.0,
        StatusCode::FORBIDDEN
    );
    let (_, member) = t.member(&admin, "nosign").await;
    assert_eq!(
        t.call(
            Some(&member),
            Method::GET,
            &format!("/api/sites/{id}/identity-secret"),
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn client_event_time_is_bounded_and_received_time_is_separate() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (id, key) = t.site(&admin, json!({"name":"clock"})).await;
    for at in [
        Utc::now() - Duration::days(8),
        Utc::now() + Duration::minutes(10),
    ] {
        let mut event = view("/");
        event["event_id"] = json!("clock-invalid");
        event["happened_at"] = json!(db::format_time(at));
        assert_eq!(
            t.collect_as(&batch(&key, "clock-browser", vec![event]), CHROME, None)
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    let at = db::format_time(Utc::now() - Duration::minutes(5));
    let mut event = view("/");
    event["event_id"] = json!("clock-valid-1");
    event["happened_at"] = json!(&at);
    t.collect(batch(&key, "clock-browser", vec![event])).await;
    let people = t.get(&admin, &format!("/api/sites/{id}/people")).await;
    let pid = people["items"][0]["id"].as_i64().unwrap();
    let events = t
        .get(&admin, &format!("/api/sites/{id}/people/{pid}/events"))
        .await;
    assert_eq!(events["items"][0]["happened_at"], at);
    assert_ne!(
        events["items"][0]["received_at"],
        events["items"][0]["happened_at"]
    );
}

#[tokio::test]
async fn expired_or_wrong_site_installation_tokens_never_fall_back_to_analytics() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (id, key) = t.site(&admin, json!({"name":"test-owner"})).await;
    let created = t
        .ok(
            &admin,
            Method::POST,
            &format!("/api/sites/{id}/installation-tests"),
            json!({}),
        )
        .await;
    let token = created["token"].as_str().unwrap();
    let (_, other) = t.site(&admin, json!({"name":"not-test-owner"})).await;
    let mut payload = batch(&other, "test-browser", vec![view("/")]);
    payload["test_token"] = json!(token);
    assert_eq!(
        t.collect_as(&payload, CHROME, None).await.0,
        StatusCode::FORBIDDEN
    );
    payload["key"] = json!(key);
    sqlx::query("UPDATE installation_tests SET expires_at='2000-01-01T00:00:00Z'")
        .execute(&t.state.db)
        .await
        .unwrap();
    assert_eq!(
        t.collect_as(&payload, CHROME, None).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(t.count("events").await, 0);
    assert_eq!(t.count("people").await, 0);
}

#[tokio::test]
async fn replayed_legacy_identity_batch_cannot_undo_a_later_account_switch() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (id, key) = t.site(&admin, json!({"name":"identity replay"})).await;
    let mut a = batch(
        &key,
        "browser-review-123",
        vec![
            json!({"type":"pageview","url":"https://shop.example/a","event_id":"review-event-aaa"}),
        ],
    );
    a["identify"] = json!({"user_id":"account-A","name":"old A"});
    let mut b = batch(
        &key,
        "browser-review-123",
        vec![
            json!({"type":"pageview","url":"https://shop.example/b","event_id":"review-event-bbb"}),
        ],
    );
    b["identify"] = json!({"user_id":"account-B","name":"current B"});
    t.collect(a.clone()).await;
    t.collect(b).await;
    t.collect(a).await;
    t.collect(batch(
        &key,
        "browser-review-123",
        vec![json!({"type":"custom","name":"after-B","event_id":"review-event-ccc"})],
    ))
    .await;
    let current: String = sqlx::query_scalar(
        "SELECT p.user_id FROM devices d JOIN people p ON p.id=d.person_id WHERE d.site_id=?",
    )
    .bind(id)
    .fetch_one(&t.state.db)
    .await
    .unwrap();
    assert_eq!(current, "account-B");
    let owner: String = sqlx::query_scalar(
        "SELECT p.user_id FROM events e JOIN people p ON p.id=e.person_id WHERE e.name='after-B'",
    )
    .fetch_one(&t.state.db)
    .await
    .unwrap();
    assert_eq!(owner, "account-B");
    let identifies: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE type='identify'")
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert_eq!(identifies, 2);
}

#[tokio::test]
async fn ordered_identity_only_replay_and_older_first_delivery_preserve_latest_subject_and_traits()
{
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (id, key) = t.site(&admin, json!({"name":"identity ordering"})).await;
    let mutation = |user: &str, revision: i64, mutation: &str| {
        identify(
            &key,
            "ordered-browser",
            json!({"user_id":user,"revision":revision,"mutation_id":mutation,"name":user,"traits":{"plan":user}}),
        )
    };
    let a = mutation("account-A", 100, "identity-mutation-A");
    let b = mutation("account-B", 200, "identity-mutation-B");
    t.collect(a.clone()).await;
    t.collect(b.clone()).await;
    t.collect(a).await;
    // A different old claim never previously delivered is also stale, even with fresh event IDs.
    let mut old = mutation("account-C", 150, "identity-mutation-C");
    old["events"] = json!([{"type":"custom","name":"stale-event","event_id":"stale-event-001"}]);
    t.collect(old).await;
    // Reusing the accepted mutation cannot update its profile or insert another identify event.
    let mut altered = b.clone();
    altered["identify"]["name"] = json!("overwrite");
    altered["identify"]["traits"] = json!({"plan":"overwrite"});
    t.collect(altered).await;
    // Legacy metadata-free identify cannot downgrade an already ordered device.
    t.collect(identify(
        &key,
        "ordered-browser",
        json!({"user_id":"account-A","name":"downgrade"}),
    ))
    .await;
    t.collect(batch(
        &key,
        "ordered-browser",
        vec![json!({"type":"custom","name":"after-newest","event_id":"ordered-after-001"})],
    ))
    .await;
    let current:(String,String,String)=sqlx::query_as("SELECT p.user_id,p.name,p.traits FROM devices d JOIN people p ON p.id=d.person_id WHERE d.site_id=?").bind(id).fetch_one(&t.state.db).await.unwrap();
    assert_eq!(
        current,
        (
            "account-B".into(),
            "account-B".into(),
            r#"{"plan":"account-B"}"#.into()
        )
    );
    let stale: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE name='stale-event'")
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert_eq!(stale, 0);
    assert_eq!(t.count("people").await, 2);
    let identifies: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE type='identify'")
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert_eq!(identifies, 2);
    let owner:String=sqlx::query_scalar("SELECT p.user_id FROM events e JOIN people p ON p.id=e.person_id WHERE e.name='after-newest'").fetch_one(&t.state.db).await.unwrap();
    assert_eq!(owner, "account-B");
}

#[tokio::test]
async fn identity_order_metadata_is_paired_and_bounded_while_legacy_email_only_remains_usable() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (_, key) = t
        .site(&admin, json!({"name":"identity compatibility"}))
        .await;
    for metadata in [
        json!({"mutation_id":"mutation-001"}),
        json!({"revision":1}),
        json!({"mutation_id":"bad","revision":1}),
        json!({"mutation_id":"mutation-001","revision":0}),
        json!({"mutation_id":"mutation-001","revision":9007199254740992_i64}),
    ] {
        let mut who = metadata;
        who["user_id"] = json!("member");
        assert_eq!(
            t.collect_as(&identify(&key, "invalid-browser", who), CHROME, None)
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    for email in ["a@example.com", "b@example.com", "a@example.com"] {
        t.collect(identify(&key, "legacy-browser", json!({"email":email})))
            .await;
    }
    let current: String =
        sqlx::query_scalar("SELECT p.email FROM devices d JOIN people p ON p.id=d.person_id")
            .fetch_one(&t.state.db)
            .await
            .unwrap();
    assert_eq!(current, "a@example.com");
}

#[tokio::test]
async fn event_identity_context_rejects_stale_delivery_without_requiring_a_new_signature() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (_, key) = t.site(&admin, json!({"name":"event context"})).await;
    for (user, revision, mutation) in [
        ("A", 100, "context-mutation-A"),
        ("B", 200, "context-mutation-B"),
    ] {
        t.collect(identify(
            &key,
            "context-browser",
            json!({"user_id":user,"revision":revision,"mutation_id":mutation}),
        ))
        .await;
    }
    for (revision, mutation, name) in [
        (100, "context-mutation-A", "stale-context"),
        (200, "context-mutation-B", "current-context"),
    ] {
        let mut payload = batch(
            &key,
            "context-browser",
            vec![
                json!({"type":"custom","event_id":format!("context-event-{revision}"),"name":name}),
            ],
        );
        payload["identity_context"] = json!({"revision":revision,"mutation_id":mutation});
        t.collect(payload).await;
    }
    let events:Vec<(String,String)>=sqlx::query_as("SELECT e.name,p.user_id FROM events e JOIN people p ON p.id=e.person_id WHERE e.type='custom'").fetch_all(&t.state.db).await.unwrap();
    assert_eq!(events, vec![("current-context".into(), "B".into())]);
}

#[tokio::test]
async fn timestamped_legacy_identity_cannot_overwrite_a_newer_claim() {
    let t = TestApp::new().await;
    let admin = t.admin().await;
    let (_, key) = t.site(&admin, json!({"name":"legacy timestamp"})).await;
    for (user, at) in [("B", Utc::now()), ("A", Utc::now() - Duration::seconds(10))] {
        let mut payload = identify(&key, "timestamp-browser", json!({"user_id":user}));
        payload["events"] = json!([{"type":"pageview","url":"https://shop.example/","event_id":format!("timestamp-event-{user}"),"happened_at":db::format_time(at)}]);
        t.collect(payload).await;
    }
    let current: String =
        sqlx::query_scalar("SELECT p.user_id FROM devices d JOIN people p ON p.id=d.person_id")
            .fetch_one(&t.state.db)
            .await
            .unwrap();
    assert_eq!(current, "B");
    assert_eq!(t.count("people").await, 1);
}
