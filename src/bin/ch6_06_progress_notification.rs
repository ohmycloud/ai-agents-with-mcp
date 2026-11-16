//! Chapter 6.6: serve a long-running tool that reports progress.

use ai_agents_with_mcp::tutorial_servers::serve_long_progress_stdio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    serve_long_progress_stdio().await
}
