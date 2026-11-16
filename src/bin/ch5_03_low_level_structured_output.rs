//! Chapter 5.3: return structured JSON output from an MCP tool.

use ai_agents_with_mcp::tutorial_servers::serve_low_level_structured_output_stdio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    serve_low_level_structured_output_stdio().await
}
