//! Chapter 6.1: request argument completions from an MCP server.

use std::{env, process::Stdio};

use ai_agents_with_mcp::completion_server::serve_stdio;
use anyhow::Result;
use rmcp::{
    ServiceExt,
    model::{ArgumentInfo, CompleteRequestParams, Reference},
    transport::TokioChildProcess,
};
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
        .complete(CompleteRequestParams::new(
            Reference::for_prompt("simple_prompt_input"),
            ArgumentInfo::new("username", "us"),
        ))
        .await?;
    println!("Username suggestions: {:?}", result.completion.values);
    let resource_result = client
        .complete(CompleteRequestParams::new(
            Reference::for_resource("file:///{filename}"),
            ArgumentInfo::new("filename", "2"),
        ))
        .await?;
    println!(
        "Filename suggestions: {:?}",
        resource_result.completion.values
    );
    Ok(())
}
