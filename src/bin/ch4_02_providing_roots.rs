//! Chapter 4.2: an interactive agent that provides a filesystem root.

use std::{env, process::Stdio};

use ai_agents_with_mcp::{
    calculator_server::serve_stdio,
    client_callbacks::{CalculatorClientHandler, CallbackFeatures},
    config::{anthropic_client, anthropic_model, load_dotenv, mcp_root, mcp_root_uri},
    interactive_agent::run_interactive_agent,
};
use anyhow::{Context, Result};
use rmcp::{ServiceExt, transport::TokioChildProcess};
use tokio::process::Command;

#[tokio::main]
async fn main() -> Result<()> {
    if env::args().nth(1).as_deref() == Some("--server") {
        return serve_stdio().await;
    }

    load_dotenv();
    let provider = anthropic_client()?;
    let model_name = anthropic_model()?;
    let model = provider.completion(model_name.clone());
    let root = mcp_root()?;
    let handler = CalculatorClientHandler::for_chapter(
        mcp_root_uri(&root)?,
        model_name,
        CallbackFeatures::SAMPLING_AND_ROOTS,
        false,
        true,
    );

    let executable = env::current_exe()?;
    let mut command = Command::new(executable);
    command
        .arg("--server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let client = handler
        .serve(TokioChildProcess::new(command)?)
        .await
        .context("could not start the local roots-aware MCP server")?;
    println!("Workspace root exposed to MCP: {}", root.display());

    let run_result = run_interactive_agent(&model, &model, &client).await;
    let disconnect_result = client.cancel().await;
    run_result?;
    disconnect_result?;
    Ok(())
}
