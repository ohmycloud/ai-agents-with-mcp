//! Chapter 5.2: expose and call tools through the low-level server API.

use ai_agents_with_mcp::tutorial_servers::serve_low_level_tool_stdio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    serve_low_level_tool_stdio().await
}
