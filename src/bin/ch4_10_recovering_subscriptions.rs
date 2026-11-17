//! Chapter 4.10: recover the current tool set after a server list-change event.

use std::{
    env,
    io::{self, Write},
};

use ai_agents_with_mcp::{
    agent_loop::{run_anthropic_tool_loop_with_message, to_model_tools},
    client_callbacks::CalculatorClientHandler,
    config::{anthropic_client, anthropic_model, load_dotenv, mcp_root, mcp_root_uri},
    interactive_agent::{collect_prompt, select_resources, user_message_with_resources},
};
use anyhow::{Context, Result};
use rmcp::ServiceExt;

#[tokio::main]
async fn main() -> Result<()> {
    load_dotenv();
    let provider = anthropic_client()?;
    let model_name = anthropic_model()?;
    let model = provider.completion(model_name.clone());
    let root = mcp_root()?;

    let endpoint = env::var("MCP_SERVER_URL")
        .ok()
        .filter(|endpoint| !endpoint.trim().is_empty())
        .unwrap_or_else(|| "http://localhost:8000/mcp".to_owned());
    let handler = CalculatorClientHandler::new(mcp_root_uri(&root)?, model_name);
    let mcp = handler
        .serve(rmcp::transport::StreamableHttpClientTransport::from_uri(
            endpoint.clone(),
        ))
        .await
        .context("could not connect to the calculator MCP server")?;

    let run_result: Result<()> = async {
        let mut resources = mcp.list_all_resources().await?;
        let mut prompts = mcp.list_all_prompts().await?;
        println!("Connected to {endpoint}");
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

            // rmcp invalidates this cached result when tools/list_changed arrives.
            let available = mcp.list_all_tools().await?;
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
                let context = select_resources(&model, &mcp, &resources, input).await;
                user_message_with_resources(input, context)
            };

            let tools = to_model_tools(&available)?;
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
