//! Chapter 5.5: run an MCP server with no tools, resources, or prompts.

use anyhow::Result;
use rmcp::{
    ServerHandler, ServiceExt,
    model::{Implementation, ServerCapabilities, ServerConfig},
    transport::stdio,
};

struct MinimalServer;

impl ServerHandler for MinimalServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().build())
            .with_server_info(Implementation::new("minimal-stdio-server", "0.1.0"))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let service = MinimalServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
