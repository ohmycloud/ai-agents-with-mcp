//! Chapter 3.13: list MCP resources from a stdio server.

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

    let resources = client.list_all_resources().await?;
    if resources.is_empty() {
        println!("No resources found.");
    }
    for resource in resources {
        println!("{} — {}", resource.name, resource.uri);
        if let Some(description) = resource.description {
            println!("  {description}");
        }
    }

    client.cancel().await?;
    Ok(())
}
