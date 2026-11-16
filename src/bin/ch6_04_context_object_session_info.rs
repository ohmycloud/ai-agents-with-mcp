//! Chapter 6.4: inspect the connected client's session information.

use ai_agents_with_mcp::ch6_servers::serve_session_context_stdio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    serve_session_context_stdio().await
}
