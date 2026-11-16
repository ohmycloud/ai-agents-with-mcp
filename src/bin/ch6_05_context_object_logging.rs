//! Chapter 6.5: send logging notifications from a server tool.

use ai_agents_with_mcp::ch6_servers::serve_logging_stdio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    serve_logging_stdio().await
}
