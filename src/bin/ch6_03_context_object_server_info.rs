//! Chapter 6.3: inspect peer and server information from request context.

use ai_agents_with_mcp::ch6_servers::serve_server_context_stdio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    serve_server_context_stdio().await
}
