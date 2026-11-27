mod support;
use access_kit::*;
use rmcp::{
    model::{CallToolRequestParams, ProtocolVersion},
    transport::{
        streamable_http_client::StreamableHttpClientTransportConfig, StreamableHttpClientTransport,
    },
    ClientLifecycleMode, ClientServiceExt,
};
use serde_json::{json, Value};
use std::future::IntoFuture;
#[tokio::test]
async fn real_sdk_discovery_call_counts_once_and_revocation_is_immediate() {
    let db = support::database().await;
    let host = support::TestHost::new();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let kit = AccessKit::new(db, AccessConfig::for_origin(&origin).unwrap(), host.clone())
        .await
        .unwrap();
    let server = tokio::spawn(axum::serve(listener, kit.router()).into_future());
    let http = reqwest::Client::new();
    let response = http
        .post(format!("{origin}/api/access/credentials"))
        .header("cookie", "session=owner")
        .json(&json!({"name":"sdk","scopes":["reports:read"],"audiences":["mcp"]}))
        .send()
        .await
        .unwrap();
    let key: Value = response.json().await.unwrap();
    let token = key["token"].as_str().unwrap();
    let config =
        StreamableHttpClientTransportConfig::with_uri(format!("{origin}/mcp")).auth_header(token);
    let client = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        ().serve_with_lifecycle(
            StreamableHttpClientTransport::from_config(config),
            ClientLifecycleMode::Discover {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            },
        ),
    )
    .await
    .unwrap()
    .expect("real SDK discover");
    assert_eq!(client.list_tools(None).await.unwrap().tools.len(), 1);
    let result = client
        .call_tool(
            CallToolRequestParams::new("read_report")
                .with_arguments(json!({"value":8}).as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    let result = serde_json::to_value(result).unwrap();
    assert_eq!(result["structuredContent"]["user_id"], 1, "{result}");
    let usage: Value = http
        .get(format!("{origin}/api/access/usage?transport=mcp"))
        .header("cookie", "session=owner")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(usage["totals"]["attempts"], 1);
    assert_eq!(usage["totals"]["successes"], 1);
    http.delete(format!(
        "{origin}/api/access/credentials/{}",
        key["credential"]["id"].as_str().unwrap()
    ))
    .header("cookie", "session=owner")
    .send()
    .await
    .unwrap()
    .error_for_status()
    .unwrap();
    assert!(client
        .call_tool(CallToolRequestParams::new("read_report"))
        .await
        .is_err());
    assert_eq!(host.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    let _ = client.cancel().await;
    server.abort();
}
#[tokio::test]
async fn malformed_tools_call_before_sdk_handler_counts_failed_attempt() {
    let db = support::database().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let kit = AccessKit::new(
        db,
        AccessConfig::for_origin(&origin).unwrap(),
        support::TestHost::new(),
    )
    .await
    .unwrap();
    let server = tokio::spawn(axum::serve(listener, kit.router()).into_future());
    let http = reqwest::Client::new();
    let key: Value = http
        .post(format!("{origin}/api/access/credentials"))
        .header("cookie", "session=owner")
        .json(&json!({"name":"malformed","scopes":["reports:read"]}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let r=http.post(format!("{origin}/mcp")).bearer_auth(key["token"].as_str().unwrap()).header("MCP-Protocol-Version","2026-07-28").header("Mcp-Method","tools/call").header("Accept","application/json, text/event-stream").json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"read_report","arguments":[],"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}})).send().await.unwrap();
    assert!(
        r.status().is_client_error() || r.json::<Value>().await.unwrap().get("error").is_some()
    );
    let usage: Value = http
        .get(format!("{origin}/api/access/usage?transport=mcp"))
        .header("cookie", "session=owner")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(usage["totals"]["attempts"], 1);
    assert_eq!(usage["totals"]["failures"], 1);
    server.abort();
}
#[tokio::test]
async fn legacy_version_missing_bearer_and_rebinding_do_not_execute() {
    let db = support::database().await;
    let host = support::TestHost::new();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let kit = AccessKit::new(db, AccessConfig::for_origin(&origin).unwrap(), host.clone())
        .await
        .unwrap();
    let server = tokio::spawn(axum::serve(listener, kit.router()).into_future());
    let http = reqwest::Client::new();
    let missing = http
        .post(format!("{origin}/mcp"))
        .json(&json!({"method":"server/discover"}))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert!(missing
        .headers()
        .get("www-authenticate")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("/.well-known/oauth-protected-resource/mcp"));
    let key: Value = http
        .post(format!("{origin}/api/access/credentials"))
        .header("cookie", "session=owner")
        .json(&json!({"name":"versions","scopes":["reports:read"]}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = key["token"].as_str().unwrap();
    let legacy = http
        .post(format!("{origin}/mcp"))
        .bearer_auth(token)
        .header("MCP-Protocol-Version", "2025-11-25")
        .header("Accept", "application/json,text/event-stream")
        .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
        .send()
        .await
        .unwrap();
    assert!(
        legacy.status().is_client_error()
            || legacy.json::<Value>().await.unwrap().get("error").is_some()
    );
    let rebound = http
        .post(format!("{origin}/mcp"))
        .bearer_auth(token)
        .header("host", "evil.example")
        .header("Accept", "application/json,text/event-stream")
        .json(&json!({"jsonrpc":"2.0","id":1,"method":"server/discover"}))
        .send()
        .await
        .unwrap();
    assert_eq!(rebound.status(), reqwest::StatusCode::FORBIDDEN);
    assert_eq!(host.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    server.abort();
}
#[tokio::test]
async fn same_name_different_schemas_are_isolated_between_real_sdk_servers() {
    let mut servers = Vec::new();
    let mut clients = Vec::new();
    for variant in [0, 4] {
        let db = support::database().await;
        let host = support::TestHost::new();
        host.variant
            .store(variant, std::sync::atomic::Ordering::SeqCst);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let kit = AccessKit::new(db, AccessConfig::for_origin(&origin).unwrap(), host)
            .await
            .unwrap();
        servers.push(tokio::spawn(
            axum::serve(listener, kit.router()).into_future(),
        ));
        let key: Value = reqwest::Client::new()
            .post(format!("{origin}/api/access/credentials"))
            .header("cookie", "session=owner")
            .json(&json!({"name":"schema","scopes":["reports:read"]}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let transport = StreamableHttpClientTransport::from_config(
            StreamableHttpClientTransportConfig::with_uri(format!("{origin}/mcp"))
                .auth_header(key["token"].as_str().unwrap()),
        );
        clients.push(
            ().serve_with_lifecycle(
                transport,
                ClientLifecycleMode::Discover {
                    preferred_versions: vec![ProtocolVersion::V_2026_07_28],
                },
            )
            .await
            .unwrap(),
        );
    }
    for (index, value) in [json!(5), json!("five")].into_iter().enumerate() {
        let result = clients[index]
            .call_tool(
                CallToolRequestParams::new("read_report")
                    .with_arguments(json!({"value":value}).as_object().unwrap().clone()),
            )
            .await
            .unwrap();
        let result = serde_json::to_value(result).unwrap();
        assert_eq!(
            result["structuredContent"]["args"]["value"], value,
            "{result}"
        );
    }
    for client in clients {
        let _ = client.cancel().await;
    }
    for server in servers {
        server.abort();
    }
}
