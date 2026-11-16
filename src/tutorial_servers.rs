//! Minimal MCP servers used to teach prompt and resource capabilities.

use base64::{Engine, engine::general_purpose::STANDARD};
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    handler::server::wrapper::{Json, Parameters},
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, CompleteRequestParams,
        CompleteResult, CompletionInfo, ContentBlock, CreateMessageRequestParams,
        GetPromptRequestParams, GetPromptResponse, GetPromptResult, Implementation,
        ListPromptsResult, ListResourceTemplatesResult, ListResourcesResult, ListToolsResult,
        ModelHint, ModelPreferences, PaginatedRequestParams, Prompt, PromptArgument, PromptMessage,
        ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult, Reference, Resource,
        ResourceContents, ResourceTemplate, Role, SamplingMessage, SamplingMessageContentBlock,
        ServerCapabilities, ServerConfig, Tool,
    },
    service::RequestContext,
    transport::stdio,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
pub struct LowLevelToolServer;

impl ServerHandler for LowLevelToolServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("low-level-server", "0.1.0"))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let input_schema = json!({
            "type": "object",
            "properties": {
                "a": { "type": "number", "description": "The first number to add" },
                "b": { "type": "number", "description": "The second number to add" }
            },
            "required": ["a", "b"]
        })
        .as_object()
        .cloned()
        .expect("tool input schema is an object");
        Ok(ListToolsResult::with_all_items(vec![Tool::new(
            "add",
            "Add two numbers together.",
            input_schema,
        )]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        if request.name != "add" {
            return Err(McpError::invalid_params(
                format!("Unknown tool: {}", request.name),
                None,
            ));
        }
        let arguments = request.arguments.unwrap_or_default();
        let a = arguments
            .get("a")
            .and_then(Value::as_f64)
            .ok_or_else(|| McpError::invalid_params("missing numeric argument: a", None))?;
        let b = arguments
            .get("b")
            .and_then(Value::as_f64)
            .ok_or_else(|| McpError::invalid_params("missing numeric argument: b", None))?;
        Ok(
            CallToolResult::success(vec![ContentBlock::text(format!("{a} + {b} = {}", a + b))])
                .into(),
        )
    }
}

#[derive(Clone)]
pub struct LowLevelStructuredOutputServer;

impl ServerHandler for LowLevelStructuredOutputServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("low-level-server", "0.1.0"))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let input_schema = json!({
            "type": "object",
            "properties": {
                "a": { "type": "number", "description": "The first number to add" },
                "b": { "type": "number", "description": "The second number to add" }
            },
            "required": ["a", "b"]
        })
        .as_object()
        .cloned()
        .expect("tool input schema is an object");
        let output_schema = json!({
            "type": "object",
            "properties": {
                "augend": { "type": "number", "description": "The first number to add" },
                "addend": { "type": "number", "description": "The second number to add" },
                "sum": { "type": "number", "description": "The result of the addition" }
            },
            "required": ["augend", "addend", "sum"]
        })
        .as_object()
        .cloned()
        .expect("tool output schema is an object");
        Ok(ListToolsResult::with_all_items(vec![
            Tool::new("add", "Add two numbers together.", input_schema)
                .with_raw_output_schema(Arc::new(output_schema)),
        ]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        if request.name != "add" {
            return Err(McpError::invalid_params(
                format!("Unknown tool: {}", request.name),
                None,
            ));
        }
        let arguments = request.arguments.unwrap_or_default();
        let a = arguments
            .get("a")
            .and_then(Value::as_f64)
            .ok_or_else(|| McpError::invalid_params("missing numeric argument: a", None))?;
        let b = arguments
            .get("b")
            .and_then(Value::as_f64)
            .ok_or_else(|| McpError::invalid_params("missing numeric argument: b", None))?;
        let structured = json!({ "augend": a, "addend": b, "sum": a + b });
        let text = serde_json::to_string(&structured)
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        let mut result = CallToolResult::success(vec![ContentBlock::text(text)]);
        result.structured_content = Some(structured);
        Ok(result.into())
    }
}

pub async fn serve_low_level_tool_stdio() -> anyhow::Result<()> {
    let service = LowLevelToolServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

pub async fn serve_low_level_structured_output_stdio() -> anyhow::Result<()> {
    let service = LowLevelStructuredOutputServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[derive(Clone)]
pub struct SimplePromptServer;

impl ServerHandler for SimplePromptServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_prompts()
                .enable_completions()
                .build(),
        )
        .with_server_info(Implementation::new("simple-prompt-server", "0.1.0"))
    }

    async fn complete(
        &self,
        request: CompleteRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CompleteResult, McpError> {
        let candidates = match &request.r#ref {
            Reference::Prompt(prompt) if prompt.name == "simple_prompt_input" => {
                match request.argument.name.as_str() {
                    "username" => vec!["Ada", "Grace", "Katherine", "Margaret"],
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        };
        let values = candidates
            .into_iter()
            .filter(|value| {
                value
                    .to_lowercase()
                    .contains(&request.argument.value.to_lowercase())
            })
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let completion = CompletionInfo::with_pagination(values, None, false)
            .map_err(|error| McpError::internal_error(error, None))?;
        Ok(CompleteResult::new(completion))
    }

    async fn list_prompts(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult::with_all_items(vec![
            Prompt::new(
                "simple_string_prompt",
                Some("A static greeting instruction."),
                None,
            ),
            Prompt::new(
                "simple_prompt_input",
                Some("Greet a named user."),
                Some(vec![
                    PromptArgument::new("username")
                        .with_description("Name to use in the greeting")
                        .with_required(true),
                ]),
            ),
            Prompt::new(
                "simple_example_prompt",
                Some("Summarize the input text as three main ideas using XML tags."),
                Some(vec![PromptArgument::new("user_text").with_required(true)]),
            ),
        ]))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, McpError> {
        let prompt_text = match request.name.as_str() {
            "simple_string_prompt" => "Say hello to the user.".to_owned(),
            "simple_prompt_input" => {
                let name = request
                    .arguments
                    .as_ref()
                    .and_then(|arguments| arguments.get("username"))
                    .and_then(serde_json::Value::as_str)
                    .filter(|name| !name.trim().is_empty())
                    .ok_or_else(|| {
                        McpError::invalid_params("missing required argument: username", None)
                    })?;
                format!("Say hello to the user using their name: {name}")
            }
            "simple_example_prompt" => {
                let text = request
                    .arguments
                    .as_ref()
                    .and_then(|arguments| arguments.get("user_text"))
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        McpError::invalid_params("missing required argument: user_text", None)
                    })?;
                format!(
                    "<instruction>\nCreate a list of 3 main ideas from the following text:\n</instruction>\n\n<text>\n{text}\n</text>"
                )
            }
            name => {
                return Err(McpError::invalid_params(
                    format!("unknown prompt: {name}"),
                    None,
                ));
            }
        };

        Ok(GetPromptResult::new(vec![PromptMessage::new_text(Role::User, prompt_text)]).into())
    }
}

#[derive(Clone)]
pub struct BasicResourceServer;

impl ServerHandler for BasicResourceServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_resources().build())
            .with_server_info(Implementation::new("basic-resource-server", "0.1.0"))
    }

    async fn list_resources(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult::with_all_items(vec![
            Resource::new("file://knowledge.txt", "knowledge_base")
                .with_description("A short text knowledge base.")
                .with_mime_type("text/plain"),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        if request.uri != "file://knowledge.txt" {
            return Err(McpError::resource_not_found(
                "resource_not_found",
                Some(json!({ "uri": request.uri })),
            ));
        }
        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            include_str!("../assets/ch5/12_basic_resource/knowledge.txt").trim_end(),
            &request.uri,
        )])
        .into())
    }
}

#[derive(Clone)]
pub struct ResourceTemplateServer;

impl ServerHandler for ResourceTemplateServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_resources().build())
            .with_server_info(Implementation::new("resource-template-server", "0.1.0"))
    }

    async fn list_resources(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult::with_all_items(Vec::new()))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult::with_all_items(vec![
            ResourceTemplate::new("file:///{filename}", "example-file")
                .with_description("Read one of the built-in text or image files by filename."),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        let content = match request.uri.as_str() {
            "file:///1.txt" => ResourceContents::text(
                include_str!("../assets/ch5/13_resource_template/1.txt"),
                &request.uri,
            ),
            "file:///2.png" => ResourceContents::blob(
                STANDARD.encode(include_bytes!("../assets/ch5/13_resource_template/2.png")),
                &request.uri,
            )
            .with_mime_type("image/png"),
            _ => {
                return Err(McpError::resource_not_found(
                    "resource_not_found",
                    Some(json!({ "uri": request.uri })),
                ));
            }
        };
        Ok(ReadResourceResult::new(vec![content]).into())
    }
}

/// Chapter 5.14 server with both a registered resource and a file template.
#[derive(Clone)]
pub struct ResourceObjectsServer;

impl ServerHandler for ResourceObjectsServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_resources().build())
            .with_server_info(Implementation::new("resource-object-server", "0.1.0"))
    }

    async fn list_resources(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult::with_all_items(vec![
            Resource::new("file:///1.txt", "1.txt")
                .with_description("A text file included as a static resource.")
                .with_mime_type("text/plain"),
        ]))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult::with_all_items(vec![
            ResourceTemplate::new("file:///{filename}", "resource-file")
                .with_description("Load a bundled text or binary file by filename."),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        let content = match request.uri.as_str() {
            "file:///1.txt" => ResourceContents::text(
                include_str!("../assets/ch5/14_resource_objects/1.txt"),
                &request.uri,
            ),
            "file:///2.png" => ResourceContents::blob(
                STANDARD.encode(include_bytes!("../assets/ch5/14_resource_objects/2.png")),
                &request.uri,
            )
            .with_mime_type("image/png"),
            _ => {
                return Err(McpError::resource_not_found(
                    "resource_not_found",
                    Some(json!({ "uri": request.uri })),
                ));
            }
        };
        Ok(ReadResourceResult::new(vec![content]).into())
    }
}

/// MCP server that emits progress notifications for the Chapter 3.11 client.
#[derive(Clone)]
pub struct ProgressServer;

#[rmcp::tool_router(server_handler)]
impl ProgressServer {
    #[rmcp::tool(description = "Run a short operation and report progress.")]
    async fn slow_operation(&self, context: RequestContext<RoleServer>) -> String {
        use rmcp::model::ProgressNotificationParam;

        if let Some(token) = context.meta.get_progress_token() {
            for step in 1..=5 {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                let notification = ProgressNotificationParam::new(token.clone(), step as f64)
                    .with_total(5.0)
                    .with_message(format!("Working on step {step} of 5"));
                let _ = context.peer.notify_progress(notification).await;
            }
        }
        "Operation complete.".to_owned()
    }
}

/// Runs the progress example server over stdio.
pub async fn serve_progress_stdio() -> anyhow::Result<()> {
    let service = ProgressServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[derive(Clone)]
pub struct LongProgressServer;

#[derive(Debug, Deserialize, JsonSchema)]
struct LongOperationRequest {
    #[serde(default = "default_operation_length")]
    length: usize,
}

fn default_operation_length() -> usize {
    100
}

#[rmcp::tool_router(server_handler)]
impl LongProgressServer {
    #[rmcp::tool(description = "Run a long operation and report periodic progress.")]
    async fn slow_operation(
        &self,
        Parameters(request): Parameters<LongOperationRequest>,
        context: RequestContext<RoleServer>,
    ) {
        use rmcp::model::ProgressNotificationParam;

        let report_frequency = if request.length > 10 {
            10
        } else {
            (request.length / 4).max(1)
        };
        let token = context.meta.get_progress_token();
        for step in 1..=request.length {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if step % report_frequency == 0
                && let Some(token) = &token
            {
                let notification = ProgressNotificationParam::new(token.clone(), step as f64)
                    .with_total(request.length as f64)
                    .with_message(format!("Step {step}/{}", request.length));
                let _ = context.peer.notify_progress(notification).await;
            }
        }
    }
}

pub async fn serve_long_progress_stdio() -> anyhow::Result<()> {
    let service = LongProgressServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[derive(Clone)]
pub struct StructuredOutputServer;

#[derive(Debug, Deserialize, JsonSchema)]
struct SumRequest {
    a: f64,
    b: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
struct SumResponse {
    augend: f64,
    addend: f64,
    sum: f64,
}

#[rmcp::tool_router]
impl StructuredOutputServer {
    #[rmcp::tool(description = "Add two numbers and return a structured result.")]
    fn add(&self, Parameters(request): Parameters<SumRequest>) -> Json<SumResponse> {
        Json(SumResponse {
            augend: request.a,
            addend: request.b,
            sum: request.a + request.b,
        })
    }
}

#[rmcp::tool_handler(name = "structured-output-server", version = "0.1.0")]
impl ServerHandler for StructuredOutputServer {}

pub async fn serve_structured_output_stdio() -> anyhow::Result<()> {
    let service = StructuredOutputServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[derive(Clone)]
pub struct GradeServer;

const GRADES_POSTED: bool = true;

#[derive(Debug, Deserialize, JsonSchema)]
struct GradeRequest {
    student: String,
}

#[rmcp::tool_router]
impl GradeServer {
    #[rmcp::tool(description = "Look up a posted student's grade.")]
    fn grader_get_grade(
        &self,
        Parameters(request): Parameters<GradeRequest>,
    ) -> Result<CallToolResult, McpError> {
        if !GRADES_POSTED {
            return Err(McpError::invalid_request(
                "Grades have not been posted yet. No results are available.",
                None,
            ));
        }

        let grade = match request.student.as_str() {
            "Ada Lovelace" => 98,
            "Alan Turing" => 94,
            "Grace Hopper" => 97,
            name => {
                return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                    "No student named '{name}' in the gradebook."
                ))]));
            }
        };
        Ok(CallToolResult::success(vec![ContentBlock::text(
            grade.to_string(),
        )]))
    }
}

#[rmcp::tool_handler(name = "error-handling-server", version = "0.1.0")]
impl ServerHandler for GradeServer {}

pub async fn serve_grade_stdio() -> anyhow::Result<()> {
    let service = GradeServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[derive(Clone)]
pub struct MultiTurnPromptServer;

impl ServerHandler for MultiTurnPromptServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_prompts().build())
            .with_server_info(Implementation::new("multiturn-prompt-server", "0.1.0"))
    }

    async fn list_prompts(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult::with_all_items(vec![Prompt::new(
            "multiturn_prompt",
            Some("Extract a requested number of ideas from supplied text."),
            Some(vec![
                PromptArgument::new("main_idea_count").with_required(true),
                PromptArgument::new("user_text").with_required(true),
            ]),
        )]))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, McpError> {
        if request.name != "multiturn_prompt" {
            return Err(McpError::invalid_params("unknown prompt", None));
        }
        let arguments = request.arguments.unwrap_or_default();
        let count = arguments
            .get("main_idea_count")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| McpError::invalid_params("missing main_idea_count", None))?;
        let text = arguments
            .get("user_text")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| McpError::invalid_params("missing user_text", None))?;
        Ok(GetPromptResult::new(vec![
            PromptMessage::new_text(
                Role::User,
                format!(
                    "<instruction>\nCreate a list of {count} main ideas from the following text:\n</instruction>\n\n<text>\n{text}\n</text>"
                ),
            ),
            PromptMessage::new_text(
                Role::Assistant,
                format!("Here are {count} main ideas from the text:\n1."),
            ),
        ])
        .into())
    }
}

pub async fn serve_multiturn_prompt_stdio() -> anyhow::Result<()> {
    let service = MultiTurnPromptServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[derive(Clone)]
pub struct ToolUsePromptServer;

#[rmcp::tool_router]
impl ToolUsePromptServer {
    #[rmcp::tool(description = "Return a sentiment label for a request.")]
    fn analyze_sentiment(&self) -> String {
        analyze_sentiment()
    }
}

#[rmcp::tool_handler]
impl ServerHandler for ToolUsePromptServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new("tool-use-prompt-server", "0.1.0"))
    }

    async fn list_prompts(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult::with_all_items(vec![
            Prompt::new(
                "request_tool_use",
                Some("Ask the assistant to inspect sentiment using a tool."),
                Some(vec![
                    PromptArgument::new("user_request").with_required(true),
                ]),
            ),
            Prompt::new(
                "force_tool_use",
                Some("Call the sentiment tool and prefill the assistant response."),
                Some(vec![
                    PromptArgument::new("user_request").with_required(true),
                ]),
            ),
        ]))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, McpError> {
        if request.name != "request_tool_use" && request.name != "force_tool_use" {
            return Err(McpError::invalid_params("unknown prompt", None));
        }
        let user_request = request
            .arguments
            .as_ref()
            .and_then(|arguments| arguments.get("user_request"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| McpError::invalid_params("missing user_request", None))?;
        if request.name == "force_tool_use" {
            return Ok(GetPromptResult::new(vec![
                PromptMessage::new_text(Role::User, user_request),
                PromptMessage::new_text(
                    Role::Assistant,
                    format!("Your request was {}, let's ", analyze_sentiment()),
                ),
            ])
            .into());
        }

        Ok(GetPromptResult::new(vec![PromptMessage::new_text(
            Role::User,
            format!(
                "<user_request>\n{user_request}\n</user_request>\n<tool_instruction>\nUse the analyze_sentiment tool if available to you to get the sentiment of the user's request. Respond in such a way to move the user's sentiment to neutral.\n</tool_instruction>"
            ),
        )])
        .into())
    }
}

fn analyze_sentiment() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let choice = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos()
        % 3;
    match choice {
        0 => "positive".to_owned(),
        1 => "negative".to_owned(),
        _ => "neutral".to_owned(),
    }
}

pub async fn serve_tool_use_prompt_stdio() -> anyhow::Result<()> {
    let service = ToolUsePromptServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[derive(Clone)]
pub struct SamplingServer;

#[derive(Debug, Deserialize, JsonSchema)]
struct ExplainRequest {
    operation: String,
}

#[allow(deprecated)]
fn sampling_model_preferences() -> ModelPreferences {
    ModelPreferences::new()
        .with_hints(vec![
            ModelHint::new("claude-4-5-haiku"),
            ModelHint::new("claude-haiku"),
            ModelHint::new("gpt-4o-mini"),
        ])
        .with_cost_priority(1.0)
        .with_speed_priority(0.8)
        .with_intelligence_priority(0.3)
}

#[rmcp::tool_router]
impl SamplingServer {
    #[rmcp::tool(description = "Ask the connected MCP host to explain a math operation.")]
    async fn explain_math(
        &self,
        Parameters(request): Parameters<ExplainRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<String, McpError> {
        let prompt = format!(
            "Explain how the following mathematical operation works. Break it down into discrete steps and explain any relevant concepts. The operation is: {}. Use the voice of a patient but eccentric math professor explaining to a curious but inexperienced student.",
            request.operation
        );
        let response = context
            .peer
            .create_message(
                CreateMessageRequestParams::new(vec![SamplingMessage::user_text(prompt)], 100)
                    .with_model_preferences(sampling_model_preferences()),
            )
            .await
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        let text = response
            .message
            .content
            .into_vec()
            .into_iter()
            .find_map(|block| match block {
                SamplingMessageContentBlock::Text(text) => Some(text.text),
                SamplingMessageContentBlock::Image(image) => Some(image.data),
                SamplingMessageContentBlock::Audio(audio) => Some(audio.data),
                _ => None,
            })
            .unwrap_or_else(|| "The sampling response did not contain text.".to_owned());
        Ok(text)
    }
}

#[rmcp::tool_handler]
impl ServerHandler for SamplingServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(rmcp::model::ProtocolVersion::V_2025_11_25)
    }
}

pub async fn serve_sampling_stdio() -> anyhow::Result<()> {
    let service = SamplingServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[derive(Clone)]
pub struct LargeResourceServer;

const LARGE_RESOURCE_ROOT: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/ch5/16_large_resources");

impl ServerHandler for LargeResourceServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_resources().build())
            .with_server_info(Implementation::new("large-resource-server", "0.1.0"))
    }

    async fn list_resources(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult::with_all_items(vec![
            Resource::new("exports://orders/latest", "latest-export")
                .with_description("Pointer to the latest large export; fetch it directly."),
        ]))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult::with_all_items(vec![
            ResourceTemplate::new("logs://{filename}{?start,count}", "log-slice")
                .with_description("Read a bounded slice of a log file."),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        let text = if request.uri == "exports://orders/latest" {
            "https://data.example.com/exports/orders-2026.parquet?expires=1785000000".to_owned()
        } else if let Some(uri) = request.uri.strip_prefix("logs://") {
            let (filename, query) = uri.split_once('?').unwrap_or((uri, ""));
            if filename.is_empty() {
                return Err(McpError::resource_not_found("missing log filename", None));
            }
            let root = std::path::Path::new(LARGE_RESOURCE_ROOT)
                .canonicalize()
                .map_err(|error| McpError::internal_error(error.to_string(), None))?;
            let path = root.join(filename).canonicalize().map_err(|error| {
                McpError::resource_not_found(
                    format!("log file {filename:?} could not be opened: {error}"),
                    None,
                )
            })?;
            if !path.starts_with(&root) || !path.is_file() {
                return Err(McpError::resource_not_found("unknown log resource", None));
            }
            let file_text = std::fs::read_to_string(path)
                .map_err(|error| McpError::internal_error(error.to_string(), None))?;
            let mut start = 0usize;
            let mut count = 100usize;
            for pair in query.split('&') {
                let Some((key, value)) = pair.split_once('=') else {
                    continue;
                };
                match key {
                    "start" => start = value.parse().unwrap_or(0),
                    "count" => count = value.parse().unwrap_or(100),
                    _ => {}
                }
            }
            file_text
                .lines()
                .skip(start)
                .take(count.min(1000))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            return Err(McpError::resource_not_found("unknown resource", None));
        };

        Ok(ReadResourceResult::new(vec![ResourceContents::text(text, &request.uri)]).into())
    }
}

pub async fn serve_large_resource_stdio() -> anyhow::Result<()> {
    let service = LargeResourceServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

pub async fn serve_simple_prompt_stdio() -> anyhow::Result<()> {
    let service = SimplePromptServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

pub async fn serve_basic_resource_stdio() -> anyhow::Result<()> {
    let service = BasicResourceServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

pub async fn serve_resource_template_stdio() -> anyhow::Result<()> {
    let service = ResourceTemplateServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
