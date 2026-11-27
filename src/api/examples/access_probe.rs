//! Local acceptance probe: official SDK only; never print bearer or tool payloads.
use rmcp::{
    model::{CallToolRequestParams, ProtocolVersion},
    transport::{
        streamable_http_client::StreamableHttpClientTransportConfig, StreamableHttpClientTransport,
    },
    ClientLifecycleMode, ClientServiceExt,
};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::var("ACCESS_TEST_BASE")?;
    let token = std::env::var("ACCESS_TEST_TOKEN")?;
    let operation = std::env::var("ACCESS_TEST_OPERATION")?;
    let args: serde_json::Value = serde_json::from_str(&std::env::var("ACCESS_TEST_ARGUMENTS")?)?;
    let config =
        StreamableHttpClientTransportConfig::with_uri(format!("{base}/mcp")).auth_header(token);
    let client = ()
        .serve_with_lifecycle(
            StreamableHttpClientTransport::from_config(config),
            ClientLifecycleMode::Discover {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            },
        )
        .await?;
    let tools = client.list_tools(None).await?;
    if !tools.tools.iter().any(|tool| tool.name == operation) {
        return Err("Required tool unavailable".into());
    }
    let result = client
        .call_tool(
            CallToolRequestParams::new(operation)
                .with_arguments(args.as_object().ok_or("Expected object")?.clone()),
        )
        .await?;
    let value = serde_json::to_value(result)?;
    if value["isError"].as_bool().unwrap_or(false) {
        return Err("Domain operation denied".into());
    }
    client.cancel().await?;
    println!("Official MCP2026-07-28 discovery and tool call passed");
    Ok(())
}
