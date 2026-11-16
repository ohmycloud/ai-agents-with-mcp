//! Chapter 6.9: request a user action through MCP elicitation.

use ai_agents_with_mcp::ch6_servers::serve_elicitation_stdio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    serve_elicitation_stdio().await
}
