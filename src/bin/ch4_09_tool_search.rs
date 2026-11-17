//! Chapter 4.9: reveal matching MCP tools while retaining resource and prompt use.

use std::{
    env,
    io::{self, Write},
    process::Stdio,
};

use ai_agents_with_mcp::{
    calculator_server::serve_stdio,
    client_callbacks::CalculatorClientHandler,
    config::{
        anthropic_client, anthropic_model, load_dotenv, max_model_calls, mcp_root, mcp_root_uri,
    },
    interactive_agent::{collect_prompt, select_resources, user_message_with_resources},
    tool_results::content_as_strings,
};
use anyhow::{Context, Result, bail};
use rig::completion::{
    message::{AssistantContent, Message, ToolName, ToolResultContent},
    request::{CompletionRequest, ToolDefinition},
};
use rmcp::{ServiceExt, model::CallToolRequestParams, transport::TokioChildProcess};
use tokio::process::Command;

const SYSTEM_PROMPT: &str = "You have a search_tools tool that finds more tools. If no available tool fits a request, search for one before deciding that you cannot complete it. The search is a simple substring match, so search with one short keyword (like 'count' or 'add'), never a phrase, and try another keyword if the first finds nothing.";

#[tokio::main]
async fn main() -> Result<()> {
    if env::args().nth(1).as_deref() == Some("--server") {
        return serve_stdio().await;
    }
    load_dotenv();
    let provider = anthropic_client()?;
    let model_name = anthropic_model()?;
    let model = provider.completion(model_name.clone());
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
    let search = ToolDefinition::new(
        ToolName::new("search_tools")?,
        "Find MCP tools whose names or descriptions contain a query. Matching tools become available immediately for this turn.",
        serde_json::json!({
            "type": "object",
            "properties": { "query": { "type": "string" } },
            "required": ["query"]
        }),
    );
    println!("The agent begins each turn with search_tools only.");
    println!("Type 'refresh', 'prompts', or 'goodbye'.");

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

        // Fetch once for each user turn so server changes are visible immediately.
        let available_tools = mcp.list_all_tools().await?;
        if prompt.eq_ignore_ascii_case("refresh") {
            resources = mcp.list_all_resources().await?;
            prompts = mcp.list_all_prompts().await?;
            println!(
                "Refreshed {} resources and {} prompts.",
                resources.len(),
                prompts.len()
            );
            continue;
        }
        let user_message = if prompt.eq_ignore_ascii_case("prompts") {
            match collect_prompt(&mcp, &prompts).await? {
                Some(prompt) => Message::user(prompt),
                None => continue,
            }
        } else {
            let context = select_resources(&model, &mcp, &resources, prompt).await;
            user_message_with_resources(prompt, context)
        };

        let mut active_tools = vec![search.clone()];
        let mut history = Vec::new();
        let mut next_message = user_message;
        let mut completed = false;
        let max_calls = max_model_calls()?;
        for _ in 0..max_calls {
            let sent_message = next_message.clone();
            let response = model
                .call(
                    CompletionRequest::new(next_message)
                        .preamble(SYSTEM_PROMPT)
                        .messages(history.iter().cloned())
                        .tools(active_tools.iter().cloned())
                        .max_tokens(4096),
                )
                .await?;
            history.push(sent_message);
            let answer = response.text();
            let calls = response
                .choice
                .iter()
                .filter_map(|item| match item {
                    AssistantContent::ToolCall(call) => Some(call.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if let Some(message) = response.message() {
                history.push(message);
            }
            if calls.is_empty() {
                println!("AI Assistant: {answer}");
                completed = true;
                break;
            }

            let mut results = Vec::with_capacity(calls.len());
            for call in calls {
                if call.function.name == "search_tools" {
                    let query = call
                        .function
                        .arguments
                        .get("query")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .trim()
                        .to_lowercase();
                    let matches = if query.is_empty() {
                        Vec::new()
                    } else {
                        available_tools
                            .iter()
                            .filter(|tool| {
                                format!(
                                    "{} {}",
                                    tool.name,
                                    tool.description.as_deref().unwrap_or("")
                                )
                                .to_lowercase()
                                .contains(&query)
                            })
                            .collect::<Vec<_>>()
                    };
                    let revealed = to_model_tools(&matches)?;
                    for definition in revealed {
                        if !active_tools
                            .iter()
                            .any(|tool| tool.name.as_ref() == definition.name.as_ref())
                        {
                            active_tools.push(definition);
                        }
                    }
                    let names = matches
                        .iter()
                        .map(|tool| tool.name.as_ref())
                        .collect::<Vec<_>>();
                    println!("Search {query:?} found: {}", names.join(", "));
                    let search_result = if query.is_empty() {
                        "Search query must not be empty.".to_owned()
                    } else {
                        names.join(", ")
                    };
                    results.push(call.result(vec![ToolResultContent::text(search_result)]));
                    continue;
                }

                let result = mcp
                    .call_tool(
                        CallToolRequestParams::new(call.function.name.to_string())
                            .with_arguments(call.function.arguments.clone()),
                    )
                    .await;
                let (content, is_error) = match result {
                    Ok(result) => (
                        content_as_strings(&result.content).join("\n"),
                        result.is_error.unwrap_or(false),
                    ),
                    Err(error) => (format!("MCP tool call failed: {error}"), true),
                };
                let mut tool_result = call.result(vec![ToolResultContent::text(content)]);
                tool_result.is_error = is_error;
                results.push(tool_result);
            }
            next_message = Message::tool_results(results);
        }
        if !completed {
            bail!(
                "agent reached the {max_calls}-call model limit; increase MCP_MAX_MODEL_CALLS to allow a longer tool cycle"
            );
        }
    }
    Ok(())
    }
    .await;
    let disconnect_result = mcp.cancel().await;
    run_result?;
    disconnect_result?;
    Ok(())
}

fn to_model_tools(tools: &[&rmcp::model::Tool]) -> Result<Vec<ToolDefinition>> {
    tools
        .iter()
        .map(|tool| {
            let name = ToolName::new(tool.name.clone())?;
            let description = tool.description.as_deref().unwrap_or_default().to_owned();
            let schema = serde_json::to_value(tool.input_schema.as_ref())?;
            Ok(ToolDefinition::new(name, description, schema))
        })
        .collect()
}
