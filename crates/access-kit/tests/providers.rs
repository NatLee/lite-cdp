#[cfg(feature = "test-support")]
#[path = "support/mock_provider.rs"]
mod mock_provider;
mod support;
use access_kit::*;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;
#[tokio::test]
async fn disabled_provider_and_identity_management_have_real_state() {
    let db = support::database().await;
    let kit = AccessKit::new(
        db,
        AccessConfig::for_origin("http://127.0.0.1:9999").unwrap(),
        support::TestHost::new(),
    )
    .await
    .unwrap();
    let app = kit.router();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/access/identities")
                .header("cookie", "session=owner")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let v: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 10000).await.unwrap()).unwrap();
    assert_eq!(v["items"], serde_json::json!([]));
    let r = app
        .oneshot(
            Request::builder()
                .uri("/oauth/login/github")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        r.headers()["location"],
        "/login?access_error=provider_disabled"
    );
}
#[cfg(feature = "test-support")]
mod controlled {
    use super::*;
    use axum::{http::HeaderMap, Router};
    use std::future::IntoFuture;

    async fn send(
        app: &Router,
        method: &str,
        path: &str,
        cookie: &str,
        body: &str,
    ) -> (StatusCode, HeaderMap, Value) {
        let r = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("cookie", cookie)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_owned()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let s = r.status();
        let h = r.headers().clone();
        let b = to_bytes(r.into_body(), 100000).await.unwrap();
        (s, h, serde_json::from_slice(&b).unwrap_or(Value::Null))
    }
    async fn start(app: &Router, provider: &str, link: bool) -> (String, String) {
        let (s, h, v) = send(
            app,
            if link { "POST" } else { "GET" },
            &if link {
                format!("/api/access/identities/{provider}/link")
            } else {
                format!("/oauth/login/{provider}")
            },
            if link { "session=owner" } else { "" },
            if link { "{}" } else { "" },
        )
        .await;
        assert_eq!(
            s,
            if link {
                StatusCode::OK
            } else {
                StatusCode::SEE_OTHER
            },
            "{v}"
        );
        let url = if link {
            v["authorization_url"].as_str().unwrap().to_owned()
        } else {
            h["location"].to_str().unwrap().to_owned()
        };
        let cookie = format!(
            "session=owner; {}",
            h["set-cookie"].to_str().unwrap().split(';').next().unwrap()
        );
        (url, cookie)
    }
    async fn callback(
        app: &Router,
        url: &str,
        cookie: &str,
        mode: &str,
    ) -> (StatusCode, HeaderMap, Value) {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let h = client.get(url).send().await.unwrap().headers().clone();
        let target = url::Url::parse(h["location"].to_str().unwrap()).unwrap();
        let state = target
            .query_pairs()
            .find(|(k, _)| k == "state")
            .unwrap()
            .1
            .to_string();
        let path = format!(
            "{}?{}",
            target.path(),
            serde_urlencoded::to_string([("state", state.as_str()), ("code", mode)]).unwrap()
        );
        send(app, "GET", &path, cookie, "").await
    }
    #[tokio::test]
    async fn real_provider_exchange_validates_google_signatures_claims_and_binding() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(
            axum::serve(
                listener,
                crate::mock_provider::MockProvider::router(&origin),
            )
            .into_future(),
        );
        let db = support::database().await;
        let config = AccessConfig::for_origin("http://127.0.0.1:9999")
            .unwrap()
            .with_mock_providers(&origin)
            .unwrap();
        let host = support::TestHost::new();
        let kit = AccessKit::new(db.clone(), config, host.clone())
            .await
            .unwrap();
        let app = kit.router();
        let (_, h, _) = send(
            &app,
            "GET",
            "/oauth/login/google?next=%2Fsettings%2Faccess",
            "",
            "",
        )
        .await;
        assert!(h["location"]
            .to_str()
            .unwrap()
            .starts_with(&format!("{origin}/google/authorize")));
        let (url, cookie) = start(&app, "google", false).await;
        let (_, h, _) = callback(&app, &url, &cookie, "good").await;
        assert_eq!(h["location"], "/login?access_error=identity_not_linked");
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM access_identities")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 0, "same email never creates or merges accounts");
        for mode in ["bad_nonce", "bad_iss", "bad_aud", "bad_sig", "expired"] {
            let (url, cookie) = start(&app, "google", true).await;
            let (_, h, _) = callback(&app, &url, &cookie, mode).await;
            assert_eq!(
                h["location"], "/settings/access?access_error=access_denied",
                "{mode}"
            );
        }
        let (url, cookie) = start(&app, "google", true).await;
        let wrong_cookie = format!("session=owner; access_browser={}", "x".repeat(43));
        let (_, h, _) = callback(&app, &url, &wrong_cookie, "good").await;
        assert_eq!(h["location"], "/settings/access?access_error=state_invalid");
        host.generation
            .store(2, std::sync::atomic::Ordering::SeqCst);
        let (_, h, _) = callback(&app, &url, &cookie, "good").await;
        assert_eq!(h["location"], "/settings/access?access_error=state_invalid");
        host.generation
            .store(1, std::sync::atomic::Ordering::SeqCst);
        let (_, h, _) = callback(&app, &url, &cookie, "good").await;
        assert_eq!(h["location"], "/settings/access");
        let (_, h, _) = callback(&app, &url, &cookie, "good").await;
        assert_eq!(h["location"], "/login?access_error=state_invalid");
        let (url, cookie) = start(&app, "google", false).await;
        let (_, h, _) = callback(&app, &url, &cookie, "good").await;
        assert!(h["set-cookie"]
            .to_str()
            .unwrap()
            .starts_with("session=owner"));
        sqlx::query("INSERT INTO access_identities(id,user_id,provider,subject,linked_at)VALUES('other',2,'github','1234',0)").execute(&db).await.unwrap();
        let (url, cookie) = start(&app, "github", true).await;
        let (_, h, _) = callback(&app, &url, &cookie, "good").await;
        assert_eq!(
            h["location"],
            "/settings/access?access_error=identity_conflict"
        );
        sqlx::query("DELETE FROM access_identities WHERE id='other'")
            .execute(&db)
            .await
            .unwrap();
        let (url, cookie) = start(&app, "github", true).await;
        let (_, h, _) = callback(&app, &url, &cookie, "good").await;
        assert_eq!(h["location"], "/settings/access");
        let subjects: Vec<String> =
            sqlx::query_scalar("SELECT subject FROM access_identities ORDER BY provider")
                .fetch_all(&db)
                .await
                .unwrap();
        assert_eq!(subjects, vec!["1234", "google-subject-1234"]);
        let secrets:i64=sqlx::query_scalar("SELECT COUNT(*) FROM access_provider_flows WHERE used_at IS NOT NULL AND (verifier!='' OR nonce!='')").fetch_one(&db).await.unwrap();
        assert_eq!(secrets, 0);
        server.abort();
    }
}
