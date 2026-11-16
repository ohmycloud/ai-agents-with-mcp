//! Chapter 5.11: serve a resource and a prompt from the same MCP server.

use ai_agents_with_mcp::content_server::ResourcePromptServer;
use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};

#[tokio::main]
async fn main() -> Result<()> {
    let service = ResourcePromptServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
