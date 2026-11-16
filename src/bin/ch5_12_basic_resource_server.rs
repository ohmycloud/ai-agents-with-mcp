//! Chapter 5.12: serve a static text resource over MCP stdio.

use ai_agents_with_mcp::tutorial_servers::serve_basic_resource_stdio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    serve_basic_resource_stdio().await
}
