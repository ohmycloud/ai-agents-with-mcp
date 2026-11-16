//! Server-side examples for MCP context, notifications, pagination, roots, and cancellation.

use std::{collections::HashSet, path::PathBuf, sync::Arc, time::Duration};

use base64::{Engine, engine::general_purpose::STANDARD};
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    handler::server::wrapper::{Json, Parameters},
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, CancelledNotificationParam,
        ContentBlock, ElicitRequestParams, ElicitationAction, ElicitationSchema,
        GetPromptRequestParams, GetPromptResponse, GetPromptResult, Icon, ListPromptsResult,
        ListResourceTemplatesResult, ListResourcesResult, ListToolsResult, LoggingLevel,
        LoggingMessageNotificationParam, PaginatedRequestParams, ProgressNotificationParam, Prompt,
        PromptArgument, PromptMessage, ReadResourceRequestParams, ReadResourceResponse,
        ReadResourceResult, Resource, ResourceContents, ResourceTemplate,
        ResourceUpdatedNotificationParam, Role, ServerCapabilities, ServerConfig, Tool,
    },
    service::{NotificationContext, RequestContext},
    transport::stdio,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Clone)]
pub struct IconServer;

fn chapter_icons() -> Vec<Icon> {
    let brain = Icon::new(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(include_bytes!("../assets/ch6/02_server_icons/brain.png"))
    ))
    .with_mime_type("image/png")
    .with_sizes(vec!["128x128".to_owned()]);
    let sloth = Icon::new(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(include_bytes!("../assets/ch6/02_server_icons/sloth.png"))
    ))
    .with_mime_type("image/png")
    .with_sizes(vec!["128x128".to_owned()]);
    vec![brain, sloth]
}

impl ServerHandler for IconServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .enable_resources()
                .build(),
        )
        .with_server_info(
            rmcp::model::Implementation::new("icons-server", "0.1.0").with_icons(chapter_icons()),
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let schema = json!({ "type": "object", "properties": {} })
            .as_object()
            .cloned()
            .expect("empty tool schema is an object");
        Ok(ListToolsResult::with_all_items(vec![
            Tool::new(
                "think",
                "A tool that makes the server think with an icon.",
                schema,
            )
            .with_icons(vec![chapter_icons().remove(0)]),
        ]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        match request.name.as_ref() {
            "think" => {
                Ok(CallToolResult::success(vec![ContentBlock::text("I'm thinking...")]).into())
            }
            _ => Err(McpError::invalid_params("unknown tool", None)),
        }
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult::with_all_items(vec![
            Prompt::new("sloth_prompt", Some("Think slowly, like a sloth."), None)
                .with_icons(vec![chapter_icons().remove(1)]),
        ]))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, McpError> {
        if request.name != "sloth_prompt" {
            return Err(McpError::invalid_params("unknown prompt", None));
        }
        Ok(GetPromptResult::new(vec![PromptMessage::new_text(
            Role::User,
            "Think very slowly, like a sloth",
        )])
        .into())
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult::with_all_items(vec![
            ResourceTemplate::new("file:///{filename}", "icon-resource")
                .with_description("A resource template with two icons.")
                .with_icons(chapter_icons()),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        if !request.uri.starts_with("file:///") {
            return Err(McpError::resource_not_found("resource not found", None));
        }
        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            "Here is your file!",
            &request.uri,
        )])
        .into())
    }
}

#[derive(Clone)]
pub struct ServerContextServer;

#[rmcp::tool_router]
impl ServerContextServer {
    #[rmcp::tool(description = "Get information about the server.")]
    fn get_server_information(&self, _context: RequestContext<RoleServer>) -> Json<Value> {
        Json(json!({
            "server_name": "context-object-server-info-server",
            "server_instructions": "Use this server to get information about the server.",
            "server_website_url": "https://www.google.com",
            "server_icon_count": chapter_icons().len(),
            "server_debug_mode": true,
            "server_log_level": "DEBUG"
        }))
    }

    #[rmcp::tool(description = "Return this server's configuration settings.")]
    fn get_server_configuration(&self, _context: RequestContext<RoleServer>) -> Json<Value> {
        Json(json!({
            "debug": true,
            "log_level": "DEBUG"
        }))
    }
}

#[rmcp::tool_handler]
impl ServerHandler for ServerContextServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("Use this server to get information about the server.")
            .with_server_info(
                rmcp::model::Implementation::new("context-object-server-info-server", "0.1.0")
                    .with_website_url("https://www.google.com")
                    .with_icons(chapter_icons()),
            )
    }
}

#[derive(Clone)]
pub struct SessionContextServer {
    knowledge_base: Arc<tokio::sync::RwLock<String>>,
}

const KNOWLEDGE_BASE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ch6/04_context_object_session_info/knowledge.txt"
);

impl SessionContextServer {
    fn new() -> Self {
        Self {
            knowledge_base: Arc::new(tokio::sync::RwLock::new(
                std::fs::read_to_string(KNOWLEDGE_BASE_PATH)
                    .unwrap_or_else(|_| "This is a font of knowledge.\n".to_owned()),
            )),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AddFactRequest {
    fact: String,
}

#[rmcp::tool_router]
impl SessionContextServer {
    #[rmcp::tool(description = "Return the connected MCP client's session information.")]
    fn get_client_info(&self, context: RequestContext<RoleServer>) -> String {
        format!("client peer information: {:#?}", context.peer.peer_info())
    }

    #[rmcp::tool(description = "Append a fact to the knowledge resource and notify subscribers.")]
    async fn add_fact_to_knowledge_base(
        &self,
        Parameters(request): Parameters<AddFactRequest>,
        context: RequestContext<RoleServer>,
    ) -> String {
        let mut knowledge = self.knowledge_base.write().await;
        let mut updated = knowledge.clone();
        updated.push_str(&request.fact);
        updated.push('\n');
        if let Err(error) = std::fs::write(KNOWLEDGE_BASE_PATH, &updated) {
            return format!("Could not update the knowledge resource: {error}");
        }
        *knowledge = updated;
        drop(knowledge);

        match context
            .peer
            .notify_resource_updated(ResourceUpdatedNotificationParam::new(
                "file://knowledge.txt",
            ))
            .await
        {
            Ok(()) => "Added the fact and notified resource subscribers.".to_owned(),
            Err(error) => format!("Added the fact, but notification failed: {error}"),
        }
    }
}

#[rmcp::tool_handler]
impl ServerHandler for SessionContextServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_resources_subscribe()
                .build(),
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult::with_all_items(vec![
            Resource::new("file://knowledge.txt", "knowledge")
                .with_description("A knowledge base that tools can extend.")
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
                Some(serde_json::json!({ "uri": request.uri })),
            ));
        }
        let text = self.knowledge_base.read().await.clone();
        Ok(ReadResourceResult::new(vec![ResourceContents::text(text, &request.uri)]).into())
    }
}

#[derive(Clone)]
pub struct LoggingServer;

#[derive(Debug, Deserialize, JsonSchema)]
struct AddRequest {
    a: f64,
    b: f64,
}

#[rmcp::tool_router(server_handler)]
impl LoggingServer {
    #[rmcp::tool(description = "Add two numbers and emit MCP logging notifications.")]
    async fn add(
        &self,
        Parameters(request): Parameters<AddRequest>,
        context: RequestContext<RoleServer>,
    ) -> Json<Value> {
        send_log(
            &context,
            LoggingLevel::Info,
            format!("{:?}: add called", std::time::SystemTime::now()),
        )
        .await;
        let result = request.a + request.b;
        let value = json!({ "augend": request.a, "addend": request.b, "sum": result });
        send_log(
            &context,
            LoggingLevel::Debug,
            format!("{:?}: add result: {value}", std::time::SystemTime::now()),
        )
        .await;
        if result < 0.0 {
            send_log(
                &context,
                LoggingLevel::Warning,
                format!(
                    "{:?}: add result is negative: {result}",
                    std::time::SystemTime::now()
                ),
            )
            .await;
        }
        Json(value)
    }
}

#[allow(deprecated)]
async fn send_log(context: &RequestContext<RoleServer>, level: LoggingLevel, message: String) {
    let _ = context
        .peer
        .notify_logging_message(
            LoggingMessageNotificationParam::new(level, json!({ "message": message }))
                .with_logger("calculator"),
        )
        .await;
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RemovePromptRequest {
    prompt_name: String,
}

#[derive(Clone)]
pub struct ManualNotificationsServer {
    prompts: Arc<tokio::sync::Mutex<HashSet<String>>>,
}

impl ManualNotificationsServer {
    fn new() -> Self {
        Self {
            prompts: Arc::new(tokio::sync::Mutex::new(HashSet::from([
                "hello_prompt".to_owned(),
                "calculate_operation".to_owned(),
            ]))),
        }
    }
}

#[rmcp::tool_router]
impl ManualNotificationsServer {
    #[rmcp::tool(description = "Remove a prompt and notify clients that the prompt list changed.")]
    async fn remove_prompt(
        &self,
        Parameters(request): Parameters<RemovePromptRequest>,
        context: RequestContext<RoleServer>,
    ) -> String {
        if !self.prompts.lock().await.remove(&request.prompt_name) {
            return format!("Prompt {} not found", request.prompt_name);
        }
        match context.peer.notify_prompt_list_changed().await {
            Ok(()) => format!("Removed prompt {}.", request.prompt_name),
            Err(error) => format!("Removed prompt, but could not notify clients: {error}"),
        }
    }
}

#[rmcp::tool_handler]
impl ServerHandler for ManualNotificationsServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .enable_prompts_list_changed()
                .build(),
        )
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        let prompts = self.prompts.lock().await;
        let mut result = Vec::new();
        if prompts.contains("hello_prompt") {
            result.push(Prompt::new("hello_prompt", None::<String>, None));
        }
        if prompts.contains("calculate_operation") {
            result.push(Prompt::new(
                "calculate_operation",
                Some("Calculate a mathematical operation."),
                Some(vec![PromptArgument::new("operation").with_required(true)]),
            ));
        }
        Ok(ListPromptsResult::with_all_items(result))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, McpError> {
        if !self.prompts.lock().await.contains(&request.name) {
            return Err(McpError::invalid_params("prompt not found", None));
        }
        let text = match request.name.as_str() {
            "hello_prompt" => "Tell the user hello, welcome to the MCP server!".to_owned(),
            "calculate_operation" => {
                let operation = request
                    .arguments
                    .as_ref()
                    .and_then(|arguments| arguments.get("operation"))
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| McpError::invalid_params("missing operation", None))?;
                format!(
                    "Use any tools available to you to calculate the operation: {operation}. Use the voice of an extremely advanced embodied AI that has convinced itself that it is a pocket calculator."
                )
            }
            _ => return Err(McpError::invalid_params("prompt not found", None)),
        };
        Ok(GetPromptResult::new(vec![PromptMessage::new_text(Role::User, text)]).into())
    }
}

#[derive(Clone)]
pub struct PaginationServer;

const RESOURCE_COUNT: usize = 1_000;
const PAGE_SIZE: usize = 100;

impl ServerHandler for PaginationServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_resources().build())
    }

    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let start = request
            .and_then(|params| params.cursor)
            .and_then(|cursor| cursor.parse::<usize>().ok())
            .unwrap_or(0)
            .min(RESOURCE_COUNT);
        let end = (start + PAGE_SIZE).min(RESOURCE_COUNT);
        let resources = (start..end)
            .map(|index| {
                Resource::new(format!("resource://{index}"), format!("Resource {index}"))
                    .with_description(format!("This is resource number {index}."))
                    .with_mime_type("text/plain")
            })
            .collect();
        let mut result = ListResourcesResult::with_all_items(resources);
        result.next_cursor = (end < RESOURCE_COUNT).then(|| end.to_string());
        Ok(result)
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        let index = request
            .uri
            .strip_prefix("resource://")
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|index| *index < RESOURCE_COUNT)
            .ok_or_else(|| McpError::resource_not_found("resource not found", None))?;
        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            format!("This is resource number {index}."),
            &request.uri,
        )])
        .into())
    }
}

#[derive(Clone)]
pub struct ElicitationServer;

#[rmcp::tool_router]
impl ElicitationServer {
    #[rmcp::tool(description = "Request an optional signup action from the connected user.")]
    async fn signup_math_facts(
        &self,
        context: RequestContext<RoleServer>,
    ) -> Result<String, McpError> {
        let requested_schema = ElicitationSchema::builder()
            .required_string_with("name", |schema| {
                schema
                    .title("Full Name")
                    .description("Your full name")
                    .min_length(1)
            })
            .required_email("email")
            .optional_integer("age", 0, 150)
            .build()
            .map_err(|error| McpError::internal_error(error, None))?;
        let response = context
            .peer
            .create_elicitation(ElicitRequestParams::FormElicitationParams {
                meta: None,
                message: "Please provide your information to sign up for daily math facts!"
                    .to_owned(),
                requested_schema,
            })
            .await
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        Ok(match response.action {
            ElicitationAction::Accept => {
                let content = response.content.unwrap_or_default();
                let name = content
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("there");
                let email = content
                    .get("email")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("your email address");
                let age = content
                    .get("age")
                    .and_then(serde_json::Value::as_i64)
                    .filter(|age| *age != 0);
                let mut message = format!(
                    "Welcome {name}! You've successfully signed up for daily math facts.\nWe'll send interesting mathematical tidbits to {email}.\n"
                );
                if let Some(age) = age {
                    message.push_str(&format!(
                        "Thanks for sharing that you're {age} years old - we'll tailor the content accordingly!\n"
                    ));
                }
                message.push_str(
                    "\nYou'll receive your first math fact soon!\n(Note: This is just a demonstration - no actual signup occurred!)",
                );
                message
            }
            ElicitationAction::Decline => "No problem! You've chosen not to sign up for math facts. You can always use our calculation tools anytime!".to_owned(),
            ElicitationAction::Cancel => "Signup cancelled. Feel free to try again later or use our other mathematical tools!".to_owned(),
            _ => "Something unexpected happened. Please try again.".to_owned(),
        })
    }
}

#[rmcp::tool_handler]
impl ServerHandler for ElicitationServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(rmcp::model::ProtocolVersion::V_2025_11_25)
    }
}

#[derive(Clone)]
pub struct RootsServer;

#[derive(Debug, Deserialize, JsonSchema)]
struct CountFilesRequest {
    path: String,
}

#[rmcp::tool_router]
impl RootsServer {
    #[rmcp::tool(description = "Count files inside one of the roots provided by the client.")]
    async fn count_files(
        &self,
        Parameters(request): Parameters<CountFilesRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<String, McpError> {
        let requested = match PathBuf::from(&request.path).canonicalize() {
            Ok(path) => path,
            Err(error) => {
                send_log(&context, LoggingLevel::Error, error.to_string()).await;
                return Err(McpError::invalid_params(error.to_string(), None));
            }
        };
        let roots = context
            .peer
            .list_roots()
            .await
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        let allowed = roots.roots.iter().any(|root| {
            root.uri
                .strip_prefix("file://")
                .map(PathBuf::from)
                .and_then(|path| path.canonicalize().ok())
                .is_some_and(|root_path| requested.starts_with(root_path))
        });
        if !allowed {
            let message = format!(
                "Access denied: {} is not within allowed roots",
                request.path
            );
            send_log(&context, LoggingLevel::Error, message.clone()).await;
            return Err(McpError::invalid_params(message, None));
        }
        if !requested.is_dir() {
            let message = format!("Path {} is not a valid directory", request.path);
            send_log(&context, LoggingLevel::Error, message.clone()).await;
            return Err(McpError::invalid_params(message, None));
        }
        let count = std::fs::read_dir(requested)
            .map_err(|error| McpError::internal_error(error.to_string(), None))?
            .try_fold(0usize, |count, entry| entry.map(|_| count + 1))
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        send_log(
            &context,
            LoggingLevel::Info,
            format!("Counting files in {} = {count}", request.path),
        )
        .await;
        Ok(format!("There are {count} files in {}", request.path))
    }
}

#[rmcp::tool_handler]
impl ServerHandler for RootsServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(rmcp::model::ProtocolVersion::V_2025_11_25)
    }
}

#[derive(Clone)]
pub struct CancellationServer;

#[derive(Debug, Deserialize, JsonSchema)]
struct SlowOperationRequest {
    #[serde(default = "default_slow_operation_length")]
    length: usize,
}

fn default_slow_operation_length() -> usize {
    100
}

#[rmcp::tool_router]
impl CancellationServer {
    #[rmcp::tool(description = "Run a cancellable operation with progress updates.")]
    async fn slow_operation(
        &self,
        Parameters(request): Parameters<SlowOperationRequest>,
        context: RequestContext<RoleServer>,
    ) -> Result<String, McpError> {
        let token = context.meta.get_progress_token();
        for step in 1..=request.length {
            tokio::select! {
                _ = context.ct.cancelled() => {
                    eprintln!("slow_operation cancelled at step {step}; cleaning up");
                    return Err(McpError::internal_error("tool request cancelled", None));
                },
                _ = tokio::time::sleep(Duration::from_millis(100)) => {}
            }
            if let Some(token) = &token {
                let _ = context
                    .peer
                    .notify_progress(
                        ProgressNotificationParam::new(token.clone(), step as f64)
                            .with_total(request.length as f64),
                    )
                    .await;
            }
        }
        Ok(format!("Completed all {} steps", request.length))
    }
}

#[rmcp::tool_handler]
impl ServerHandler for CancellationServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn on_cancelled(
        &self,
        notification: CancelledNotificationParam,
        _context: NotificationContext<RoleServer>,
    ) {
        eprintln!(
            "Cancelled notification received for request ID {:?}: {:?}",
            notification.request_id, notification.reason
        );
    }
}

pub async fn serve_icon_stdio() -> anyhow::Result<()> {
    run(IconServer).await
}
pub async fn serve_server_context_stdio() -> anyhow::Result<()> {
    run(ServerContextServer).await
}
pub async fn serve_session_context_stdio() -> anyhow::Result<()> {
    run(SessionContextServer::new()).await
}
pub async fn serve_logging_stdio() -> anyhow::Result<()> {
    run(LoggingServer).await
}
pub async fn serve_notifications_stdio() -> anyhow::Result<()> {
    run(ManualNotificationsServer::new()).await
}
pub async fn serve_pagination_stdio() -> anyhow::Result<()> {
    run(PaginationServer).await
}
pub async fn serve_elicitation_stdio() -> anyhow::Result<()> {
    run(ElicitationServer).await
}
pub async fn serve_roots_stdio() -> anyhow::Result<()> {
    run(RootsServer).await
}
pub async fn serve_cancellation_stdio() -> anyhow::Result<()> {
    run(CancellationServer).await
}

async fn run<S>(server: S) -> anyhow::Result<()>
where
    S: ServiceExt<RoleServer>,
{
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
