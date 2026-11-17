//! Chapter 4.8: refresh tools per turn and use server resources and prompts.

use std::{
    env,
    io::{self, Write},
    process::Stdio,
};

use ai_agents_with_mcp::{
    agent_loop::{run_anthropic_tool_loop_with_message, to_model_tools},
    calculator_server::serve_stdio,
    client_callbacks::CalculatorClientHandler,
    config::{anthropic_client, anthropic_model, load_dotenv, mcp_root, mcp_root_uri},
    interactive_agent::{collect_prompt, select_resources, user_message_with_resources},
};
use anyhow::{Context, Result};
use rmcp::{ServiceExt, transport::TokioChildProcess};
use tokio::process::Command;

#[tokio::main]
async fn main() -> Result<()> {
    if env::args().nth(1).as_deref() == Some("--server") {
        return serve_stdio().await;
    }

    load_dotenv();
    let provider = anthropic_client()?;
    let model_name = anthropic_model()?;
    let model = provider.completion(model_name.clone());
    let selector_name = configured_model("ANTHROPIC_MODEL_RESOURCE_SELECTOR", &model_name);
    let selector = provider.completion(selector_name);

    let root = mcp_root()?;
    let handler = CalculatorClientHandler::new(mcp_root_uri(&root)?, model_name);

    let executable = env::current_exe()?;
    let mut command = Command::new(executable);
    command
        .arg("--server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let mcp = handler
        .serve(TokioChildProcess::new(command)?)
        .await
        .context("could not start the local calculator MCP server")?;

    let run_result: Result<()> = async {
        let mut resources = mcp.list_all_resources().await?;
        let mut prompts = mcp.list_all_prompts().await?;
        println!("Tool definitions are refreshed at the start of each turn.");
        println!("Workspace root exposed to MCP: {}", root.display());
        println!("Type 'refresh', 'prompts', or 'goodbye'.");

        loop {
            print!("You: ");
            io::stdout().flush()?;
            let mut input = String::new();
            if io::stdin().read_line(&mut input)? == 0 {
                break;
            }
            let input = input.trim();
            if input.eq_ignore_ascii_case("goodbye") {
                println!("AI Assistant: Goodbye!");
                break;
            }
            if input.is_empty() {
                continue;
            }

            // Rebuild the model's advertised tool set at the start of every turn.
            let available_tools = mcp.list_all_tools().await?;
            println!(
                "Tools assembled for this turn: {}",
                available_tools
                    .iter()
                    .map(|tool| tool.name.as_ref())
                    .collect::<Vec<_>>()
                    .join(", ")
            );

            if input.eq_ignore_ascii_case("refresh") {
                resources = mcp.list_all_resources().await?;
                prompts = mcp.list_all_prompts().await?;
                println!(
                    "Refreshed {} resources and {} prompts.",
                    resources.len(),
                    prompts.len()
                );
                continue;
            }

            let user_message = if input.eq_ignore_ascii_case("prompts") {
                match collect_prompt(&mcp, &prompts).await? {
                    Some(prompt) => rig::completion::message::Message::user(prompt),
                    None => continue,
                }
            } else {
                let context = select_resources(&selector, &mcp, &resources, input).await;
                user_message_with_resources(input, context)
            };

            let tools = to_model_tools(&available_tools)?;
            let answer =
                run_anthropic_tool_loop_with_message(&model, &mcp, &tools, user_message).await?;
            println!("AI Assistant: {answer}");
        }
        Ok(())
    }
    .await;
    let disconnect_result = mcp.cancel().await;
    run_result?;
    disconnect_result?;
    Ok(())
}

fn configured_model(variable: &str, fallback: &str) -> String {
    env::var(variable)
        .ok()
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}
