//! Runtime logging deliberately suppresses SDK payload targets at every level.
use tracing::Metadata;
use tracing_subscriber::{
    filter::filter_fn, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

/// An explicit RUST_LOG directive must never re-enable SDK request/result bodies.
pub fn allow_log_metadata(metadata: &Metadata<'_>) -> bool {
    !metadata.target().starts_with("rmcp")
}

pub fn init() {
    let environment =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn"));
    tracing_subscriber::registry()
        .with(environment)
        .with(tracing_subscriber::fmt::layer().with_filter(filter_fn(allow_log_metadata)))
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        sync::{Arc, Mutex},
    };
    use tracing_subscriber::{filter::filter_fn, layer::SubscriberExt, Layer};
    #[derive(Clone)]
    struct Capture(Arc<Mutex<Vec<u8>>>);
    impl Write for Capture {
        fn write(&mut self, value: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(value);
            Ok(value.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn explicit_trace_directives_cannot_enable_sdk_payload_logging() {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let output = bytes.clone();
        let formatting = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(move || Capture(output.clone()))
            .with_filter(filter_fn(allow_log_metadata));
        let subscriber = tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new(
                "trace,rmcp=trace,rmcp::service=trace",
            ))
            .with(formatting);
        tracing::subscriber::with_default(subscriber, || {
            tracing::debug!(target:"rmcp::service",payload="private-query-marker",token="test-secret-marker");
            tracing::info!(target:"rmcp",message="notification-secret-marker");
            tracing::info!(target:"host_audit",operation="list_sites",status=200);
        });
        let text = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
        assert!(text.contains("list_sites"));
        assert!(!text.contains("private-query-marker"));
        assert!(!text.contains("test-secret-marker"));
        assert!(!text.contains("notification-secret-marker"));
    }
    #[tokio::test(flavor = "current_thread")]
    async fn public_route_trace_uses_templates_without_secret_path_or_query() {
        use tower::ServiceExt;
        use tracing_subscriber::fmt::format::FmtSpan;
        let directory = tempfile::TempDir::new().unwrap();
        let state = crate::build_state(crate::Config {
            bind: "127.0.0.1:0".into(),
            database_path: directory.path().join("trace.db"),
            static_dir: directory.path().join("web"),
            public_url: Some("http://127.0.0.1:8787".into()),
            public_origin: Some("http://127.0.0.1:8787".into()),
            cookie_secure: false,
            trust_proxy: false,
            rate_limit: 0,
            retention_days: 0,
        })
        .await
        .unwrap();
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let output = bytes.clone();
        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
                .with_writer(move || Capture(output.clone()))
                .with_filter(filter_fn(allow_log_metadata)),
        );
        let _guard = tracing::subscriber::set_default(subscriber);
        let response = crate::app(state)
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/sites/987654321?secret=QUERY-TRACE-CANARY")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(!response.status().is_server_error());
        let text = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
        assert!(
            text.contains("/api/sites/{id}"),
            "matched route template must remain observable"
        );
        for canary in ["FORM-KEY-TRACE-CANARY", "QUERY-TRACE-CANARY", "987654321"] {
            assert!(
                !text.contains(canary),
                "raw path/query appeared in captured logs"
            );
        }
    }
}
