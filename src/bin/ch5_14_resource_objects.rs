//! Chapter 5.14: combine a registered resource with a text/binary file template.

use ai_agents_with_mcp::tutorial_servers::ResourceObjectsServer;
use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};

#[tokio::main]
async fn main() -> Result<()> {
    let service = ResourceObjectsServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
