//! Chapter 4.1: an interactive agent whose client answers MCP sampling requests.

use std::{env, path::PathBuf, process::Stdio};

use ai_agents_with_mcp::{
    calculator_server::serve_stdio,
    client_callbacks::{CalculatorClientHandler, CallbackFeatures},
    config::{anthropic_client, anthropic_model, load_dotenv},
    interactive_agent::run_interactive_agent,
};
use anyhow::{Context, Result, ensure};
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
    let root = env::var_os("MCP_ROOT")
        .map(PathBuf::from)
        .unwrap_or(env::current_dir()?);
    let root = root
        .canonicalize()
        .context("MCP_ROOT must point to an existing directory")?;
    ensure!(root.is_dir(), "MCP_ROOT must point to a directory");
    let handler = CalculatorClientHandler::for_chapter(
        format!("file://{}", root.display()),
        model_name,
        CallbackFeatures::SAMPLING,
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
        .context("could not start the local calculator MCP server")?;

    run_interactive_agent(&model, &model, &client).await?;
    client.cancel().await?;
    Ok(())
}
