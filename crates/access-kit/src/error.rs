use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
pub struct AccessError {
    status: StatusCode,
    code: &'static str,
    message: String,
}
impl AccessError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
    pub fn status(&self) -> StatusCode {
        self.status
    }
    pub fn code(&self) -> &'static str {
        self.code
    }
    pub(crate) fn bad(code: &'static str, message: &str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }
    pub(crate) fn denied() -> Self {
        Self::new(StatusCode::FORBIDDEN, "access_denied", "Access denied")
    }
    pub(crate) fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "Authentication required",
        )
    }
    pub(crate) fn unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "access_unavailable",
            "Access service is unavailable",
        )
    }
    pub(crate) fn json(&self) -> serde_json::Value {
        json!({"error":self.message,"code":self.code})
    }
}
impl std::fmt::Debug for AccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccessError")
            .field("status", &self.status)
            .field("code", &self.code)
            .finish()
    }
}
impl std::fmt::Display for AccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code)
    }
}
impl std::error::Error for AccessError {}
impl From<sqlx::Error> for AccessError {
    fn from(_: sqlx::Error) -> Self {
        Self::unavailable()
    }
}
impl IntoResponse for AccessError {
    fn into_response(self) -> Response {
        let mut response = (self.status, Json(self.json())).into_response();
        response.headers_mut().insert(
            "cache-control",
            axum::http::HeaderValue::from_static("no-store"),
        );
        response
    }
}
