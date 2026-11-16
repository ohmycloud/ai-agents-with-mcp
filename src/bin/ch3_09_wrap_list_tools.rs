//! Chapter 3.9: list the tools exposed by an MCP stdio server.

use std::{env, process::Stdio};

use ai_agents_with_mcp::calculator_server::serve_stdio;
use anyhow::Result;
use rmcp::{ServiceExt, transport::TokioChildProcess};
use tokio::process::Command;

#[tokio::main]
async fn main() -> Result<()> {
    if env::args().nth(1).as_deref() == Some("--server") {
        return serve_stdio().await;
    }

    let executable = env::current_exe()?;
    let mut command = Command::new(executable);
    command
        .arg("--server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let client = ().serve(TokioChildProcess::new(command)?).await?;

    let tools = client.list_all_tools().await?;
    if tools.is_empty() {
        println!("No tools found.");
    }
    for tool in tools {
        println!(
            "{}: {}",
            tool.name,
            tool.description.as_deref().unwrap_or("")
        );
    }

    client.cancel().await?;
    Ok(())
}
