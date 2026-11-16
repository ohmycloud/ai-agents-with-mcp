//! Chapter 3.10: call a named tool through an MCP stdio client.

use std::{env, process::Stdio};

use ai_agents_with_mcp::calculator_server::serve_stdio;
use ai_agents_with_mcp::tool_results::content_as_strings;
use anyhow::Result;
use rmcp::{ServiceExt, model::CallToolRequestParams, transport::TokioChildProcess};
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

    let result = client
        .call_tool(
            CallToolRequestParams::new("add").with_arguments(rmcp::object!({ "a": 20, "b": 22 })),
        )
        .await?;
    let content = content_as_strings(&result.content);
    println!("add(20, 22): {}", content.join("\n"));
    client.cancel().await?;
    Ok(())
}
