//! Chapter 3.4: explicitly connect to and disconnect from a stdio server.

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

    println!("Connected to calculator server.");
    let tools = client.list_all_tools().await?;
    println!("Server offers {} tool(s).", tools.len());
    client.cancel().await?;
    println!("Disconnected from calculator server.");
    Ok(())
}
