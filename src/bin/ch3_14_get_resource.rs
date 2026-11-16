//! Chapter 3.14: read a resource after discovering it over MCP stdio.

use std::{env, process::Stdio};

use ai_agents_with_mcp::ch3_content_server::{MATH_CONSTANTS_URI, serve_stdio};
use anyhow::Result;
use rmcp::{ServiceExt, model::ReadResourceRequestParams, transport::TokioChildProcess};
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

    for resource in client.list_all_resources().await? {
        println!("Found resource: {} ({})", resource.name, resource.uri);
    }

    let result = client
        .read_resource(ReadResourceRequestParams::new(MATH_CONSTANTS_URI))
        .await?;
    for content in result.contents {
        if let rmcp::model::ResourceContents::TextResourceContents { text, .. } = content {
            println!("Resource contents: {text}");
        }
    }

    client.cancel().await?;
    Ok(())
}
