use crate::{audit, credentials, types::AuthContext, AccessError, AccessKit, Transport};
use axum::{
    body::{to_bytes, Body},
    extract::{Request, State},
    http::HeaderValue,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ListToolsResult,
        PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerConfig, Tool,
        ToolAnnotations,
    },
    service::{RequestContext, RoleServer},
    transport::streamable_http_server::{
        session::never::NeverSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    },
    ErrorData, ServerHandler,
};
use serde_json::Value;
use std::{borrow::Cow, sync::Arc};
#[derive(Clone)]
struct DomainServer {
    kit: AccessKit,
}
impl DomainServer {
    fn auth(ctx: &RequestContext<RoleServer>) -> Result<AuthContext, ErrorData> {
        ctx.extensions
            .get::<axum::http::request::Parts>()
            .and_then(|p| p.extensions.get::<AuthContext>())
            .cloned()
            .ok_or_else(|| {
                ErrorData::invalid_request("Authenticated request context is required", None)
            })
    }
    fn tool(&self, name: &str) -> Option<Tool> {
        let op = &self.kit.0.operations.get(name)?.operation;
        let mut tool = Tool::new(
            op.name,
            op.description,
            Arc::new(op.input_schema.as_object()?.clone()),
        );
        tool.annotations = Some(
            ToolAnnotations::new()
                .read_only(true)
                .destructive(false)
                .idempotent(true),
        );
        Some(tool)
    }
}
impl ServerHandler for DomainServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(&[ProtocolVersion::V_2026_07_28])
    }
    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tool(name)
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let auth = Self::auth(&ctx)?;
        Ok(ListToolsResult::with_all_items(
            self.kit
                .0
                .operations
                .values()
                .filter(|r| auth.scopes.iter().any(|s| s == r.operation.required_scope))
                .filter_map(|r| self.tool(r.operation.name))
                .collect(),
        ))
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let auth = Self::auth(&ctx)?;
        let args = Value::Object(request.arguments.unwrap_or_default());
        let result =
            crate::dispatch::dispatch(&self.kit, auth, Transport::Mcp, &request.name, args).await;
        Ok(match result {
            Ok(v) => CallToolResult::structured(v),
            Err(e) => CallToolResult::structured_error(e.json()),
        }
        .into())
    }
}
pub(crate) fn router(kit: AccessKit) -> Router {
    let config = &kit.0.config;
    let authority = match config.origin.port() {
        Some(p) => format!("{}:{p}", config.origin.host_str().unwrap_or("localhost")),
        None => config.origin.host_str().unwrap_or("localhost").to_owned(),
    };
    let origin = format!(
        "{}://{}:{}",
        config.origin.scheme(),
        config.origin.host_str().unwrap_or("localhost"),
        config.origin.port_or_known_default().unwrap_or(443)
    );
    let options = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_sse_keep_alive(None)
        .with_max_request_body_bytes(1024 * 1024)
        .with_stateless_protocol_metadata_required(true)
        .with_allowed_hosts([authority])
        .with_allowed_origins([origin])
        .enforce_origin_validation();
    let server_kit = kit.clone();
    let service = StreamableHttpService::new(
        move || {
            Ok(DomainServer {
                kit: server_kit.clone(),
            })
        },
        Arc::new(NeverSessionManager::default()),
        options,
    );
    Router::new()
        .route_service("/mcp", service)
        .route_layer(middleware::from_fn_with_state(kit, guard))
}
async fn guard(State(kit): State<AccessKit>, mut req: Request, next: Next) -> Response {
    if !credentials::same_origin(&kit, req.headers()) {
        return AccessError::denied().into_response();
    }
    match credentials::authenticate(&kit, req.headers(), Transport::Mcp).await {
        Ok(mut auth) => {
            let mut ticket = None;
            if req.method() == axum::http::Method::POST {
                let (parts, body) = req.into_parts();
                let bytes = match to_bytes(body, 1024 * 1024).await {
                    Ok(b) => b,
                    Err(_) => return axum::http::StatusCode::PAYLOAD_TOO_LARGE.into_response(),
                };
                if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
                    if value["method"] == "tools/call" {
                        let name = value["params"]["name"]
                            .as_str()
                            .and_then(|n| kit.0.operations.get(n))
                            .map(|r| r.operation.name)
                            .unwrap_or("unknown");
                        let id = match audit::reserve(&kit.0.db, &auth, Transport::Mcp, name).await
                        {
                            Ok(id) => id,
                            Err(e) => return e.into_response(),
                        };
                        let current = crate::types::AuditTicket {
                            id,
                            started: std::time::Instant::now(),
                        };
                        auth.audit_ticket = Some(current.clone());
                        ticket = Some(current);
                    }
                }
                req = Request::from_parts(parts, Body::from(bytes));
            }
            req.extensions_mut().insert(auth);
            let response = next.run(req).await;
            if let Some(ticket) = ticket {
                let status = response.status();
                let (parts, body) = response.into_parts();
                let bytes = match to_bytes(body, 4 * 1024 * 1024).await {
                    Ok(b) => b,
                    Err(_) => return AccessError::unavailable().into_response(),
                };
                let value = serde_json::from_slice::<Value>(&bytes).ok();
                let failed = !status.is_success()
                    || value
                        .as_ref()
                        .is_none_or(|v| v.get("error").is_some() || v["result"]["isError"] == true);
                let outcome = if !status.is_success() {
                    status.as_u16()
                } else if failed {
                    400
                } else {
                    200
                };
                if audit::finish(
                    &kit.0.db,
                    &ticket.id,
                    outcome,
                    ticket.started.elapsed().as_millis().min(i64::MAX as u128) as i64,
                    false,
                )
                .await
                .is_err()
                {
                    return AccessError::unavailable().into_response();
                }
                let mut response = Response::from_parts(parts, Body::from(bytes));
                if let Ok(value) = HeaderValue::from_str(&ticket.id) {
                    response.headers_mut().insert("x-request-id", value);
                }
                response
            } else {
                response
            }
        }
        Err(error) => {
            if audit::security(
                &kit.0.db,
                Transport::Mcp,
                "unknown",
                error.status().as_u16(),
                None,
            )
            .await
            .is_err()
            {
                return AccessError::unavailable().into_response();
            }
            let mut response = error.into_response();
            let challenge = format!(
                "Bearer resource_metadata=\"{}\"",
                kit.0
                    .config
                    .url("/.well-known/oauth-protected-resource/mcp")
            );
            if let Ok(v) = HeaderValue::from_str(&challenge) {
                response.headers_mut().insert("www-authenticate", v);
            }
            response
        }
    }
}
