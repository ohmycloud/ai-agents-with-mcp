//! Chapter 5.8: serve static and parameterized prompts over MCP stdio.

use ai_agents_with_mcp::tutorial_servers::serve_simple_prompt_stdio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    serve_simple_prompt_stdio().await
}
