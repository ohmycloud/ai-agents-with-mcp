//! Chapter 7.3: protect an MCP HTTP endpoint with a bearer-token middleware.

use std::{net::SocketAddr, sync::Arc};

use anyhow::{Context, Result};
use axum::{
    Router,
    extract::{Request, State},
    http::{StatusCode, header::AUTHORIZATION},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use rmcp::{
    handler::server::wrapper::Parameters,
    schemars::JsonSchema,
    tool, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
struct ProtectedServer;

#[derive(Deserialize, JsonSchema)]
struct EchoRequest {
    request: String,
}

#[tool_router(server_handler)]
impl ProtectedServer {
    #[tool(description = "Uppercase a short request after bearer-token authorization.")]
    fn example_tool(&self, Parameters(input): Parameters<EchoRequest>) -> String {
        input.request.to_uppercase()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let token = std::env::var("MCP_RESOURCE_SERVER_TOKEN")
        .context("set MCP_RESOURCE_SERVER_TOKEN to the expected bearer token")?;
    let bind = std::env::var("MCP_BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:3001".to_owned());
    let address: SocketAddr = bind.parse().context("invalid MCP_BIND_ADDRESS")?;
    let cancellation = CancellationToken::new();
    let service = StreamableHttpService::new(
        || Ok(ProtectedServer),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default()
            .with_json_response(true)
            .with_cancellation_token(cancellation.child_token()),
    );
    let app = Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(
            Arc::new(token),
            require_bearer,
        ));
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("Authorized MCP resource server listening at http://{address}/mcp");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            cancellation.cancel();
        })
        .await?;
    Ok(())
}

async fn require_bearer(
    State(expected): State<Arc<String>>,
    request: Request,
    next: Next,
) -> Response {
    let accepted = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|provided| provided == expected.as_str());
    if accepted {
        next.run(request).await
    } else {
        (StatusCode::UNAUTHORIZED, "Bearer token required").into_response()
    }
}
