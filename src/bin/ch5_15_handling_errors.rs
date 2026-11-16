//! Chapter 5.15: distinguish invalid tool input from successful tool results.

use ai_agents_with_mcp::tutorial_servers::serve_grade_stdio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    serve_grade_stdio().await
}
