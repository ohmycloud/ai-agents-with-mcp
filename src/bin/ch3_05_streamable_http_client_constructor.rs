//! Chapter 3.5: construct a Streamable HTTP MCP client from an endpoint URL.

use std::env;

use anyhow::{Context, Result};
use rmcp::{ServiceExt, transport::StreamableHttpClientTransport};

#[tokio::main]
async fn main() -> Result<()> {
    let endpoint =
        env::var("MCP_SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:8000/mcp".to_owned());
    let transport = StreamableHttpClientTransport::from_uri(endpoint.clone());
    let client = ().serve(transport).await.context("could not connect to MCP endpoint")?;
    println!("Connected to {endpoint}");
    println!("Server info: {:#?}", client.peer_info());
    client.cancel().await?;
    Ok(())
}
