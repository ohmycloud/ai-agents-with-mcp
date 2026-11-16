//! Chapter 3.6: connect over Streamable HTTP, list tools, and close the session.

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
    for tool in client.list_all_tools().await? {
        println!(
            "{}: {}",
            tool.name,
            tool.description.as_deref().unwrap_or("")
        );
    }

    client.cancel().await?;
    println!("Disconnected.");
    Ok(())
}
