//! HTTP API（規格見 `docs/api.md`）、收追蹤資料的公開網址，和前端靜態檔。

mod auth;
mod collect;
mod collection;
mod labels;
mod overview;
mod pages;
mod people;
mod segments;
pub mod sites;
mod users;

use axum::{
    extract::State,
    http::{header, HeaderValue},
    middleware,
    routing::{get, patch, post},
    Json, Router,
};
use serde_json::{json, Value};
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};

use crate::{auth::origin_guard, error::AppError, state::AppState};

fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/meta", get(meta))
        .route("/auth/state", get(auth::state))
        .route("/auth/setup", post(auth::setup))
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", patch(auth::update_me))
        .route("/auth/password", post(auth::change_password))
        .route("/users", get(users::list).post(users::create))
        .route("/users/{id}", patch(users::update))
        .route("/sites", get(sites::list).post(sites::create))
        .route(
            "/sites/{id}",
            get(sites::detail)
                .patch(sites::update)
                .delete(sites::remove),
        )
        .route("/sites/{id}/rotate-key", post(sites::rotate_key))
        .route(
            "/sites/{id}/installation-tests",
            post(collection::create_test),
        )
        .route(
            "/sites/{id}/installation-tests/{token}",
            get(collection::receipt),
        )
        .route(
            "/sites/{id}/identity-secret",
            get(collection::identity_secret),
        )
        .route("/sites/{id}/overview", get(overview::overview))
        .route("/sites/{id}/people", get(people::list))
        .route("/sites/{id}/people/export", get(people::export))
        .route(
            "/sites/{id}/people/{pid}",
            get(people::detail).delete(people::remove),
        )
        .route("/sites/{id}/people/{pid}/events", get(people::events))
        .route("/sites/{id}/pages", get(pages::list))
        .route("/sites/{id}/labels", get(labels::list).post(labels::create))
        .route(
            "/sites/{id}/labels/{lid}",
            patch(labels::update).delete(labels::remove),
        )
        .route(
            "/sites/{id}/segments",
            get(segments::list).post(segments::create),
        )
        .route("/sites/{id}/segments/preview", post(segments::preview))
        .route(
            "/sites/{id}/segments/{sid}",
            get(segments::detail)
                .patch(segments::update)
                .delete(segments::remove),
        )
        .fallback(|| async { AppError::not_found("找不到這個 API") })
}

/// Fixed domain reuse path: no public tracing, no access routes, no network.
pub fn domain_router(state: AppState) -> Router {
    Router::new().nest("/api", api_routes()).with_state(state)
}

pub fn router(state: AppState) -> Router {
    let api = api_routes();

    // 前端是單頁應用：找不到的路徑一律回 index.html，交給前端路由。
    let static_dir = state.config.static_dir.clone();
    let spa = ServeDir::new(&static_dir).fallback(ServeFile::new(static_dir.join("index.html")));

    Router::new()
        .nest("/api", api)
        // 同源檢查只套在 API 上
        .route_layer(middleware::from_fn_with_state(state.clone(), origin_guard))
        // 追蹤資料本來就是從別的網站送過來的，所以放在同源檢查之後才加
        .route(
            "/collect",
            post(collect::collect).options(collect::preflight),
        )
        .route("/t.js", get(collect::tracker))
        .route("/tracker-config/{key}", get(collection::config))
        .fallback_service(spa)
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            HeaderValue::from_static("same-origin"),
        ))
        .layer(TraceLayer::new_for_http().make_span_with(
            |request: &axum::http::Request<axum::body::Body>| {
                let route = request
                    .extensions()
                    .get::<axum::extract::MatchedPath>()
                    .map(|path| path.as_str())
                    .unwrap_or("static");
                tracing::info_span!("http",method=%request.method(),route)
            },
        ))
        .with_state(state)
}

async fn meta(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "public_url": state.config.public_url,
        "retention_days": state.config.retention_days,
    }))
}
