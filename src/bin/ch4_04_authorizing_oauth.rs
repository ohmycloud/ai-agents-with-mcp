//! Chapter 4.4: authorize an MCP client with OAuth authorization code + PKCE.

use std::sync::Arc;

use anyhow::{Context, Result, bail};
use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use rmcp::{
    ServiceExt,
    transport::{
        StreamableHttpClientTransport,
        auth::{AuthClient, AuthorizationRequest, OAuthState},
        streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde::Deserialize;
use tokio::sync::{Mutex, oneshot};

#[derive(Debug, Deserialize)]
struct OAuthCallback {
    code: Option<String>,
    state: Option<String>,
    iss: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let endpoint =
        std::env::var("MCP_SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:8000/mcp".to_owned());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let callback_address = listener.local_addr()?;
    let redirect_uri = format!("http://{callback_address}/oauth/callback");
    let (callback_sender, callback_receiver) = oneshot::channel();
    let callback_sender = Arc::new(Mutex::new(Some(callback_sender)));
    let app = Router::new()
        .route("/oauth/callback", get(receive_callback))
        .with_state(callback_sender);
    let (shutdown_sender, shutdown_receiver) = oneshot::channel();
    let callback_server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_receiver.await;
            })
            .await
    });

    let mut oauth = OAuthState::new(endpoint.clone(), None)
        .await
        .context("could not discover the MCP authorization server")?;
    oauth
        .start_authorization(
            AuthorizationRequest::new(redirect_uri)
                .with_client_name("AI Agents with MCP Rust")
                .with_scopes(["read", "write"]),
        )
        .await
        .context("could not start OAuth authorization")?;
    println!(
        "Authorize this client in your browser, then return to this terminal:\n{}",
        oauth.get_authorization_url().await?
    );

    let callback = callback_receiver
        .await
        .context("authorization callback listener stopped unexpectedly")?;
    let _ = shutdown_sender.send(());
    callback_server
        .await
        .context("callback listener task failed")??;
    if let Some(error) = callback.error {
        bail!(
            "authorization server rejected the request: {}",
            callback
                .error_description
                .unwrap_or_else(|| error.to_owned())
        );
    }
    let code = callback
        .code
        .context("authorization callback did not contain a code")?;
    let csrf_token = callback
        .state
        .context("authorization callback did not contain state")?;
    oauth
        .handle_callback_with_issuer(&code, &csrf_token, callback.iss.as_deref())
        .await
        .context("could not exchange the authorization code")?;
    let manager = oauth
        .into_authorization_manager()
        .context("OAuth authorization did not produce an authorized client")?;

    let transport = StreamableHttpClientTransport::<AuthClient<reqwest::Client>>::with_client(
        AuthClient::new(reqwest::Client::new(), manager),
        StreamableHttpClientTransportConfig::with_uri(endpoint.clone()),
    );
    let client = ().serve(transport).await?;
    println!("OAuth connection established with {endpoint}.");
    println!(
        "Server offers {} tool(s).",
        client.list_all_tools().await?.len()
    );
    client.cancel().await?;
    Ok(())
}

async fn receive_callback(
    State(sender): State<Arc<Mutex<Option<oneshot::Sender<OAuthCallback>>>>>,
    Query(callback): Query<OAuthCallback>,
) -> impl IntoResponse {
    if callback.code.is_none() && callback.error.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            "Missing authorization code or error",
        );
    }
    let Some(sender) = sender.lock().await.take() else {
        return (
            StatusCode::BAD_REQUEST,
            "Authorization callback was already used",
        );
    };
    if sender.send(callback).is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not deliver callback",
        );
    }
    (
        StatusCode::OK,
        "Authorization complete. Return to the terminal.",
    )
}
