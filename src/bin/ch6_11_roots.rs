//! Chapter 6.11: limit filesystem operations to client-provided roots.

use ai_agents_with_mcp::ch6_servers::serve_roots_stdio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    serve_roots_stdio().await
}
