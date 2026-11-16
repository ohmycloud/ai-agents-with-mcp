//! Chapter 3.3: configure and create a stdio MCP client.

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
    println!("Constructed stdio MCP client: {:#?}", client.peer_info());
    client.cancel().await?;
    Ok(())
}
