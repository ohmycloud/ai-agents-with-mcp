//! Chapter 6.10: request model sampling from the connected host.

use ai_agents_with_mcp::tutorial_servers::serve_sampling_stdio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    serve_sampling_stdio().await
}
