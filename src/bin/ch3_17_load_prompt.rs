//! Chapter 3.17: load a parameterized prompt from an MCP server.

use std::{env, process::Stdio};

use ai_agents_with_mcp::ch3_content_server::serve_stdio;
use anyhow::Result;
use rmcp::{ServiceExt, model::GetPromptRequestParams, transport::TokioChildProcess};
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

    let prompt = client
        .get_prompt(
            GetPromptRequestParams::new("calculate_operation")
                .with_arguments(rmcp::object!({ "operation": "2 + 2" })),
        )
        .await?;

    println!("{}", serde_json::to_string_pretty(&prompt.messages)?);
    client.cancel().await?;
    Ok(())
}
