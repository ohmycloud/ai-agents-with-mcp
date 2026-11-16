//! Chapter 6.12: cancel a running tool request and stop server-side work.

use std::{env, process::Stdio, time::Duration};

use ai_agents_with_mcp::ch6_servers::serve_cancellation_stdio;
use anyhow::Result;
use rmcp::{ServiceExt, model::CallToolRequestParams, transport::TokioChildProcess};
use tokio::{process::Command, time::timeout};

#[tokio::main]
async fn main() -> Result<()> {
    if env::args().nth(1).as_deref() == Some("--server") {
        return serve_cancellation_stdio().await;
    }

    let executable = env::current_exe()?;
    let mut command = Command::new(executable);
    command
        .arg("--server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let client = ().serve(TokioChildProcess::new(command)?).await?;
    let call = client.call_tool(CallToolRequestParams::new("slow_operation"));
    match timeout(Duration::from_millis(350), call).await {
        Ok(result) => println!("Tool result: {:#?}", result?.content),
        Err(_) => println!("Client stopped waiting; MCP cancellation was sent."),
    }
    client.cancel().await?;
    Ok(())
}
