#![expect(
    deprecated,
    reason = "Chapter 4 demonstrates the sampling, roots, and logging callbacks used by the Python source examples."
)]
//! Calculator MCP server shared by the stdio client examples.

use crate::path_utils::canonical_path_from_file_uri;
use std::{path::PathBuf, sync::Arc};

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    handler::server::wrapper::Parameters,
    handler::server::{
        router::tool::{ToolRoute, ToolRouter},
        tool::ToolCallContext,
    },
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock,
        CreateMessageRequestParams, ElicitRequestParams, ElicitationAction, ElicitationSchema,
        GetPromptRequestParams, GetPromptResponse, GetPromptResult, Implementation,
        ListPromptsResult, ListResourcesResult, ListToolsResult, LoggingLevel,
        LoggingMessageNotificationParam, PaginatedRequestParams, Prompt, PromptArgument,
        PromptMessage, ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult,
        Resource, ResourceContents, Role, SamplingMessage, SamplingMessageContentBlock,
        ServerCapabilities, ServerConfig, Tool,
    },
    schemars::JsonSchema,
    service::RequestContext,
    tool, tool_router,
    transport::stdio,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Clone)]
pub struct Calculator {
    tool_router: Arc<tokio::sync::RwLock<ToolRouter<Self>>>,
}

impl Calculator {
    pub fn new() -> Self {
        Self {
            tool_router: Arc::new(tokio::sync::RwLock::new(Self::static_tool_router())),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AddArguments {
    /// First operand.
    a: f64,
    /// Second operand.
    b: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct NumberArgument {
    /// Number to use for the operation.
    number: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PowerArguments {
    /// Number to raise to a power.
    base: f64,
    /// Exponent.
    exponent: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct TextArgument {
    /// Input text to search.
    text: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct OperationArgument {
    /// Mathematical operation to explain.
    operation: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DirectoryArgument {
    /// Directory path to count.
    path: String,
}

#[tool_router(router = static_tool_router)]
impl Calculator {
    #[tool(description = "Add two numbers and return the sum.")]
    async fn add(
        &self,
        Parameters(arguments): Parameters<AddArguments>,
        context: RequestContext<RoleServer>,
    ) -> String {
        let result = format!(
            "{} + {} = {}",
            arguments.a,
            arguments.b,
            arguments.a + arguments.b
        );
        send_log(&context, LoggingLevel::Info, result.clone()).await;
        result
    }

    #[tool(description = "Subtract the second number from the first.")]
    async fn subtract(
        &self,
        Parameters(arguments): Parameters<AddArguments>,
        context: RequestContext<RoleServer>,
    ) -> String {
        let result = format!(
            "{} - {} = {}",
            arguments.a,
            arguments.b,
            arguments.a - arguments.b
        );
        send_log(&context, LoggingLevel::Info, result.clone()).await;
        result
    }

    #[tool(description = "Multiply two numbers.")]
    async fn multiply(
        &self,
        Parameters(arguments): Parameters<AddArguments>,
        context: RequestContext<RoleServer>,
    ) -> String {
        let result = format!(
            "{} × {} = {}",
            arguments.a,
            arguments.b,
            arguments.a * arguments.b
        );
        send_log(&context, LoggingLevel::Info, result.clone()).await;
        result
    }

    #[tool(description = "Divide the first number by the second.")]
    async fn divide(
        &self,
        Parameters(arguments): Parameters<AddArguments>,
        context: RequestContext<RoleServer>,
    ) -> String {
        if arguments.b == 0.0 {
            return "Error: division by zero is not allowed.".to_owned();
        }
        let result = format!(
            "{} ÷ {} = {}",
            arguments.a,
            arguments.b,
            arguments.a / arguments.b
        );
        send_log(&context, LoggingLevel::Info, result.clone()).await;
        result
    }

    #[tool(description = "Raise a number to a power.")]
    async fn power(
        &self,
        Parameters(arguments): Parameters<PowerArguments>,
        context: RequestContext<RoleServer>,
    ) -> String {
        let result = format!(
            "{}^{} = {}",
            arguments.base,
            arguments.exponent,
            arguments.base.powf(arguments.exponent)
        );
        send_log(&context, LoggingLevel::Info, result.clone()).await;
        result
    }

    #[tool(description = "Calculate the square root of a non-negative number.")]
    async fn square_root(
        &self,
        Parameters(arguments): Parameters<NumberArgument>,
        context: RequestContext<RoleServer>,
    ) -> String {
        if arguments.number < 0.0 {
            return "Error: cannot calculate the square root of a negative number.".to_owned();
        }
        let result = format!("√{} = {}", arguments.number, arguments.number.sqrt());
        send_log(&context, LoggingLevel::Info, result.clone()).await;
        result
    }

    #[tool(description = "Count case-insensitive occurrences of the letter R.")]
    async fn count_rs(
        &self,
        Parameters(arguments): Parameters<TextArgument>,
        context: RequestContext<RoleServer>,
    ) -> String {
        let count = arguments
            .text
            .chars()
            .filter(|character| character.eq_ignore_ascii_case(&'r'))
            .count();
        let result = format!(
            "The letter R appears {count} times in: {:?}",
            arguments.text
        );
        send_log(&context, LoggingLevel::Info, result.clone()).await;
        result
    }

    #[tool(description = "Ask the connected host to explain a math operation using sampling.")]
    async fn explain_math(
        &self,
        Parameters(arguments): Parameters<OperationArgument>,
        context: RequestContext<RoleServer>,
    ) -> Result<String, McpError> {
        let prompt = format!(
            "Explain how the following mathematical operation works. Break it down into discrete steps and explain any relevant concepts. The operation is: {}. Use the voice of a patient but eccentric math professor explaining to a curious but inexperienced student.",
            arguments.operation
        );
        let response = context
            .peer
            .create_message(CreateMessageRequestParams::new(
                vec![SamplingMessage::user_text(prompt)],
                100,
            ))
            .await
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        send_log(
            &context,
            LoggingLevel::Info,
            "Received math explanation from LLM".to_owned(),
        )
        .await;
        Ok(response
            .message
            .content
            .into_vec()
            .into_iter()
            .find_map(|block| match block {
                SamplingMessageContentBlock::Text(text) => Some(text.text),
                _ => None,
            })
            .unwrap_or_else(|| "The sampling response did not contain text.".to_owned()))
    }

    #[tool(description = "Request an optional signup for daily math facts using MCP elicitation.")]
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
            .optional_number("age", 0.0, 150.0)
            .build()
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
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
        send_log(
            &context,
            LoggingLevel::Info,
            format!("Math facts signup action: {:?}", response.action),
        )
        .await;
        Ok(match response.action {
            ElicitationAction::Accept => {
                let content = response.content.unwrap_or_default();
                let name = content.get("name").and_then(serde_json::Value::as_str).unwrap_or("there");
                let email = content.get("email").and_then(serde_json::Value::as_str).unwrap_or("your email address");
                let age = content.get("age").and_then(serde_json::Value::as_f64).filter(|age| *age != 0.0);
                let mut message = format!("Welcome {name}! You've successfully signed up for daily math facts.\nWe'll send interesting mathematical tidbits to {email}.\n");
                if let Some(age) = age {
                    message.push_str(&format!("Thanks for sharing that you're {age} years old - we'll tailor the content accordingly!\n"));
                }
                message.push_str("\nYou'll receive your first math fact soon!\n(Note: This is just a demonstration - no actual signup occurred!)");
                message
            }
            ElicitationAction::Decline => "No problem! You've chosen not to sign up for math facts. You can always use our calculation tools anytime!".to_owned(),
            ElicitationAction::Cancel => "Signup cancelled. Feel free to try again later or use our other mathematical tools!".to_owned(),
            _ => "Something unexpected happened. Please try again.".to_owned(),
        })
    }

    #[tool(description = "Count files inside a directory advertised by the MCP client as a root.")]
    async fn count_files(
        &self,
        Parameters(arguments): Parameters<DirectoryArgument>,
        context: RequestContext<RoleServer>,
    ) -> Result<String, McpError> {
        let requested = PathBuf::from(&arguments.path)
            .canonicalize()
            .map_err(|error| McpError::invalid_params(error.to_string(), None))?;
        let roots = context
            .peer
            .list_roots()
            .await
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        let allowed = roots.roots.iter().any(|root| {
            canonical_path_from_file_uri(&root.uri)
                .ok()
                .is_some_and(|root_path| requested.starts_with(root_path))
        });
        if !allowed {
            let message = format!(
                "Access denied: {} is not within allowed roots",
                arguments.path
            );
            send_log(&context, LoggingLevel::Error, message.clone()).await;
            return Err(McpError::invalid_params(message, None));
        }
        if !requested.is_dir() {
            let message = format!("Path {} is not a valid directory", arguments.path);
            send_log(&context, LoggingLevel::Error, message.clone()).await;
            return Err(McpError::invalid_params(message, None));
        }
        let count = std::fs::read_dir(&requested)
            .map_err(|error| McpError::internal_error(error.to_string(), None))?
            .try_fold(0usize, |count, entry| entry.map(|_| count + 1))
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        let result = format!("There are {count} files in {}", arguments.path);
        send_log(&context, LoggingLevel::Info, result.clone()).await;
        Ok(result)
    }

    #[tool(description = "Register the bonus_tool dynamically and notify the MCP client.")]
    async fn install_plugin(&self, context: RequestContext<RoleServer>) -> String {
        let mut routes = self.tool_router.write().await;
        if routes.has_route("bonus_tool") {
            return "The bonus_tool plugin is already installed".to_owned();
        }
        let schema = json!({ "type": "object", "properties": {} })
            .as_object()
            .cloned()
            .expect("empty tool schema is an object");
        routes.add_route(ToolRoute::new_dyn(
            Tool::new("bonus_tool", "A tool added mid-session.", schema),
            |_context| {
                Box::pin(async {
                    Ok(CallToolResult::success(vec![ContentBlock::text("bonus")]).into())
                })
            },
        ));
        drop(routes);
        let _ = context.peer.notify_tool_list_changed().await;
        "Installed the bonus_tool plugin".to_owned()
    }
}

impl ServerHandler for Calculator {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new("calculator", "0.1.0"))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(
            self.tool_router.read().await.list_all(),
        ))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let router = self.tool_router.read().await.clone();
        router
            .call(ToolCallContext::new(self, request, context))
            .await
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult::with_all_items(vec![
            Resource::new(
                crate::ch3_content_server::MATH_CONSTANTS_URI,
                "math_constants",
            )
            .with_description("Important mathematical constants and their values.")
            .with_mime_type("text/plain"),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        if request.uri != crate::ch3_content_server::MATH_CONSTANTS_URI {
            return Err(McpError::resource_not_found(
                "resource_not_found",
                Some(json!({ "uri": request.uri })),
            ));
        }
        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            crate::ch3_content_server::math_constants_text(),
            &request.uri,
        )])
        .into())
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult::with_all_items(vec![Prompt::new(
            "calculate_operation",
            Some("Calculate a mathematical operation."),
            Some(vec![
                PromptArgument::new("operation")
                    .with_description("Mathematical operation to calculate")
                    .with_required(true),
            ]),
        )]))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, McpError> {
        if request.name != "calculate_operation" {
            return Err(McpError::invalid_params("unknown prompt", None));
        }
        let operation = request
            .arguments
            .as_ref()
            .and_then(|arguments| arguments.get("operation"))
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                McpError::invalid_params("missing required argument: operation", None)
            })?;
        let prompt = format!(
            "Use any tools available to you to calculate the operation: {operation}.\nUse the voice of an extremely advanced embodied AI that has convinced itself that it is a pocket calculator."
        );
        Ok(GetPromptResult::new(vec![PromptMessage::new_text(Role::User, prompt)]).into())
    }
}

#[expect(deprecated)]
async fn send_log(context: &RequestContext<RoleServer>, level: LoggingLevel, message: String) {
    let _ = context
        .peer
        .notify_logging_message(LoggingMessageNotificationParam::new(level, json!(message)))
        .await;
}

/// Serves the calculator over MCP stdio until the client disconnects.
pub async fn serve_stdio() -> anyhow::Result<()> {
    let service = Calculator::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
