//! Chapter 3.12: drive a handwritten Anthropic tool loop over MCP stdio.

use std::{
    env,
    io::{self, Write},
    process::Stdio,
};

use ai_agents_with_mcp::{
    agent_loop::{run_anthropic_tool_loop, to_model_tools},
    calculator_server::serve_stdio,
    config::{anthropic_client, anthropic_model, load_dotenv},
};
use anyhow::Result;
use rmcp::{ServiceExt, transport::TokioChildProcess};
use tokio::process::Command;

#[tokio::main]
async fn main() -> Result<()> {
    if env::args().nth(1).as_deref() == Some("--server") {
        serve_stdio().await
    } else {
        load_dotenv();
        run_agent().await
    }
}

async fn run_agent() -> Result<()> {
    let model_client = anthropic_client()?;
    let model = model_client.completion(anthropic_model()?);

    let executable = env::current_exe()?;
    let mut command = Command::new(executable);
    command
        .arg("--server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let transport = TokioChildProcess::new(command)?;
    let mcp_client = ().serve(transport).await?;

    let mcp_tools = mcp_client.list_all_tools().await?;
    let model_tools = to_model_tools(&mcp_tools)?;
    println!(
        "Connected MCP tools: {}",
        mcp_tools
            .iter()
            .map(|tool| tool.name.as_ref())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let agent_result: Result<()> = async {
        loop {
            print!("You: ");
            io::stdout().flush()?;

            let mut prompt = String::new();
            if io::stdin().read_line(&mut prompt)? == 0 {
                break;
            }
            let prompt = prompt.trim();
            if prompt.eq_ignore_ascii_case("goodbye") {
                println!("AI Assistant: Goodbye!");
                break;
            }
            if prompt.is_empty() {
                continue;
            }

            let answer = run_anthropic_tool_loop(&model, &mcp_client, &model_tools, prompt).await?;
            println!("AI Assistant: {answer}");
        }

        Ok(())
    }
    .await;

    mcp_client.cancel().await?;
    agent_result
}
