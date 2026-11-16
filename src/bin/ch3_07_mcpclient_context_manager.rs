//! Chapter 3.7: wrap MCP connection lifetime in a small client type.

use std::{env, process::Stdio};

use ai_agents_with_mcp::calculator_server::serve_stdio;
use anyhow::Result;
use rmcp::{RoleClient, ServiceExt, service::RunningService, transport::TokioChildProcess};
use tokio::process::Command;

struct McpClient {
    connection: RunningService<RoleClient, ()>,
}

impl McpClient {
    async fn connect() -> Result<Self> {
        let executable = env::current_exe()?;
        let mut command = Command::new(executable);
        command
            .arg("--server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        let connection = ().serve(TokioChildProcess::new(command)?).await?;
        Ok(Self { connection })
    }

    async fn list_tools(&self) -> Result<()> {
        for tool in self.connection.list_all_tools().await? {
            println!(
                "{}: {}",
                tool.name,
                tool.description.as_deref().unwrap_or("")
            );
        }
        Ok(())
    }

    async fn disconnect(self) -> Result<()> {
        self.connection.cancel().await?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    if env::args().nth(1).as_deref() == Some("--server") {
        return serve_stdio().await;
    }

    let client = McpClient::connect().await?;
    client.list_tools().await?;
    client.disconnect().await
}
