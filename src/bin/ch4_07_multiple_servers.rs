//! Chapter 4.7: combine multiple MCP tool, resource, and prompt servers.

use std::{
    env,
    io::{self, Write},
    path::PathBuf,
    process::Stdio,
};

use ai_agents_with_mcp::{
    calculator_server::serve_stdio as serve_calculator,
    client_callbacks::CalculatorClientHandler,
    config::{anthropic_client, anthropic_model, load_dotenv},
    content_server::serve_stdio as serve_content,
    tool_results::content_as_strings,
};
use anyhow::{Context, Result, bail, ensure};
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

const MAX_MODEL_CALLS: usize = 16;
type McpClient = RunningService<RoleClient, CalculatorClientHandler>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ServerKind {
    Calculator,
    Content,
}

impl ServerKind {
    fn label(self) -> &'static str {
        match self {
            Self::Calculator => "calculator",
            Self::Content => "content",
        }
    }
}

#[derive(Clone)]
struct RoutedTool {
    model_name: String,
    server: ServerKind,
    server_name: String,
}

#[derive(Clone)]
struct RoutedResource {
    id: String,
    server: ServerKind,
    resource: Resource,
}

#[derive(Clone)]
struct RoutedPrompt {
    id: String,
    server: ServerKind,
    prompt: Prompt,
}

#[tokio::main]
async fn main() -> Result<()> {
    match env::args().nth(1).as_deref() {
        Some("--calculator-server") => return serve_calculator().await,
        Some("--content-server") => return serve_content().await,
        _ => {}
    }

    load_dotenv();
    let model_client = anthropic_client()?;
    let default_model = anthropic_model()?;
    let model = model_client.completion(default_model.clone());
    let root = env::var_os("MCP_ROOT")
        .map(PathBuf::from)
        .unwrap_or(env::current_dir()?);
    let root = root
        .canonicalize()
        .context("MCP_ROOT must point to an existing directory")?;
    ensure!(root.is_dir(), "MCP_ROOT must point to a directory");
    let handler = CalculatorClientHandler::new(format!("file://{}", root.display()), default_model);
    let executable = env::current_exe()?;

    let mut calculator_command = Command::new(&executable);
    calculator_command
        .arg("--calculator-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let calculator = handler
        .clone()
        .serve(TokioChildProcess::new(calculator_command)?)
        .await?;

    let mut content_command = Command::new(executable);
    content_command
        .arg("--content-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let content = handler
        .serve(TokioChildProcess::new(content_command)?)
        .await?;

    let (mut tools, mut routed_tools) = collect_tools(&calculator, &content).await?;
    let mut resources = collect_resources(&calculator, &content).await?;
    let mut prompts = collect_prompts(&calculator, &content).await?;
    println!(
        "Connected servers: calculator (tools/resources/prompts), content (resources/prompts)"
    );
    println!(
        "Loaded {} tools, {} resources, and {} prompts.",
        tools.len(),
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
            (tools, routed_tools) = collect_tools(&calculator, &content).await?;
            resources = collect_resources(&calculator, &content).await?;
            prompts = collect_prompts(&calculator, &content).await?;
            println!(
                "Refreshed {} tools, {} resources, and {} prompts.",
                tools.len(),
                resources.len(),
                prompts.len()
            );
            continue;
        }

        let selected_resources = select_resources(&model, &resources, question).await;
        let selected_prompts = select_prompts(&model, &prompts, question).await;
        let resource_context =
            load_resources(&calculator, &content, &resources, &selected_resources).await;
        let system_instructions =
            load_prompts(&calculator, &content, &prompts, selected_prompts).await;
        let user_prompt = if resource_context.is_empty() {
            question.to_owned()
        } else {
            format!("Question: {question}\n\nRelevant MCP resources:\n{resource_context}")
        };

        let mut history = Vec::new();
        let mut next_message = Message::user(user_prompt);
        let mut completed = false;
        for _ in 0..MAX_MODEL_CALLS {
            let mut request = CompletionRequest::new(next_message)
                .messages(history.iter().cloned())
                .tools(tools.iter().cloned())
                .max_tokens(4096);
            if !system_instructions.is_empty() {
                request = request.preamble(system_instructions.clone());
            }
            let response = model.call(request).await?;
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
                println!(
                    "AI Assistant: {}",
                    if answer.trim().is_empty() {
                        "[No text response available]"
                    } else {
                        &answer
                    }
                );
                completed = true;
                break;
            }

            let mut results = Vec::with_capacity(calls.len());
            for call in calls {
                let qualified_name = call.function.name.as_str();
                let Some(route) = routed_tools
                    .iter()
                    .find(|route| route.model_name == qualified_name)
                else {
                    let mut result = call.result(vec![ToolResultContent::text(format!(
                        "No MCP tool is registered as {qualified_name:?}."
                    ))]);
                    result.is_error = true;
                    results.push(result);
                    continue;
                };
                println!("Using {} tool: {}", route.server.label(), route.server_name);
                let request = CallToolRequestParams::new(route.server_name.clone())
                    .with_arguments(call.function.arguments.clone());
                let response = match route.server {
                    ServerKind::Calculator => calculator.call_tool(request).await,
                    ServerKind::Content => content.call_tool(request).await,
                };
                let (text, is_error) = match response {
                    Ok(result) => (
                        content_as_strings(&result.content).join("\n"),
                        result.is_error.unwrap_or(false),
                    ),
                    Err(error) => (format!("MCP tool call failed: {error}"), true),
                };
                let mut result = call.result(vec![ToolResultContent::text(text)]);
                result.is_error = is_error;
                results.push(result);
            }
            next_message = Message::tool_results(results);
        }
        if !completed {
            bail!("agent reached the {MAX_MODEL_CALLS}-call model limit");
        }
    }

    calculator.cancel().await?;
    content.cancel().await?;
    Ok(())
}

async fn collect_tools(
    calculator: &McpClient,
    content: &McpClient,
) -> Result<(Vec<ToolDefinition>, Vec<RoutedTool>)> {
    let mut definitions = Vec::new();
    let mut routes = Vec::new();
    for (server, tools) in [
        (ServerKind::Calculator, calculator.list_all_tools().await?),
        (ServerKind::Content, content.list_all_tools().await?),
    ] {
        for tool in tools {
            let model_name = format!("{}__{}", server.label(), tool.name);
            let definition = ToolDefinition::new(
                ToolName::new(model_name.clone())?,
                tool.description.as_deref().unwrap_or_default(),
                serde_json::to_value(tool.input_schema.as_ref())?,
            );
            routes.push(RoutedTool {
                model_name,
                server,
                server_name: tool.name.to_string(),
            });
            definitions.push(definition);
        }
    }
    Ok((definitions, routes))
}

async fn collect_resources(
    calculator: &McpClient,
    content: &McpClient,
) -> Result<Vec<RoutedResource>> {
    let mut resources = Vec::new();
    for (server, items) in [
        (
            ServerKind::Calculator,
            calculator.list_all_resources().await?,
        ),
        (ServerKind::Content, content.list_all_resources().await?),
    ] {
        for resource in items {
            resources.push(RoutedResource {
                id: format!("{}::{}", server.label(), resource.name),
                server,
                resource,
            });
        }
    }
    Ok(resources)
}

async fn collect_prompts(calculator: &McpClient, content: &McpClient) -> Result<Vec<RoutedPrompt>> {
    let mut prompts = Vec::new();
    for (server, items) in [
        (ServerKind::Calculator, calculator.list_all_prompts().await?),
        (ServerKind::Content, content.list_all_prompts().await?),
    ] {
        for prompt in items {
            prompts.push(RoutedPrompt {
                id: format!("{}::{}", server.label(), prompt.name),
                server,
                prompt,
            });
        }
    }
    Ok(prompts)
}

async fn select_resources(
    model: &Model<rig::providers::anthropic::wire::Messages>,
    resources: &[RoutedResource],
    question: &str,
) -> Vec<String> {
    if resources.is_empty() {
        return Vec::new();
    }
    let descriptions = resources
        .iter()
        .map(|item| {
            (
                item.id.clone(),
                json!({
                    "name": &item.resource.name,
                    "description": item.resource.description.as_deref().unwrap_or("(no description)"),
                }),
            )
        })
        .collect::<Map<_, _>>();
    let prompt = format!(
        "Given this user question: {question:?}\n\nAvailable MCP resources:\n{}\n\nReturn only a JSON array of resource ids that are directly relevant, or an empty array if none are relevant.",
        serde_json::to_string_pretty(&descriptions).unwrap_or_default()
    );
    match model
        .call(CompletionRequest::new(prompt).max_tokens(200))
        .await
    {
        Ok(response) => parse_string_array(&response.text())
            .into_iter()
            .filter(|id| resources.iter().any(|resource| &resource.id == id))
            .collect(),
        Err(error) => {
            eprintln!("Failed to select resources with model: {error}");
            Vec::new()
        }
    }
}

async fn select_prompts(
    model: &Model<rig::providers::anthropic::wire::Messages>,
    prompts: &[RoutedPrompt],
    question: &str,
) -> Vec<(String, Map<String, Value>)> {
    if prompts.is_empty() {
        return Vec::new();
    }
    let templates = prompts
        .iter()
        .map(|item| {
            json!({
                "id": &item.id,
                "name": &item.prompt.name,
                "description": item.prompt.description.as_deref().unwrap_or("(no description)"),
                "arguments": item.prompt.arguments.as_deref().unwrap_or_default(),
            })
        })
        .collect::<Vec<_>>();
    let selection_prompt = format!(
        "Given this user question: {question:?}\n\nAvailable MCP prompt templates:\n{}\n\nReturn only a JSON array of prompt objects with an id and arguments object. Return an empty array if none are directly relevant.",
        serde_json::to_string_pretty(&templates).unwrap_or_default()
    );
    let response = match model
        .call(CompletionRequest::new(selection_prompt).max_tokens(300))
        .await
    {
        Ok(response) => response,
        Err(error) => {
            eprintln!("Failed to select prompts with model: {error}");
            return Vec::new();
        }
    };
    let answer = response.text();
    let Some(json) = json_array_slice(&answer) else {
        return Vec::new();
    };
    let Ok(Value::Array(selected)) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    selected
        .into_iter()
        .filter_map(|entry| {
            let object = entry.as_object()?;
            let id = object.get("id")?.as_str()?.to_owned();
            if !prompts.iter().any(|prompt| prompt.id == id) {
                return None;
            }
            let arguments = object
                .get("arguments")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            Some((id, arguments))
        })
        .collect()
}

async fn load_resources(
    calculator: &McpClient,
    content: &McpClient,
    resources: &[RoutedResource],
    selected: &[String],
) -> String {
    let mut context = Vec::new();
    for item in resources
        .iter()
        .filter(|item| selected.iter().any(|id| id == &item.id))
    {
        println!("LLM selected resource: {}", item.id);
        let request = ReadResourceRequestParams::new(item.resource.uri.clone());
        let response = match item.server {
            ServerKind::Calculator => calculator.read_resource(request).await,
            ServerKind::Content => content.read_resource(request).await,
        };
        match response {
            Ok(response) => {
                for contents in response.contents {
                    match contents {
                        rmcp::model::ResourceContents::TextResourceContents { text, .. } => {
                            context.push(format!("[Resource: {}]\n{text}", item.id));
                        }
                        rmcp::model::ResourceContents::BlobResourceContents {
                            mime_type, ..
                        } => {
                            eprintln!("Skipping binary resource {} ({mime_type:?}).", item.id);
                        }
                        _ => eprintln!("Skipping an unsupported resource block for {}.", item.id),
                    }
                }
            }
            Err(error) => eprintln!("Could not load resource {}: {error}", item.id),
        }
    }
    context.join("\n\n")
}

async fn load_prompts(
    calculator: &McpClient,
    content: &McpClient,
    prompts: &[RoutedPrompt],
    selected: Vec<(String, Map<String, Value>)>,
) -> String {
    let mut instructions = Vec::new();
    for (id, arguments) in selected {
        let Some(item) = prompts.iter().find(|prompt| prompt.id == id) else {
            continue;
        };
        println!("Using prompt: {}", item.id);
        let request = GetPromptRequestParams::new(item.prompt.name.clone()).with_arguments(
            arguments
                .into_iter()
                .map(|(key, value)| (key, Value::String(value_to_text(value))))
                .collect::<Map<_, _>>(),
        );
        let response = match item.server {
            ServerKind::Calculator => calculator.get_prompt(request).await,
            ServerKind::Content => content.get_prompt(request).await,
        };
        match response {
            Ok(response) => {
                let text = response
                    .messages
                    .into_iter()
                    .filter_map(|message| message.content.as_text().map(|block| block.text.clone()))
                    .collect::<Vec<_>>()
                    .join("\n");
                if !text.trim().is_empty() {
                    instructions.push(format!("[Prompt: {}]\n{}", item.id, text.trim()));
                }
            }
            Err(error) => eprintln!("Could not load prompt {}: {error}", item.id),
        }
    }
    instructions.join("\n\n")
}

fn value_to_text(value: Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
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
