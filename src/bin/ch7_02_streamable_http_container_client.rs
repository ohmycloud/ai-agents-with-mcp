//! Chapter 7.2: connect to an MCP server published from a container.

use anyhow::{Context, Result};
use rmcp::{ServiceExt, transport::StreamableHttpClientTransport};

#[tokio::main]
async fn main() -> Result<()> {
    let endpoint =
        std::env::var("MCP_SERVER_URL").unwrap_or_else(|_| "http://localhost:8000/mcp".to_owned());
    let client =
        ().serve(StreamableHttpClientTransport::from_uri(endpoint.clone()))
            .await
            .context("could not connect to the remote MCP server")?;
    println!("Connected to {endpoint}.");
    println!("Tools: {:#?}", client.list_all_tools().await?);
    client.cancel().await?;
    Ok(())
}
