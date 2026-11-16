//! Chapter 6.2: advertise an icon in server implementation metadata.

use ai_agents_with_mcp::ch6_servers::serve_icon_stdio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    serve_icon_stdio().await
}
