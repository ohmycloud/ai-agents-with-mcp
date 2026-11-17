//! Chapter 4.5: use separate draft and review models with a full MCP client.

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
    let default_model = anthropic_model()?;
    let draft_model_name = configured_model("ANTHROPIC_MODEL_FAST", &default_model);
    let review_model_name = configured_model("ANTHROPIC_MODEL_REVIEW", &default_model);
    let selector_model_name = configured_model("ANTHROPIC_MODEL_RESOURCE_SELECTOR", &default_model);
    let draft_model = provider.completion(draft_model_name.clone());
    let review_model = provider.completion(review_model_name.clone());
    let selector_model = provider.completion(selector_model_name);

    let root = mcp_root()?;
    let handler = CalculatorClientHandler::new(mcp_root_uri(&root)?, default_model);
    let executable = env::current_exe()?;
    let mut command = Command::new(executable);
    command
        .arg("--server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let mcp_client = handler
        .serve(TokioChildProcess::new(command)?)
        .await
        .context("could not start the local calculator MCP server")?;

    let run_result: Result<()> = async {
    let mut tools = mcp_client.list_all_tools().await?;
    let mut model_tools = to_model_tools(&tools)?;
    let mut resources = mcp_client.list_all_resources().await?;
    let mut prompts = mcp_client.list_all_prompts().await?;

    println!("Draft model: {draft_model_name}; review model: {review_model_name}");
    println!("Calculator MCP tools: {}", tool_names(&tools));
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
            break;
        }
        if input.is_empty() {
            continue;
        }
        if input.eq_ignore_ascii_case("refresh") {
            resources = mcp_client.list_all_resources().await?;
            prompts = mcp_client.list_all_prompts().await?;
            println!(
                "Refreshed {} resources and {} prompts.",
                resources.len(),
                prompts.len()
            );
            continue;
        }

        let user_message = if input.eq_ignore_ascii_case("prompts") {
            match collect_prompt(&mcp_client, &prompts).await? {
                Some(prompt) => rig::completion::message::Message::user(prompt),
                None => continue,
            }
        } else {
            let context = select_resources(&selector_model, &mcp_client, &resources, input).await;
            user_message_with_resources(input, context)
        };

        let draft =
            run_anthropic_tool_loop_with_message(
                &draft_model,
                &mcp_client,
                &model_tools,
                user_message,
            )
            .await?;
        let reviewed = review_model
            .call(format!(
                "Improve this draft for accuracy and clarity. Return only the final answer.\n\nRequest: {input}\n\nDraft:\n{draft}"
            ))
            .await?;
        println!("AI Assistant: {}", reviewed.text());

        let refreshed_tools = mcp_client.list_all_tools().await?;
        if tool_names(&refreshed_tools) != tool_names(&tools) {
            println!(
                "Calculator MCP tools updated: {}",
                tool_names(&refreshed_tools)
            );
            model_tools = to_model_tools(&refreshed_tools)?;
            tools = refreshed_tools;
        }
    }
    Ok(())
    }
    .await;
    let disconnect_result = mcp_client.cancel().await;
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

fn tool_names(tools: &[rmcp::model::Tool]) -> String {
    tools
        .iter()
        .map(|tool| tool.name.as_ref())
        .collect::<Vec<_>>()
        .join(", ")
}
