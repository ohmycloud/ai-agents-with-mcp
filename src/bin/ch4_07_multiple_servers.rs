//! Chapter 4.7: select calculator resources and prompts for each turn.
//!
//! The Python lesson is named "multiple servers", but its agent connects only
//! to the calculator server. This binary mirrors that actual execution path.

use std::{
    env,
    io::{self, Write},
    process::Stdio,
};

use ai_agents_with_mcp::{
    calculator_server::serve_stdio as serve_calculator,
    client_callbacks::CalculatorClientHandler,
    config::{
        anthropic_client, anthropic_model, load_dotenv, max_model_calls, mcp_root, mcp_root_uri,
    },
    interactive_agent::resource_content_as_user_content,
    tool_results::content_as_strings,
};
use anyhow::{Result, bail};
use rig::Model;
use rig::completion::{
    message::{AssistantContent, Message, ToolName, ToolResultContent},
    request::{CompletionRequest, ToolDefinition},
};
use rmcp::{
    RoleClient, ServiceExt,
    model::{
        CallToolRequestParams, GetPromptRequestParams, Prompt, ReadResourceRequestParams, Resource,
    },
    service::RunningService,
    transport::TokioChildProcess,
};
use serde_json::{Map, Value, json};
use tokio::process::Command;

type McpClient = RunningService<RoleClient, CalculatorClientHandler>;

#[derive(Debug)]
struct SelectedPrompt {
    name: String,
    arguments: Map<String, Value>,
}

#[tokio::main]
async fn main() -> Result<()> {
    if env::args().nth(1).as_deref() == Some("--calculator-server") {
        return serve_calculator().await;
    }

    load_dotenv();
    let default_model = anthropic_model()?;
    let model_client = anthropic_client()?;
    let model = model_client.completion(default_model.clone());
    let root = mcp_root()?;
    let handler = CalculatorClientHandler::new(mcp_root_uri(&root)?, default_model);
    let mut server_command = Command::new(env::current_exe()?);
    server_command
        .arg("--calculator-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let client = handler
        .serve(TokioChildProcess::new(server_command)?)
        .await?;

    let run_result = run_agent(&model, &client).await;
    let disconnect_result = client.cancel().await;
    run_result?;
    disconnect_result?;
    Ok(())
}

async fn run_agent(
    model: &Model<rig::providers::anthropic::wire::Messages>,
    client: &McpClient,
) -> Result<()> {
    println!(
        "Welcome to your AI Assistant. Type 'goodbye' to quit or 'refresh' to reload and redisplay available resources."
    );
    let tools = collect_tools(client).await?;
    let mut resources = client.list_all_resources().await?;
    let mut prompts = client.list_all_prompts().await?;
    println!(
        "Loaded {} resources and {} prompts.",
        resources.len(),
        prompts.len()
    );

    loop {
        print!("You: ");
        io::stdout().flush()?;
        let mut question = String::new();
        if io::stdin().read_line(&mut question)? == 0 {
            break;
        }
        let question = question.trim();
        if question.eq_ignore_ascii_case("goodbye") {
            println!("AI Assistant: Goodbye!");
            break;
        }
        if question.is_empty() {
            continue;
        }
        if question.eq_ignore_ascii_case("refresh") {
            resources = client.list_all_resources().await?;
            prompts = client.list_all_prompts().await?;
            println!(
                "Refreshed {} resources and {} prompts.",
                resources.len(),
                prompts.len()
            );
            continue;
        }

        let selected_resources = select_resources(model, &resources, question).await;
        let selected_prompts = select_prompts(model, &prompts, question).await;
        let resource_context = load_resources(client, &resources, &selected_resources).await;
        let system_instructions = load_prompts(client, &prompts, selected_prompts).await;
        let mut user_content = vec![rig::completion::message::UserContent::text(question)];
        user_content.extend(resource_context);
        let user_message = Message::from(user_content);

        run_tool_loop(model, client, &tools, user_message, system_instructions).await?;
    }

    Ok(())
}

async fn collect_tools(client: &McpClient) -> Result<Vec<ToolDefinition>> {
    client
        .list_all_tools()
        .await?
        .into_iter()
        .map(|tool| {
            Ok(ToolDefinition::new(
                ToolName::new(tool.name.to_string())?,
                tool.description.as_deref().unwrap_or_default(),
                serde_json::to_value(tool.input_schema.as_ref())?,
            ))
        })
        .collect()
}

async fn run_tool_loop(
    model: &Model<rig::providers::anthropic::wire::Messages>,
    client: &McpClient,
    tools: &[ToolDefinition],
    initial_message: Message,
    system_instructions: String,
) -> Result<()> {
    let mut history = Vec::new();
    let mut next_message = initial_message;
    let max_calls = max_model_calls()?;

    for _ in 0..max_calls {
        let sent_message = next_message.clone();
        let mut request = CompletionRequest::new(next_message)
            .messages(history.iter().cloned())
            .tools(tools.iter().cloned())
            .max_tokens(4096);
        if !system_instructions.is_empty() {
            request = request.preamble(system_instructions.clone());
        }
        let response = model.call(request).await?;
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
            let text_blocks = answer.trim();
            println!(
                "Assistant: {}",
                if text_blocks.is_empty() {
                    "[No text response available]"
                } else {
                    text_blocks
                }
            );
            return Ok(());
        }

        let mut results = Vec::with_capacity(calls.len());
        for call in calls {
            let name = call.function.name.as_str();
            println!("Using tool: {name}");
            let params = CallToolRequestParams::new(name.to_owned())
                .with_arguments(call.function.arguments.clone());
            let (content, is_error) = match client.call_tool(params).await {
                Ok(result) => (
                    content_as_strings(&result.content).join("\n"),
                    result.is_error.unwrap_or(false),
                ),
                Err(error) => (format!("MCP tool call failed: {error}"), true),
            };
            let mut result = call.result(vec![ToolResultContent::text(content)]);
            result.is_error = is_error;
            results.push(result);
        }
        next_message = Message::tool_results(results);
    }

    bail!(
        "agent reached the {max_calls}-call model limit; increase MCP_MAX_MODEL_CALLS to allow a longer tool cycle"
    )
}

async fn select_resources(
    model: &Model<rig::providers::anthropic::wire::Messages>,
    resources: &[Resource],
    question: &str,
) -> Vec<String> {
    if resources.is_empty() {
        return Vec::new();
    }
    let descriptions = resources
        .iter()
        .map(|resource| {
            (
                resource.name.clone(),
                Value::String(
                    resource
                        .description
                        .clone()
                        .unwrap_or_else(|| format!("Resource: {}", resource.name)),
                ),
            )
        })
        .collect::<Map<_, _>>();
    let prompt = format!(
        "Given this user question: {question:?}\n\nAnd these available resources:\n{}\n\nWhich resources (if any) would be helpful to answer the user's question? Return a JSON array of resource names, or an empty array. Only include directly relevant resources.",
        serde_json::to_string_pretty(&descriptions).unwrap_or_default()
    );
    match model
        .call(CompletionRequest::new(prompt).max_tokens(200))
        .await
    {
        Ok(response) => parse_string_array(&response.text())
            .into_iter()
            .filter(|name| {
                resources
                    .iter()
                    .any(|resource| resource.name.as_str() == name.as_str())
            })
            .collect(),
        Err(error) => {
            eprintln!("Failed to select resources with model: {error}");
            Vec::new()
        }
    }
}

async fn select_prompts(
    model: &Model<rig::providers::anthropic::wire::Messages>,
    prompts: &[Prompt],
    question: &str,
) -> Vec<SelectedPrompt> {
    if prompts.is_empty() {
        return Vec::new();
    }
    let templates = prompts
        .iter()
        .map(|prompt| {
            json!({
                "name": prompt.name,
                "description": prompt.description,
                "arguments": prompt.arguments,
            })
        })
        .collect::<Vec<_>>();
    let prompt = format!(
        "Given this user question: {question:?}\n\nAnd these available prompt templates:\n{}\n\nWhich prompts (if any) would provide helpful instructions or guidance? Return a JSON array of prompt objects with a name and arguments object, or an empty array. Only include directly relevant prompts.",
        serde_json::to_string_pretty(&templates).unwrap_or_default()
    );
    let response = match model
        .call(CompletionRequest::new(prompt).max_tokens(200))
        .await
    {
        Ok(response) => response,
        Err(error) => {
            eprintln!("Failed to select prompts with model: {error}");
            return Vec::new();
        }
    };
    let response_text = response.text();
    let Some(json) = json_array_slice(&response_text) else {
        return Vec::new();
    };
    let Ok(Value::Array(selected)) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    selected
        .into_iter()
        .filter_map(|entry| {
            let object = entry.as_object()?;
            let name = object.get("name")?.as_str()?.to_owned();
            if !prompts.iter().any(|prompt| prompt.name == name) {
                return None;
            }
            let arguments = object
                .get("arguments")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            Some(SelectedPrompt { name, arguments })
        })
        .collect()
}

async fn load_resources(
    client: &McpClient,
    resources: &[Resource],
    selected: &[String],
) -> Vec<rig::completion::message::UserContent> {
    let mut context = Vec::new();
    for resource in resources
        .iter()
        .filter(|resource| selected.iter().any(|name| name == &resource.name))
    {
        println!("LLM selected resource: {}", resource.name);
        match client
            .read_resource(ReadResourceRequestParams::new(resource.uri.clone()))
            .await
        {
            Ok(response) => {
                for contents in response.contents {
                    context.extend(resource_content_as_user_content(&resource.name, contents));
                }
            }
            Err(error) => eprintln!("Error loading resource {}: {error}", resource.name),
        }
    }
    context
}

async fn load_prompts(
    client: &McpClient,
    prompts: &[Prompt],
    selected: Vec<SelectedPrompt>,
) -> String {
    let mut instructions = Vec::new();
    for selected_prompt in selected {
        let Some(prompt) = prompts
            .iter()
            .find(|prompt| prompt.name == selected_prompt.name)
        else {
            continue;
        };
        println!("Using prompt: {}", prompt.name);
        let arguments = selected_prompt
            .arguments
            .into_iter()
            .map(|(key, value)| (key, Value::String(value_to_text(&value))))
            .collect();
        match client
            .get_prompt(GetPromptRequestParams::new(prompt.name.clone()).with_arguments(arguments))
            .await
        {
            Ok(response) => {
                let text = response
                    .messages
                    .into_iter()
                    .filter_map(|message| message.content.as_text().map(|block| block.text.clone()))
                    .collect::<Vec<_>>()
                    .join("\n");
                if !text.trim().is_empty() {
                    instructions.push(format!("[Prompt: {}]\n{}", prompt.name, text.trim()));
                }
            }
            Err(error) => eprintln!("Error loading prompt {}: {error}", prompt.name),
        }
    }
    instructions.join("\n\n")
}

fn value_to_text(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_owned)
}

fn parse_string_array(answer: &str) -> Vec<String> {
    json_array_slice(answer)
        .and_then(|json| serde_json::from_str::<Vec<String>>(json).ok())
        .unwrap_or_default()
}

fn json_array_slice(answer: &str) -> Option<&str> {
    let start = answer.find('[')?;
    let end = answer.rfind(']')?;
    (end >= start).then_some(&answer[start..=end])
}
