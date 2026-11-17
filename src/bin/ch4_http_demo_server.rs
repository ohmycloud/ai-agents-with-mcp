//! Streamable HTTP calculator server used by Chapter 4.10.

use std::{env, net::SocketAddr};

use ai_agents_with_mcp::calculator_server::Calculator;
use anyhow::Result;
use axum::Router;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    let address: SocketAddr = env::var("MCP_BIND_ADDRESS")
        .unwrap_or_else(|_| "127.0.0.1:8000".to_owned())
        .parse()?;
    let cancellation = CancellationToken::new();
    let calculator = Calculator::new();
    let service = StreamableHttpService::new(
        move || Ok(calculator.clone()),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default()
            .with_json_response(true)
            .with_cancellation_token(cancellation.child_token()),
    );
    let app = Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("Streamable HTTP calculator server listening at http://{address}/mcp");

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            cancellation.cancel();
        })
        .await?;

    Ok(())
}
