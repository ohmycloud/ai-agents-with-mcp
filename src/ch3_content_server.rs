//! Chapter 3 calculator content: the math-constants resource and calculation prompt.

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    model::{
        GetPromptRequestParams, GetPromptResponse, GetPromptResult, ListPromptsResult,
        ListResourcesResult, PaginatedRequestParams, Prompt, PromptArgument, PromptMessage,
        ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult, Resource,
        ResourceContents, Role, ServerCapabilities, ServerConfig,
    },
    service::RequestContext,
    transport::stdio,
};
use serde_json::json;

#[derive(Clone)]
pub struct Chapter3ContentServer;

pub const MATH_CONSTANTS_URI: &str = "resource://math-constants";

impl ServerHandler for Chapter3ContentServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult::with_all_items(vec![
            Resource::new(MATH_CONSTANTS_URI, "math_constants")
                .with_description("Important mathematical constants and their values.")
                .with_mime_type("text/plain"),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        if request.uri != MATH_CONSTANTS_URI {
            return Err(McpError::resource_not_found(
                "resource_not_found",
                Some(json!({ "uri": request.uri })),
            ));
        }

        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            math_constants_text(),
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
            "Use any tools available to you to calculate the operation: {operation}.\n\
             Use the voice of an extremely advanced embodied AI that has convinced\n\
             itself that it is a pocket calculator."
        );
        Ok(GetPromptResult::new(vec![PromptMessage::new_text(Role::User, prompt)]).into())
    }
}

pub fn math_constants_text() -> String {
    let phi = (1.0 + 5.0_f64.sqrt()) / 2.0;
    let constants = [
        ("π (Pi)", std::f64::consts::PI),
        ("e (Euler's number)", std::f64::consts::E),
        ("τ (Tau)", std::f64::consts::TAU),
        ("φ (Golden ratio)", phi),
        ("√2 (Square root of 2)", 2.0_f64.sqrt()),
        ("√3 (Square root of 3)", 3.0_f64.sqrt()),
        ("ln(2) (Natural log of 2)", std::f64::consts::LN_2),
        ("ln(10) (Natural log of 10)", std::f64::consts::LN_10),
    ];
    let mut result = String::from("Mathematical Constants:\n=========================\n\n");
    for (name, value) in constants {
        result.push_str(&format!("{name:<25} = {value:.10}\n"));
    }
    result.push_str("\nThese constants can be used in calculations with the calculator tools.");
    result
}

/// Serve the Chapter 3 content server over stdio until its client disconnects.
pub async fn serve_stdio() -> anyhow::Result<()> {
    let service = Chapter3ContentServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
