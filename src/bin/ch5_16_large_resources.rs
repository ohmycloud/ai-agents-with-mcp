//! Chapter 5.16: point to large exports and serve bounded log slices.

use ai_agents_with_mcp::tutorial_servers::serve_large_resource_stdio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    serve_large_resource_stdio().await
}
