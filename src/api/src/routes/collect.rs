//! 收追蹤資料的公開網址：`POST /collect`。不用登入，別的網站的瀏覽器會直接送過來。
//! 追蹤程式碼在 `/t.js`。

use std::net::SocketAddr;

use axum::{
    body::to_bytes,
    extract::{ConnectInfo, Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::{
    config::origin_of,
    db,
    error::AppError,
    ingest::{self, truncate, Payload},
    policy::CollectionPolicy,
    routes::sites::parse_list,
    state::AppState,
};

/// 一次送來的內容上限。
const MAX_BODY_BYTES: usize = 64 * 1024;

const TRACKER: &str = include_str!("../tracker.js");

pub async fn tracker() -> Response {
    (
        [
            (
                header::CONTENT_TYPE,
                "application/javascript; charset=utf-8",
            ),
            (header::CACHE_CONTROL, "public, max-age=3600"),
            // 追蹤程式碼是給別的網站載入的
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
        ],
        TRACKER,
    )
        .into_response()
}

/// 不帶 cookie，所以任何來源都可以讀回應；要不要收是看網站設定的來源清單。
fn with_cors(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    response
}

pub async fn preflight() -> Response {
    with_cors(
        (
            StatusCode::NO_CONTENT,
            [
                (header::ACCESS_CONTROL_ALLOW_METHODS, "POST, OPTIONS"),
                (header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type"),
                (header::ACCESS_CONTROL_MAX_AGE, "86400"),
            ],
        )
            .into_response(),
    )
}

/// 訪客的 IP，只用來限制流量，不會存起來。前面有反向代理時（`CDP_TRUST_PROXY`）
/// 看 `X-Forwarded-For` 的最後一個，也就是代理自己加上去的那個；前面幾個是對方可以亂填的。
fn client_ip(state: &AppState, req: &Request) -> String {
    if state.config.trust_proxy {
        let forwarded = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit(',').next())
            .map(str::trim)
            .filter(|ip| !ip.is_empty());
        if let Some(ip) = forwarded {
            return truncate(ip, 60);
        }
    }
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip().to_string())
        .unwrap_or_default()
}

fn header_text(headers: &HeaderMap, name: header::HeaderName, max: usize) -> String {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|v| truncate(v.trim(), max))
        .unwrap_or_default()
}

fn failure(status: StatusCode, message: &str) -> Response {
    with_cors((status, Json(json!({ "error": message }))).into_response())
}

pub async fn collect(State(state): State<AppState>, req: Request) -> Result<Response, AppError> {
    let headers = req.headers().clone();
    let ip = client_ip(&state, &req);
    if !state.collects.allow(&ip) {
        return Ok(failure(StatusCode::TOO_MANY_REQUESTS, "送得太頻繁了"));
    }
    let Ok(body) = to_bytes(req.into_body(), MAX_BODY_BYTES).await else {
        return Ok(failure(StatusCode::PAYLOAD_TOO_LARGE, "內容太長了"));
    };
    // 追蹤程式碼用 text/plain 送（不會觸發 CORS 預檢），所以不看 Content-Type
    let Ok(mut payload) = serde_json::from_slice::<Payload>(&body) else {
        return Ok(failure(StatusCode::BAD_REQUEST, "內容不是合法的 JSON"));
    };

    let site: Option<(i64, String, bool, String, String)> =
        sqlx::query_as("SELECT id, allowed_origins, active, collection_policy, identity_secret FROM sites WHERE key = ?")
            .bind(&payload.key)
            .fetch_optional(&state.db)
            .await?;
    let Some((site_id, allowed_origins, active, raw_policy, secret)) = site else {
        return Ok(failure(StatusCode::NOT_FOUND, "沒有這個追蹤金鑰"));
    };
    if !active {
        return Ok(failure(StatusCode::FORBIDDEN, "這個網站已經停止收資料"));
    }
    // 沒有 Origin 的不是瀏覽器的跨站請求（伺服器之間、命令列工具）
    let origins = parse_list(&allowed_origins);
    if let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        let allowed = origins.is_empty() || origin_of(origin).is_some_and(|o| origins.contains(&o));
        if !allowed {
            return Ok(failure(StatusCode::FORBIDDEN, "不收從這個網站送來的資料"));
        }
    }

    let policy = CollectionPolicy::parse(&raw_policy);
    if policy.consent_required && payload.consent != Some(true) {
        return Ok(failure(StatusCode::FORBIDDEN, "需要訪客同意才能採集"));
    }
    if policy.respect_dnt && headers.get("dnt").is_some_and(|v| v == "1") {
        return Ok(failure(StatusCode::FORBIDDEN, "已尊重 Do Not Track"));
    }
    let mut verified = false;
    if let Some(who) = &payload.identify {
        if let Some(proof) = &who.proof {
            verified = super::collection::verify_identity(
                &secret,
                &payload.key,
                who.user_id.as_deref().unwrap_or("").trim(),
                proof,
            );
            if !verified {
                return Ok(failure(StatusCode::FORBIDDEN, "身分簽章無效或已過期"));
            }
        }
        if policy.require_signed_identity && !verified {
            return Ok(failure(
                StatusCode::FORBIDDEN,
                "此網站要求伺服器簽署的 user_id",
            ));
        }
    }
    let test_token = payload.test_token.clone();
    let had_data = !payload.events.is_empty() || payload.identify.is_some();
    policy.apply(&mut payload);
    if had_data && payload.events.is_empty() && payload.identify.is_none() {
        return Ok(with_cors(StatusCode::NO_CONTENT.into_response()));
    }
    let mut batch = match payload.clean() {
        Ok(batch) => batch,
        Err(message) => return Ok(failure(StatusCode::BAD_REQUEST, &message)),
    };
    if let Some(who) = &mut batch.identify {
        who.verified = verified;
    }
    let user_agent = header_text(&headers, header::USER_AGENT, 400);
    if let Some(token) = test_token {
        let receipt = json!({"site_id":site_id,"origin":header_text(&headers,header::ORIGIN,500),"events":batch.events,"identity":batch.identify.as_ref().map(|who|json!({"user_id":who.user_id,"email":who.email,"verified":who.verified})),"consent_checked":policy.consent_required,"bot":ingest::is_bot(&user_agent),"analytics_written":false});
        if !super::collection::store_receipt(&state, site_id, &token, receipt).await? {
            return Ok(failure(
                StatusCode::FORBIDDEN,
                "安裝測試已過期或不屬於這個網站",
            ));
        }
        return Ok(with_cors(StatusCode::NO_CONTENT.into_response()));
    }
    // 爬蟲：照樣回成功，但不存
    if !ingest::is_bot(&user_agent) {
        ingest::store(&state.db, site_id, &batch, &user_agent, &db::now()).await?;
    }
    Ok(with_cors(StatusCode::NO_CONTENT.into_response()))
}
