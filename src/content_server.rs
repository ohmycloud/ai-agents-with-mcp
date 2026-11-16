//! Small MCP server exposing one resource and one parameterized prompt.

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    model::{
        GetPromptRequestParams, GetPromptResponse, GetPromptResult, Implementation,
        ListPromptsResult, ListResourcesResult, Prompt, PromptArgument, PromptMessage,
        ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult, Resource,
        ResourceContents, Role, ServerCapabilities, ServerConfig,
    },
    service::RequestContext,
};
use serde_json::json;

#[derive(Clone)]
pub struct ContentServer;

/// Chapter 5.11 server: only the resource-linked knowledge prompt is exposed.
#[derive(Clone)]
pub struct ResourcePromptServer;

pub const KNOWLEDGE_URI: &str = "file://knowledge.txt";

impl ServerHandler for ContentServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new("content-server", "0.1.0"))
    }

    async fn list_resources(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult::with_all_items(vec![
            Resource::new(KNOWLEDGE_URI, "knowledge_base")
                .with_description("A short text knowledge base used by the examples.")
                .with_mime_type("text/plain"),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        if request.uri != KNOWLEDGE_URI {
            return Err(McpError::resource_not_found(
                "resource_not_found",
                Some(json!({ "uri": request.uri })),
            ));
        }

        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            include_str!("../assets/ch5/11_resource_prompt/knowledge.txt").trim_end(),
            &request.uri,
        )])
        .into())
    }

    async fn list_prompts(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult::with_all_items(vec![
            Prompt::new(
                "greet_user",
                Some("Create a short greeting for the named user."),
                Some(vec![
                    PromptArgument::new("username")
                        .with_description("Name to include in the greeting")
                        .with_required(true),
                ]),
            ),
            Prompt::new(
                "knowledge_base_prompt",
                Some("Answer a request using the server's knowledge resource."),
                Some(vec![
                    PromptArgument::new("user_request")
                        .with_description("Question to answer")
                        .with_required(true),
                ]),
            ),
        ]))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, McpError> {
        let arguments = request.arguments.as_ref();
        let result = match request.name.as_str() {
            "greet_user" => {
                let username = arguments
                    .and_then(|arguments| arguments.get("username"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        McpError::invalid_params("missing required argument: username", None)
                    })?;
                GetPromptResult::new(vec![PromptMessage::new_text(
                    Role::User,
                    format!("Say hello to the user using their name: {username}"),
                )])
                .with_description("A greeting prompt rendered for the supplied username.")
            }
            "knowledge_base_prompt" => {
                let user_request = arguments
                    .and_then(|arguments| arguments.get("user_request"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        McpError::invalid_params("missing required argument: user_request", None)
                    })?;
                GetPromptResult::new(vec![
                    PromptMessage::new_text(Role::User, user_request),
                    PromptMessage::new_text(
                        Role::User,
                        "Use the server knowledge resource to answer the request.",
                    ),
                    PromptMessage::new_resource_link(
                        Role::User,
                        Resource::new(KNOWLEDGE_URI, "knowledge_base")
                            .with_description("Knowledge resource for the prompt.")
                            .with_mime_type("text/plain"),
                    ),
                ])
                .with_description("A prompt that links the knowledge-base resource.")
            }
            name => {
                return Err(McpError::invalid_params(
                    format!("unknown prompt: {name}"),
                    None,
                ));
            }
        };

        Ok(result.into())
    }
}

impl ServerHandler for ResourcePromptServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new("resource-prompt-server", "0.1.0"))
    }

    async fn list_resources(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult::with_all_items(vec![
            Resource::new(KNOWLEDGE_URI, "knowledge_base")
                .with_description("A short text knowledge base used by the example.")
                .with_mime_type("text/plain"),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        if request.uri != KNOWLEDGE_URI {
            return Err(McpError::resource_not_found(
                "resource_not_found",
                Some(json!({ "uri": request.uri })),
            ));
        }
        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            include_str!("../assets/ch5/11_resource_prompt/knowledge.txt").trim_end(),
            &request.uri,
        )])
        .into())
    }

    async fn list_prompts(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult::with_all_items(vec![Prompt::new(
            "knowledge_base_prompt",
            Some("Answer a request using the server's knowledge resource."),
            Some(vec![
                PromptArgument::new("user_request")
                    .with_description("Question to answer")
                    .with_required(true),
            ]),
        )]))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, McpError> {
        if request.name != "knowledge_base_prompt" {
            return Err(McpError::invalid_params("unknown prompt", None));
        }
        let user_request = request
            .arguments
            .as_ref()
            .and_then(|arguments| arguments.get("user_request"))
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                McpError::invalid_params("missing required argument: user_request", None)
            })?;

        Ok(GetPromptResult::new(vec![
            PromptMessage::new_text(Role::User, user_request),
            PromptMessage::new_text(
                Role::User,
                "This prompt includes knowledge base from the resource URI: file://knowledge.txt, please use this resource to answer the user's request. The resource follows this message:",
            ),
            PromptMessage::new_resource_link(
                Role::User,
                Resource::new(KNOWLEDGE_URI, "knowledge_base")
                    .with_mime_type("text/plain"),
            ),
        ])
        .into())
    }
}

/// Runs the example content server over stdio until its client disconnects.
pub async fn serve_stdio() -> anyhow::Result<()> {
    use rmcp::{ServiceExt, transport::stdio};

    let service = ContentServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
