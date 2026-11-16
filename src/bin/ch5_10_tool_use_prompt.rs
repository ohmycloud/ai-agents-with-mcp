//! Chapter 5.10: provide a prompt that guides the model toward a tool call.

use ai_agents_with_mcp::tutorial_servers::serve_tool_use_prompt_stdio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    serve_tool_use_prompt_stdio().await
}
