//! Chapter 5.1: the smallest explicit MCP stdio server handler.

use anyhow::Result;
use rmcp::{
    ServerHandler, ServiceExt,
    model::{Implementation, ServerCapabilities, ServerConfig},
    transport::stdio,
};

struct EmptyServer;

impl ServerHandler for EmptyServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().build())
            .with_server_info(Implementation::new("low-level-server", "0.1.0"))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    eprintln!("Running the low-level example MCP server");
    let service = EmptyServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
