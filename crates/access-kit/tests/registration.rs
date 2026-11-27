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
    method: &str,
    path: &str,
    cookie: &str,
    body: Value,
) -> (StatusCode, Value, bool) {
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .header("origin", "http://127.0.0.1:9999")
                .header("cookie", cookie)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = r.status();
    let cookie = r.headers().contains_key("set-cookie");
    let b = to_bytes(r.into_body(), 100000).await.unwrap();
    (
        status,
        serde_json::from_slice(&b).unwrap_or(Value::Null),
        cookie,
    )
}
#[tokio::test]
async fn registration_defaults_closed_and_requires_admin_for_policy() {
    let db = support::database().await;
    let kit = AccessKit::new(
        db,
        AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
        support::TestHost::new(),
    )
    .await
    .unwrap();
    let app = kit.router();
    let (s, v, _) = req(
        &app,
        "GET",
        "/api/auth/registration-policy",
        "",
        Value::Null,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["mode"], "closed");
    let (s, v, _) = req(
        &app,
        "POST",
        "/api/auth/register",
        "session=owner",
        json!({"username":"another","password":"test-password-123"}),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["code"], "already_signed_in");
    let (s, _, _) = req(
        &app,
        "PATCH",
        "/api/auth/registration-policy",
        "",
        json!({"mode":"open"}),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

struct DatabaseHost {
    db: sqlx::SqlitePool,
    base: std::sync::Arc<support::TestHost>,
}
impl Host for DatabaseHost {
    fn registration_admin<'a>(
        &'a self,
        c: &'a mut sqlx::SqliteConnection,
        _: &'a axum::http::HeaderMap,
        id: i64,
    ) -> HostFuture<'a, bool> {
        Box::pin(async move {
            Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id=? AND role='admin' AND account_status='active' AND disabled=0)").bind(id).fetch_one(c).await?)
        })
    }

    fn registration_count<'a>(&'a self, c: &'a mut sqlx::SqliteConnection) -> HostFuture<'a, i64> {
        Box::pin(async move {
            Ok(sqlx::query_scalar("SELECT COUNT(*) FROM users")
                .fetch_one(c)
                .await?)
        })
    }
    fn registration_create<'a>(
        &'a self,
        c: &'a mut sqlx::SqliteConnection,
        u: RegistrationUser,
    ) -> HostFuture<'a, i64> {
        Box::pin(async move {
            Ok(sqlx::query("INSERT INTO users(username,display_name,password_hash,account_status,password_login_enabled)VALUES(?,?,?,?,?)").bind(u.username).bind(u.display_name).bind(u.password_hash).bind(u.account_status).bind(u.password_login_enabled).execute(c).await?.last_insert_rowid())
        })
    }
    fn registration_account<'a>(
        &'a self,
        c: &'a mut sqlx::SqliteConnection,
        id: i64,
    ) -> HostFuture<'a, Option<RegistrationAccount>> {
        Box::pin(async move {
            let r: Option<(String, bool)> = sqlx::query_as(
                "SELECT account_status,password_login_enabled FROM users WHERE id=?",
            )
            .bind(id)
            .fetch_optional(c)
            .await?;
            Ok(
                r.map(|(status, password_login_enabled)| RegistrationAccount {
                    status,
                    password_login_enabled,
                }),
            )
        })
    }
    fn registration_set_status<'a>(
        &'a self,
        c: &'a mut sqlx::SqliteConnection,
        id: i64,
        status: &'a str,
    ) -> HostFuture<'a, bool> {
        Box::pin(async move {
            Ok(sqlx::query("UPDATE users SET account_status=? WHERE id=? AND account_status='pending' AND role='member'").bind(status).bind(id).execute(c).await?.rows_affected()==1)
        })
    }
    fn cookie_principal<'a>(
        &'a self,
        h: &'a axum::http::HeaderMap,
    ) -> HostFuture<'a, Option<CookieSession>> {
        self.base.cookie_principal(h)
    }
    fn active_principal(&self, id: i64) -> HostFuture<'_, Option<Principal>> {
        Box::pin(async move {
            let r:Option<(String,String)>=sqlx::query_as("SELECT username,display_name FROM users WHERE id=? AND account_status='active' AND disabled=0").bind(id).fetch_optional(&self.db).await?;
            Ok(r.map(|(username, display_name)| Principal {
                user_id: id,
                username,
                display_name,
                role: if id == 1 { Role::Admin } else { Role::Member },
            }))
        })
    }
    fn create_session(&self, id: i64) -> HostFuture<'_, axum::http::HeaderValue> {
        Box::pin(async move {
            if self.active_principal(id).await?.is_none() {
                return Err(AccessError::new(
                    StatusCode::FORBIDDEN,
                    "account_unavailable",
                    "Account not active",
                ));
            }
            Ok(
                axum::http::HeaderValue::from_str(&format!("session=user-{id}; Path=/; HttpOnly"))
                    .unwrap(),
            )
        })
    }
    fn operations(&self) -> Vec<DomainOperation> {
        self.base.operations()
    }
    fn execute(&self, i: Invocation) -> HostFuture<'_, Value> {
        self.base.execute(i)
    }
    fn paths(&self) -> HostPaths {
        self.base.paths()
    }
}
async fn dbhost() -> (sqlx::SqlitePool, std::sync::Arc<DatabaseHost>) {
    let db = support::database().await;
    sqlx::raw_sql("CREATE TABLE users(id INTEGER PRIMARY KEY,username TEXT NOT NULL UNIQUE,display_name TEXT NOT NULL,password_hash TEXT NOT NULL,account_status TEXT NOT NULL DEFAULT 'active',password_login_enabled INTEGER NOT NULL DEFAULT 1,disabled INTEGER NOT NULL DEFAULT 0,role TEXT NOT NULL DEFAULT 'member');INSERT INTO users(username,display_name,password_hash,role)VALUES('admin','Admin','unused','admin')").execute(&db).await.unwrap();
    let h = std::sync::Arc::new(DatabaseHost {
        db: db.clone(),
        base: support::TestHost::new(),
    });
    (db, h)
}
#[tokio::test]
async fn account_cap_rejects_atomically_and_last_identity_requires_password() {
    let (db, host) = dbhost().await;
    let kit = AccessKit::new(
        db.clone(),
        AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
        host,
    )
    .await
    .unwrap();
    let app = kit.router();
    req(
        &app,
        "PATCH",
        "/api/auth/registration-policy",
        "session=owner",
        json!({"mode":"open"}),
    )
    .await;
    sqlx::query("WITH RECURSIVE n(x) AS(VALUES(2) UNION ALL SELECT x+1 FROM n WHERE x<500) INSERT INTO users(id,username,display_name,password_hash) SELECT x,'user-'||x,'User','unused' FROM n").execute(&db).await.unwrap();
    let (s, v, _) = req(
        &app,
        "POST",
        "/api/auth/register",
        "",
        json!({"username":"overflow","password":"test-password-123"}),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(v["code"], "account_limit");
    sqlx::query("DELETE FROM users WHERE id=500")
        .execute(&db)
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        req(
            &app,
            "POST",
            "/api/auth/register",
            "",
            json!({"username":"race-one","password":"test-password-123"})
        ),
        req(
            &app,
            "POST",
            "/api/auth/register",
            "",
            json!({"username":"race-two","password":"test-password-123"})
        )
    );
    assert!([a.0, b.0].contains(&StatusCode::CREATED));
    assert!([a.0, b.0].contains(&StatusCode::CONFLICT));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 500);
    sqlx::query("UPDATE users SET password_login_enabled=0 WHERE id=1")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO access_identities VALUES('only',1,'google','stable-sub',0,NULL)")
        .execute(&db)
        .await
        .unwrap();
    let (s, v, _) = req(
        &app,
        "DELETE",
        "/api/access/identities/only",
        "session=owner",
        Value::Null,
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(v["code"], "last_login_method");
    sqlx::query("INSERT INTO access_identities VALUES('second',1,'github','42',0,NULL)")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        req(
            &app,
            "DELETE",
            "/api/access/identities/only",
            "session=owner",
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
}
#[cfg(feature = "test-support")]
#[path = "support/mock_provider.rs"]
mod mock_provider;
#[cfg(feature = "test-support")]
#[tokio::test]
async fn provider_signup_rechecks_policy_and_keeps_pending_separate_from_login() {
    use std::future::IntoFuture;
    async fn start(app: &Router, provider: &str, signup: bool) -> (String, String) {
        let r = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/oauth/login/{provider}{}",
                        if signup { "?intent=register" } else { "" }
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::SEE_OTHER);
        (
            r.headers()["location"].to_str().unwrap().to_owned(),
            r.headers()["set-cookie"]
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .to_owned(),
        )
    }
    async fn callback(app: &Router, url: &str, cookie: &str) -> axum::response::Response {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let r = client.get(url).send().await.unwrap();
        let u = url::Url::parse(r.headers()["location"].to_str().unwrap()).unwrap();
        let state = u
            .query_pairs()
            .find(|(k, _)| k == "state")
            .unwrap()
            .1
            .to_string();
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "{}?{}",
                        u.path(),
                        serde_urlencoded::to_string([("state", state.as_str()), ("code", "good")])
                            .unwrap()
                    ))
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(
        axum::serve(listener, mock_provider::MockProvider::router(&origin)).into_future(),
    );
    let (db, host) = dbhost().await;
    let config = AccessConfig::for_origin("http://127.0.0.1:9999")
        .unwrap()
        .with_mock_providers(&origin)
        .unwrap();
    let kit = AccessKit::new(db.clone(), config, host).await.unwrap();
    let app = kit.router();
    req(
        &app,
        "PATCH",
        "/api/auth/registration-policy",
        "session=owner",
        json!({"mode":"approval"}),
    )
    .await;
    let (url, cookie) = start(&app, "google", true).await;
    req(
        &app,
        "PATCH",
        "/api/auth/registration-policy",
        "session=owner",
        json!({"mode":"closed"}),
    )
    .await;
    let r = callback(&app, &url, &cookie).await;
    assert_eq!(
        r.headers()["location"],
        "/login?access_error=registration_closed"
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 1);
    req(
        &app,
        "PATCH",
        "/api/auth/registration-policy",
        "session=owner",
        json!({"mode":"approval"}),
    )
    .await;
    let (url, cookie) = start(&app, "google", true).await;
    let r = callback(&app, &url, &cookie).await;
    assert_eq!(r.headers()["location"], "/login?registration=pending");
    assert!(!r.headers().contains_key("set-cookie"));
    let (status, enabled): (String, bool) =
        sqlx::query_as("SELECT account_status,password_login_enabled FROM users WHERE id=2")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(status, "pending");
    assert!(!enabled);
    let (url, cookie) = start(&app, "google", false).await;
    let r = callback(&app, &url, &cookie).await;
    assert_eq!(
        r.headers()["location"],
        "/login?access_error=account_pending"
    );
    let (url, cookie) = start(&app, "google", true).await;
    let r = callback(&app, &url, &cookie).await;
    assert_eq!(
        r.headers()["location"],
        "/login?access_error=identity_conflict"
    );
    req(
        &app,
        "POST",
        "/api/auth/registrations/1/review",
        "session=owner",
        json!({"decision":"reject","reason":"Not allowed"}),
    )
    .await;
    let (url, cookie) = start(&app, "google", false).await;
    let r = callback(&app, &url, &cookie).await;
    assert_eq!(
        r.headers()["location"],
        "/login?access_error=account_rejected"
    );
    req(
        &app,
        "PATCH",
        "/api/auth/registration-policy",
        "session=owner",
        json!({"mode":"open"}),
    )
    .await;
    let (url, cookie) = start(&app, "github", true).await;
    let r = callback(&app, &url, &cookie).await;
    assert!(r.headers().contains_key("set-cookie"));
    assert_eq!(r.headers()["location"], "/settings/access");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 3, "Subjects never merge accounts by email");
    server.abort();
}

#[tokio::test]
async fn write_transaction_revalidates_admin_and_review_never_activates_admins() {
    let (db, host) = dbhost().await;
    let kit = AccessKit::new(
        db.clone(),
        AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
        host,
    )
    .await
    .unwrap();
    let app = kit.router();
    // The outer fixture still reports an admin cookie; the transactional authority is current.
    sqlx::query("UPDATE users SET role='member' WHERE id=1")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        req(
            &app,
            "PATCH",
            "/api/auth/registration-policy",
            "session=owner",
            json!({"mode":"approval"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM access_registration_events")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 0);
    sqlx::query("UPDATE users SET role='admin' WHERE id=1")
        .execute(&db)
        .await
        .unwrap();
    req(
        &app,
        "PATCH",
        "/api/auth/registration-policy",
        "session=owner",
        json!({"mode":"approval"}),
    )
    .await;
    assert_eq!(
        req(
            &app,
            "POST",
            "/api/auth/register",
            "",
            json!({"username":"pending","password":"test-password-123"})
        )
        .await
        .0,
        StatusCode::CREATED
    );
    sqlx::query("UPDATE users SET role='admin' WHERE id=2")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        req(
            &app,
            "POST",
            "/api/auth/registrations/1/review",
            "session=owner",
            json!({"decision":"approve"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let status: String = sqlx::query_scalar("SELECT status FROM access_registrations WHERE id=1")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(status, "pending");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM access_registration_events")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn new_registration_mutations_require_explicit_origin() {
    let db = support::database().await;
    let kit = AccessKit::new(
        db,
        AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
        support::TestHost::new(),
    )
    .await
    .unwrap();
    let app = kit.router();
    let r = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/auth/registration-policy")
                .header("content-type", "application/json")
                .header("cookie", "session=owner")
                .body(Body::from(json!({"mode":"open"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}
