//! Explicit Anthropic tool loop shared by the chapter examples.

use anyhow::{Result, bail};
use rig::{
    Model,
    completion::{
        message::{AssistantContent, Message, ToolName, ToolResultContent},
        request::{CompletionRequest, ToolDefinition},
    },
    providers::anthropic::wire::Messages,
};
use rmcp::{
    RoleClient, handler::client::ClientHandler, model::Tool as McpTool, service::RunningService,
};

use crate::{config::max_model_calls, tool_results::content_as_strings};

/// Convert MCP tool schemas into provider-neutral Rig definitions.
///
/// # Errors
///
/// Returns an error if an MCP tool name is invalid for Rig or its schema cannot
/// be serialized.
pub fn to_model_tools(tools: &[McpTool]) -> Result<Vec<ToolDefinition>> {
    tools
        .iter()
        .map(|tool| {
            Ok(ToolDefinition::new(
                ToolName::new(tool.name.clone())?,
                tool.description.as_deref().unwrap_or_default(),
                serde_json::to_value(tool.input_schema.as_ref())?,
            ))
        })
        .collect()
}

/// Run a handwritten tool loop against one initialized MCP client.
///
/// # Errors
///
/// Returns an error if model configuration or a model request fails, or if the
/// configured model-call limit is reached before a final response.
pub async fn run_anthropic_tool_loop<H: ClientHandler>(
    model: &Model<Messages>,
    client: &RunningService<RoleClient, H>,
    tools: &[ToolDefinition],
    prompt: impl Into<String>,
) -> Result<String> {
    run_anthropic_tool_loop_with_message(model, client, tools, Message::user(prompt.into())).await
}

/// Run the tool loop with a prebuilt message, including typed image context.
///
/// # Errors
///
/// Returns an error if model configuration or a model request fails, or if the
/// configured model-call limit is reached before a final response.
pub async fn run_anthropic_tool_loop_with_message<H: ClientHandler>(
    model: &Model<Messages>,
    client: &RunningService<RoleClient, H>,
    tools: &[ToolDefinition],
    initial_message: Message,
) -> Result<String> {
    let mut history = Vec::new();
    let mut next_message = initial_message;
    let max_calls = max_model_calls()?;

    for _ in 0..max_calls {
        let sent_message = next_message.clone();
        let response = model
            .call(
                CompletionRequest::new(next_message)
                    .messages(history.iter().cloned())
                    .tools(tools.iter().cloned())
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
            return Ok(if answer.trim().is_empty() {
                "[No text response available]".to_owned()
            } else {
                answer
            });
        }

        let mut results = Vec::with_capacity(calls.len());
        for call in calls {
            let name = call.function.name.as_str();
            println!("Using tool: {name}");
            let request = rmcp::model::CallToolRequestParams::new(name.to_owned())
                .with_arguments(call.function.arguments.clone());
            let (content, is_error) = match client.call_tool(request).await {
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
