//! Chapter 3.11: receive progress notifications while a tool is running.

use std::{env, future::Future, process::Stdio};

use ai_agents_with_mcp::tutorial_servers::serve_progress_stdio;
use anyhow::Result;
use rmcp::{
    RoleClient, ServiceExt,
    handler::client::ClientHandler,
    model::{CallToolRequestParams, ProgressNotificationParam},
    service::{NotificationContext, RunningService},
    transport::TokioChildProcess,
};
use tokio::process::Command;

#[derive(Clone)]
struct ProgressHandler;

impl ClientHandler for ProgressHandler {
    fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) -> impl Future<Output = ()> + Send + '_ {
        async move {
            let percent = params
                .total
                .filter(|total| *total > 0.0)
                .map(|total| format!(" ({:.0}%)", params.progress / total * 100.0))
                .unwrap_or_default();
            println!("Progress: {}{percent}", params.message.unwrap_or_default());
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    if env::args().nth(1).as_deref() == Some("--server") {
        return serve_progress_stdio().await;
    }

    let executable = env::current_exe()?;
    let mut command = Command::new(executable);
    command
        .arg("--server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let client: RunningService<RoleClient, ProgressHandler> = ProgressHandler
        .serve(TokioChildProcess::new(command)?)
        .await?;

    let result = client
        .call_tool(CallToolRequestParams::new("slow_operation"))
        .await?;
    println!("Tool result: {:#?}", result.content);
    client.cancel().await?;
    Ok(())
}
