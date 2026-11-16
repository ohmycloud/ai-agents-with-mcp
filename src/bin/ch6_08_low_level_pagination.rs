//! Chapter 6.8: list a thousand resources in bounded pages.

use ai_agents_with_mcp::ch6_servers::serve_pagination_stdio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    serve_pagination_stdio().await
}
