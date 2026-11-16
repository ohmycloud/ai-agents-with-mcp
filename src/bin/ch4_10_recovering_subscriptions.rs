//! Chapter 4.10: recover the current tool set after a server list-change event.

use std::{
    env,
    io::{self, Write},
    path::PathBuf,
};

use ai_agents_with_mcp::{
    agent_loop::{run_anthropic_tool_loop, to_model_tools},
    calculator_server::{Calculator, serve_stdio},
    client_callbacks::CalculatorClientHandler,
    config::{anthropic_client, anthropic_model, load_dotenv},
    interactive_agent::{collect_prompt, select_resources},
};
use anyhow::{Context, Result, ensure};
use axum::Router;
use rmcp::ServiceExt;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    if env::args().nth(1).as_deref() == Some("--stdio-server") {
        return serve_stdio().await;
    }

    load_dotenv();
    let provider = anthropic_client()?;
    let model_name = anthropic_model()?;
    let model = provider.completion(model_name.clone());
    let root = env::var_os("MCP_ROOT")
        .map(PathBuf::from)
        .unwrap_or(env::current_dir()?);
    let root = root
        .canonicalize()
        .context("MCP_ROOT must point to an existing directory")?;
    ensure!(root.is_dir(), "MCP_ROOT must point to a directory");

    let (endpoint, local_server) = match env::var("MCP_SERVER_URL") {
        Ok(endpoint) if !endpoint.trim().is_empty() => (endpoint, None),
        _ => {
            let (endpoint, cancellation, task) = start_local_server().await?;
            (endpoint, Some((cancellation, task)))
        }
    };
    let handler = CalculatorClientHandler::new(format!("file://{}", root.display()), model_name);
    let mcp = handler
        .serve(rmcp::transport::StreamableHttpClientTransport::from_uri(
            endpoint.clone(),
        ))
        .await
        .context("could not connect to the calculator MCP server")?;

    let mut resources = mcp.list_all_resources().await?;
    let mut prompts = mcp.list_all_prompts().await?;
    println!("Connected to {endpoint}");
    println!("Type 'refresh', 'prompts', or 'goodbye'.");

    loop {
        print!("You: ");
        io::stdout().flush()?;
        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break;
        }
        let input = input.trim();
        if input.eq_ignore_ascii_case("goodbye") {
            println!("AI Assistant: Goodbye!");
            break;
        }
        if input.is_empty() {
            continue;
        }

        // rmcp invalidates this cached result when tools/list_changed arrives.
        let available = mcp.list_all_tools().await?;
        if input.eq_ignore_ascii_case("refresh") {
            resources = mcp.list_all_resources().await?;
            prompts = mcp.list_all_prompts().await?;
            println!(
                "Refreshed {} resources and {} prompts.",
                resources.len(),
                prompts.len()
            );
            continue;
        }

        let prompt = if input.eq_ignore_ascii_case("prompts") {
            match collect_prompt(&mcp, &prompts).await? {
                Some(prompt) => prompt,
                None => continue,
            }
        } else {
            let context = select_resources(&model, &mcp, &resources, input).await;
            if context.is_empty() {
                input.to_owned()
            } else {
                format!("Question: {input}\n\nRelevant MCP resources:\n{context}")
            }
        };

        let tools = to_model_tools(&available)?;
        let answer = run_anthropic_tool_loop(&model, &mcp, &tools, prompt).await?;
        println!("AI Assistant: {answer}");
    }

    mcp.cancel().await?;
    if let Some((cancellation, task)) = local_server {
        cancellation.cancel();
        task.await??;
    }
    Ok(())
}

async fn start_local_server() -> Result<(String, CancellationToken, JoinHandle<std::io::Result<()>>)>
{
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let cancellation = CancellationToken::new();
    let server_cancellation = cancellation.clone();
    let calculator = Calculator::new();
    let service = rmcp::transport::streamable_http_server::StreamableHttpService::new(
        move || Ok(calculator.clone()),
        rmcp::transport::streamable_http_server::session::local::LocalSessionManager::default()
            .into(),
        rmcp::transport::streamable_http_server::StreamableHttpServerConfig::default()
            .with_json_response(true)
            .with_cancellation_token(cancellation.child_token()),
    );
    let app = Router::new().nest_service("/mcp", service);
    let task = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move { server_cancellation.cancelled().await })
            .await
    });
    Ok((format!("http://{address}/mcp"), cancellation, task))
}
