//! Chapter 7.4: serve a stateless calculator over Streamable HTTP.

use std::net::SocketAddr;

use ai_agents_with_mcp::deployment_calculator::DeploymentCalculator;
use anyhow::Result;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    let address: SocketAddr = std::env::var("MCP_BIND_ADDRESS")
        .unwrap_or_else(|_| "127.0.0.1:8000".to_owned())
        .parse()?;
    let cancellation = CancellationToken::new();
    let service = StreamableHttpService::new(
        || Ok(DeploymentCalculator),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true)
            .with_cancellation_token(cancellation.child_token()),
    );
    let app = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("Stateless calculator MCP server listening at http://{address}/mcp");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            cancellation.cancel();
        })
        .await?;
    Ok(())
}
