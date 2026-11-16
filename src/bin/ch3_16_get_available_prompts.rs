//! Chapter 3.16: list the prompts exposed by an MCP server.

use std::{env, process::Stdio};

use ai_agents_with_mcp::ch3_content_server::serve_stdio;
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

    for prompt in client.list_all_prompts().await? {
        println!(
            "{}: {}",
            prompt.name,
            prompt.description.as_deref().unwrap_or("(no description)")
        );
        for argument in prompt.arguments.unwrap_or_default() {
            println!(
                "  - {} (required: {})",
                argument.name,
                argument.required.unwrap_or(false)
            );
        }
    }

    client.cancel().await?;
    Ok(())
}
