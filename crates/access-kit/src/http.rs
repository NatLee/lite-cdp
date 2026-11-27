use crate::{
    audit, credentials,
    crypto::{now, time},
    AccessError, AccessKit, Role, Transport,
};
use axum::{
    body::Bytes,
    extract::{ConnectInfo, DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Extension, Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use sqlx::Row;
use std::collections::HashMap;

pub(crate) fn router(kit: AccessKit) -> Router {
    Router::new()
        .route("/api/access/providers", get(providers))
        .route("/api/access/overview", get(overview))
        .route("/api/access/identities", get(identity_list))
        .route("/api/access/identities/{id}", delete(identity_delete))
        .route(
            "/api/access/identities/{provider}/link",
            post(identity_link),
        )
        .route("/oauth/login/{provider}", get(provider_login))
        .route("/oauth/callback/{provider}", get(provider_callback))
        .route("/api/access/clients", get(clients_get).post(clients_post))
        .route("/api/access/grants", get(grant_list))
        .route("/api/access/grants/{id}", delete(grant_revoke))
        .route("/api/access/consents/{id}", get(consent_get))
        .route("/oauth/consent/{id}", post(consent_post))
        .route("/oauth/authorize", get(authorize))
        .route("/oauth/token", post(token))
        .route("/oauth/revoke", post(revoke))
        .route("/oauth/register", post(register))
        .route(
            "/.well-known/oauth-authorization-server",
            get(server_metadata),
        )
        .route("/.well-known/oauth-protected-resource", get(mcp_metadata))
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            get(mcp_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource/api/v1",
            get(api_metadata),
        )
        .route("/api/access/credentials", get(keys).post(create_key))
        .route("/api/access/credentials/{id}", delete(revoke_key))
        .route("/api/access/usage", get(usage))
        .route("/api/access/events", get(events))
        .route("/api/v1/openapi.json", get(openapi))
        .route("/api/v1/{operation}", post(operation))
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .layer(middleware::from_fn(no_cache))
        .with_state(kit)
}
pub(crate) fn parse_json<T: DeserializeOwned>(
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<T, AccessError> {
    if !headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(';').next() == Some("application/json"))
    {
        return Err(AccessError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "invalid_content_type",
            "Use application/json",
        ));
    }
    serde_json::from_slice(body)
        .map_err(|_| AccessError::bad("invalid_request", "Invalid JSON request"))
}
pub(crate) fn no_store(value: Value) -> Response {
    let mut response = Json(value).into_response();
    response.headers_mut().insert(
        "cache-control",
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}
async fn providers(State(kit): State<AccessKit>) -> Response {
    no_store(
        json!({"items":([("github","GitHub",kit.0.config.github.is_some()),("google","Google",kit.0.config.google.is_some())].map(|(provider,name,enabled)|json!({"provider":provider,"name":name,"enabled":enabled,"login_start_url":format!("/oauth/login/{provider}"),"disabled_reason":if enabled{None}else{Some("Provider credentials are not configured")}})))}),
    )
}
async fn keys(State(kit): State<AccessKit>, headers: HeaderMap) -> Result<Response, AccessError> {
    let s = credentials::cookie(&kit, &headers, false).await?;
    Ok(no_store(
        json!({"items":credentials::list(&kit,s.principal.user_id).await?}),
    ))
}
async fn create_key(
    State(kit): State<AccessKit>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, AccessError> {
    let s = credentials::cookie(&kit, &headers, true).await?;
    let value =
        credentials::create(&kit, s.principal.user_id, parse_json(&headers, &body)?).await?;
    let mut response = no_store(value);
    *response.status_mut() = StatusCode::CREATED;
    Ok(response)
}
async fn revoke_key(
    State(kit): State<AccessKit>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, AccessError> {
    let s = credentials::cookie(&kit, &headers, true).await?;
    credentials::revoke(&kit, &s.principal, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn operation(
    State(kit): State<AccessKit>,
    Path(name): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, AccessError> {
    let auth = match credentials::authenticate(&kit, &headers, Transport::Api).await {
        Ok(a) => a,
        Err(e) => {
            audit::security(
                &kit.0.db,
                Transport::Api,
                "unknown",
                e.status().as_u16(),
                None,
            )
            .await?;
            let mut response = e.into_response();
            if let Ok(challenge) = axum::http::HeaderValue::from_str(&format!(
                "Bearer resource_metadata=\"{}\"",
                kit.0
                    .config
                    .url("/.well-known/oauth-protected-resource/api/v1")
            )) {
                response.headers_mut().insert("www-authenticate", challenge);
            }
            return Ok(response);
        }
    };
    let id = auth.request_id.clone();
    let result = crate::dispatch::dispatch_result(
        &kit,
        auth,
        Transport::Api,
        &name,
        parse_json(&headers, &body),
    )
    .await;
    let mut response = match result {
        Ok(value) => no_store(value),
        Err(e) => e.into_response(),
    };
    response.headers_mut().insert(
        "x-request-id",
        axum::http::HeaderValue::from_str(&id).map_err(|_| AccessError::unavailable())?,
    );
    Ok(response)
}
async fn openapi(State(kit): State<AccessKit>) -> Json<Value> {
    let paths=kit.0.operations.values().map(|r|(format!("/api/v1/{}",r.operation.name),json!({"post":{"operationId":r.operation.name,"description":r.operation.description,"security":[{"bearer":[]}],"requestBody":{"required":true,"content":{"application/json":{"schema":r.operation.input_schema}}},"responses":{"200":{"description":"Authorized domain result"}},"x-required-scope":r.operation.required_scope}}))).collect::<serde_json::Map<_,_>>();
    Json(
        json!({"openapi":"3.1.0","info":{"title":"Authorized domain API","version":"0.1.0"},"paths":paths,"components":{"securitySchemes":{"bearer":{"type":"http","scheme":"bearer"}}}}),
    )
}
pub(crate) fn selected_user(
    principal: &crate::Principal,
    q: &HashMap<String, String>,
) -> Result<Option<i64>, AccessError> {
    match q.get("user_id").map(String::as_str) {
        None => Ok(Some(principal.user_id)),
        Some("all") => {
            if principal.role == Role::Admin {
                Ok(None)
            } else {
                Err(AccessError::denied())
            }
        }
        Some(value) => {
            let id = value
                .parse::<i64>()
                .ok()
                .filter(|i| *i > 0)
                .ok_or_else(|| AccessError::bad("invalid_user", "Invalid user filter"))?;
            if id != principal.user_id && principal.role != Role::Admin {
                return Err(AccessError::denied());
            }
            Ok(Some(id))
        }
    }
}
pub(crate) async fn usage_value(
    kit: &AccessKit,
    user: Option<i64>,
    q: &HashMap<String, String>,
) -> Result<Value, AccessError> {
    let days = q
        .get("days")
        .map(|d| d.parse::<i64>())
        .transpose()
        .map_err(|_| AccessError::bad("invalid_days", "Days must be 7, 30 or 90"))?
        .unwrap_or(7);
    if ![7, 30, 90].contains(&days) {
        return Err(AccessError::bad("invalid_days", "Days must be 7, 30 or 90"));
    }
    let transport = transport_filter(q)?;
    let to = (now() / 86400 + 1) * 86400;
    let from = to - days * 86400;
    let min = audit::day(from);
    let max = audit::day(to);
    let rows=sqlx::query("SELECT day,SUM(attempts) attempts,SUM(successes) successes,SUM(failures) failures,SUM(pending) pending FROM access_daily WHERE day>=? AND day<? AND (? IS NULL OR user_id=?) AND (? IS NULL OR transport=?) GROUP BY day").bind(min).bind(max).bind(user).bind(user).bind(transport).bind(transport).fetch_all(&kit.0.db).await?;
    let mut total = [0i64; 4];
    let mut daily = Vec::new();
    for d in 0..days {
        let date = audit::day(from + d * 86400);
        let mut counts = [0i64; 4];
        if let Some(row) = rows.iter().find(|r| r.get::<String, _>("day") == date) {
            for (i, key) in ["attempts", "successes", "failures", "pending"]
                .iter()
                .enumerate()
            {
                counts[i] = row.get(*key);
            }
        }
        for i in 0..4 {
            total[i] += counts[i];
        }
        daily.push(json!({"date":date,"attempts":counts[0],"successes":counts[1],"failures":counts[2],"pending":counts[3]}));
    }
    Ok(
        json!({"days":days,"from":time(from),"to":time(to),"timezone":"UTC","transport":transport,"totals":{"attempts":total[0],"successes":total[1],"failures":total[2],"pending":total[3]},"daily":daily}),
    )
}
fn transport_filter(q: &HashMap<String, String>) -> Result<Option<&str>, AccessError> {
    match q.get("transport").map(String::as_str) {
        None => Ok(None),
        Some(t @ ("api" | "mcp")) => Ok(Some(t)),
        _ => Err(AccessError::bad("invalid_transport", "Invalid transport")),
    }
}
async fn usage(
    State(kit): State<AccessKit>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Response, AccessError> {
    let s = credentials::cookie(&kit, &headers, false).await?;
    Ok(no_store(
        usage_value(&kit, selected_user(&s.principal, &q)?, &q).await?,
    ))
}
async fn events(
    State(kit): State<AccessKit>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Response, AccessError> {
    let s = credentials::cookie(&kit, &headers, false).await?;
    let user = selected_user(&s.principal, &q)?;
    let admin = s.principal.role == Role::Admin;
    let transport = transport_filter(&q)?;
    let limit = q
        .get("limit")
        .map(|n| n.parse::<i64>())
        .transpose()
        .map_err(|_| AccessError::bad("invalid_limit", "Invalid limit"))?
        .unwrap_or(25);
    if !(1..=100).contains(&limit) {
        return Err(AccessError::bad("invalid_limit", "Limit must be 1 to 100"));
    }
    let cursor = match q.get("cursor") {
        None => i64::MAX,
        Some(c) => {
            if c.len() > 64 {
                return Err(AccessError::bad("invalid_cursor", "Invalid cursor"));
            }
            let raw = URL_SAFE_NO_PAD
                .decode(c)
                .map_err(|_| AccessError::bad("invalid_cursor", "Invalid cursor"))?;
            String::from_utf8(raw)
                .ok()
                .and_then(|v| v.parse::<i64>().ok())
                .filter(|v| *v > 0)
                .ok_or_else(|| AccessError::bad("invalid_cursor", "Invalid cursor"))?
        }
    };
    let outcome = q.get("outcome").map(String::as_str);
    if outcome.is_some_and(|v| !["pending", "success", "failure", "interrupted"].contains(&v)) {
        return Err(AccessError::bad("invalid_outcome", "Invalid outcome"));
    }
    let rows=sqlx::query("SELECT * FROM access_events WHERE id<? AND (? IS NULL OR user_id=?) AND (? OR kind!='security') AND (? IS NULL OR transport=?) AND (? IS NULL OR outcome=?) ORDER BY id DESC LIMIT ?").bind(cursor).bind(user).bind(user).bind(admin).bind(transport).bind(transport).bind(outcome).bind(outcome).bind(limit+1).fetch_all(&kit.0.db).await?;
    let has_more = rows.len() > limit as usize;
    let rows = &rows[..rows.len().min(limit as usize)];
    let next = if has_more {
        rows.last()
            .map(|r| URL_SAFE_NO_PAD.encode(r.get::<i64, _>("id").to_string()))
    } else {
        None
    };
    let items=rows.iter().map(|r|json!({"id":r.get::<i64,_>("id").to_string(),"request_id":r.get::<String,_>("request_id"),"at":time(r.get("at")),"kind":r.get::<String,_>("kind"),"transport":r.get::<String,_>("transport"),"operation":r.get::<String,_>("operation"),"user_id":r.get::<Option<i64>,_>("user_id"),"client_id":r.get::<Option<String>,_>("client_id"),"credential_id":r.get::<Option<String>,_>("credential_id"),"status":r.get::<i64,_>("status"),"outcome":r.get::<String,_>("outcome"),"duration_ms":r.get::<i64,_>("duration_ms")})).collect::<Vec<_>>();
    Ok(no_store(json!({"items":items,"next_cursor":next})))
}

async fn clients_get(State(kit): State<AccessKit>, h: HeaderMap) -> Result<Response, AccessError> {
    let s = credentials::cookie(&kit, &h, false).await?;
    if s.principal.role != Role::Admin {
        return Err(AccessError::denied());
    }
    Ok(no_store(crate::clients::list(&kit).await?))
}
async fn clients_post(
    State(kit): State<AccessKit>,
    h: HeaderMap,
    body: Bytes,
) -> Result<Response, AccessError> {
    let s = credentials::cookie(&kit, &h, true).await?;
    if s.principal.role != Role::Admin {
        return Err(AccessError::denied());
    }
    let mut response = no_store(crate::clients::register(&kit, parse_json(&h, &body)?).await?);
    *response.status_mut() = StatusCode::CREATED;
    Ok(response)
}
async fn register(
    State(kit): State<AccessKit>,
    peer: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    h: HeaderMap,
    body: Bytes,
) -> Response {
    let result = async {
        kit.rate(
            format!(
                "dcr:{}",
                peer.as_ref()
                    .map(|p| p.0 .0.ip().to_string())
                    .unwrap_or_else(|| "unknown-peer".into())
            ),
            10,
        )?;
        crate::clients::register(&kit, parse_json(&h, &body)?).await
    }
    .await;
    oauth_response(result, true)
}
async fn authorize(
    State(kit): State<AccessKit>,
    h: HeaderMap,
    params: Result<
        Query<crate::authorization::Authorize>,
        axum::extract::rejection::QueryRejection,
    >,
) -> Result<Response, AccessError> {
    let Query(b) =
        params.map_err(|_| AccessError::bad("invalid_request", "Invalid authorization request"))?;
    crate::authorization::start(&kit, &h, b).await
}
async fn consent_get(
    State(kit): State<AccessKit>,
    Path(id): Path<String>,
    h: HeaderMap,
) -> Result<Response, AccessError> {
    Ok(no_store(
        crate::authorization::details(&kit, &id, &h).await?,
    ))
}
async fn consent_post(
    State(kit): State<AccessKit>,
    Path(id): Path<String>,
    h: HeaderMap,
    b: Bytes,
) -> Result<Response, AccessError> {
    crate::authorization::consent(&kit, &id, &h, &b).await
}
fn form(h: &HeaderMap, b: &Bytes) -> Result<HashMap<String, String>, AccessError> {
    if !h
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(';').next() == Some("application/x-www-form-urlencoded"))
    {
        return Err(AccessError::bad(
            "invalid_request",
            "Use application/x-www-form-urlencoded",
        ));
    }
    let pairs: Vec<(String, String)> = serde_urlencoded::from_bytes(b)
        .map_err(|_| AccessError::bad("invalid_request", "Invalid form"))?;
    let mut result = HashMap::new();
    for (k, v) in pairs {
        if result.insert(k, v).is_some() {
            return Err(AccessError::bad(
                "invalid_request",
                "Duplicate form parameter",
            ));
        }
    }
    Ok(result)
}
fn oauth_response(result: Result<Value, AccessError>, created: bool) -> Response {
    let mut r = match result {
        Ok(v) => {
            let mut r = no_store(v);
            if created {
                *r.status_mut() = StatusCode::CREATED;
            }
            r
        }
        Err(e) => {
            let mut r = no_store(
                json!({"error":e.code(),"error_description":"The access request could not be completed"}),
            );
            *r.status_mut() = e.status();
            r
        }
    };
    r.headers_mut().insert(
        "cache-control",
        axum::http::HeaderValue::from_static("no-store"),
    );
    r
}
async fn token(State(kit): State<AccessKit>, h: HeaderMap, b: Bytes) -> Response {
    oauth_response(
        async { crate::tokens::exchange(&kit, form(&h, &b)?).await }.await,
        false,
    )
}
async fn revoke(State(kit): State<AccessKit>, h: HeaderMap, b: Bytes) -> Response {
    oauth_response(
        async {
            crate::tokens::revoke_token(&kit, &form(&h, &b)?).await?;
            Ok(json!({}))
        }
        .await,
        false,
    )
}
async fn grant_list(State(kit): State<AccessKit>, h: HeaderMap) -> Result<Response, AccessError> {
    let s = credentials::cookie(&kit, &h, false).await?;
    Ok(no_store(
        json!({"items":crate::tokens::grants(&kit,s.principal.user_id).await?}),
    ))
}
async fn grant_revoke(
    State(kit): State<AccessKit>,
    Path(id): Path<String>,
    h: HeaderMap,
) -> Result<StatusCode, AccessError> {
    let s = credentials::cookie(&kit, &h, true).await?;
    crate::tokens::revoke_grant(&kit, &s.principal, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn server_metadata(State(kit): State<AccessKit>) -> Json<Value> {
    Json(
        json!({"issuer":kit.0.config.origin.origin().ascii_serialization(),"authorization_endpoint":kit.0.config.url("/oauth/authorize"),"token_endpoint":kit.0.config.url("/oauth/token"),"revocation_endpoint":kit.0.config.url("/oauth/revoke"),"registration_endpoint":kit.0.config.url("/oauth/register"),"response_types_supported":["code"],"grant_types_supported":["authorization_code","refresh_token"],"token_endpoint_auth_methods_supported":["none"],"code_challenge_methods_supported":["S256"],"scopes_supported":kit.scopes()}),
    )
}
async fn mcp_metadata(State(kit): State<AccessKit>) -> Json<Value> {
    resource_metadata(&kit, "/mcp")
}
async fn api_metadata(State(kit): State<AccessKit>) -> Json<Value> {
    resource_metadata(&kit, "/api/v1")
}
fn resource_metadata(kit: &AccessKit, path: &str) -> Json<Value> {
    Json(
        json!({"resource":kit.0.config.url(path),"authorization_servers":[kit.0.config.origin.origin().ascii_serialization()],"scopes_supported":kit.scopes(),"bearer_methods_supported":["header"]}),
    )
}
async fn identity_list(
    State(kit): State<AccessKit>,
    h: HeaderMap,
) -> Result<Response, AccessError> {
    let s = credentials::cookie(&kit, &h, false).await?;
    Ok(no_store(
        json!({"items":crate::providers::identities(&kit,s.principal.user_id).await?}),
    ))
}
async fn identity_delete(
    State(kit): State<AccessKit>,
    Path(id): Path<String>,
    h: HeaderMap,
) -> Result<StatusCode, AccessError> {
    let s = credentials::cookie(&kit, &h, true).await?;
    crate::providers::unlink(&kit, s.principal.user_id, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn identity_link(
    State(kit): State<AccessKit>,
    Path(provider): Path<String>,
    h: HeaderMap,
    b: Bytes,
) -> Result<Response, AccessError> {
    if !b.is_empty() && b.as_ref() != b"{}" {
        return Err(AccessError::bad(
            "invalid_request",
            "Link does not accept account parameters",
        ));
    }
    crate::providers::start(&kit, &provider, &h, true, None).await
}
async fn provider_login(
    State(kit): State<AccessKit>,
    Path(provider): Path<String>,
    h: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Response, AccessError> {
    match crate::providers::start(
        &kit,
        &provider,
        &h,
        false,
        q.get("next").map(String::as_str).filter(|s| !s.is_empty()),
    )
    .await
    {
        Ok(r) => Ok(r),
        Err(e) => crate::providers::failure_redirect(&kit, kit.0.host.paths().login, e.code()),
    }
}
async fn provider_callback(
    State(kit): State<AccessKit>,
    Path(provider): Path<String>,
    h: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Response, AccessError> {
    match crate::providers::callback(&kit, &provider, &h, &q).await {
        Ok(r) => Ok(r),
        Err(e) => crate::providers::failure_redirect(&kit, kit.0.host.paths().login, e.code()),
    }
}
async fn overview(State(kit): State<AccessKit>, h: HeaderMap) -> Result<Response, AccessError> {
    let s = credentials::cookie(&kit, &h, false).await?;
    let user = s.principal.user_id;
    let usage = usage_value(&kit, Some(user), &HashMap::new()).await?;
    let mut summary = usage["totals"].clone();
    for name in ["days", "from", "to", "timezone"] {
        summary[name] = usage[name].clone();
    }
    Ok(no_store(
        json!({"identities":crate::providers::identities(&kit,user).await?,"credentials":credentials::list(&kit,user).await?,"grants":crate::tokens::grants(&kit,user).await?,"summary":summary}),
    ))
}

async fn no_cache(request: axum::extract::Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        "cache-control",
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}
