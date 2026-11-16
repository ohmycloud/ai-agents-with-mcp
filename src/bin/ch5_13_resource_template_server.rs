//! Chapter 5.13: serve resources addressed through a URI template.

use ai_agents_with_mcp::tutorial_servers::serve_resource_template_stdio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    serve_resource_template_stdio().await
}
