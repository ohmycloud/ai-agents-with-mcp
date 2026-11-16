//! Chapter 6.7: manually notify clients that the tool list changed.

use ai_agents_with_mcp::ch6_servers::serve_notifications_stdio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    serve_notifications_stdio().await
}
