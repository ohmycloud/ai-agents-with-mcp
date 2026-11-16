//! Small Streamable HTTP server paired with the Chapter 3 HTTP clients.

use std::net::SocketAddr;

use ai_agents_with_mcp::content_server::ContentServer;
use anyhow::Result;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio_util::sync::CancellationToken;

const ADDRESS: &str = "127.0.0.1:8000";

#[tokio::main]
async fn main() -> Result<()> {
    let cancellation = CancellationToken::new();
    let service = StreamableHttpService::new(
        || Ok(ContentServer),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default().with_cancellation_token(cancellation.child_token()),
    );
    let app = axum::Router::new().nest_service("/mcp", service);
    let address: SocketAddr = ADDRESS.parse()?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("Streamable HTTP MCP server listening at http://{ADDRESS}/mcp");

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            cancellation.cancel();
        })
        .await?;

    Ok(())
}
