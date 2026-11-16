//! Chapter 5.9: return a user request and assistant prefill from a prompt.

use ai_agents_with_mcp::tutorial_servers::serve_multiturn_prompt_stdio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    serve_multiturn_prompt_stdio().await
}
