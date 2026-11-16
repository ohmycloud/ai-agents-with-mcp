//! Chapter 6.1: completions for a prompt and a resource URI template.

use base64::{Engine, engine::general_purpose::STANDARD};
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    model::{
        CompleteRequestParams, CompleteResult, CompletionInfo, GetPromptRequestParams,
        GetPromptResponse, GetPromptResult, ListPromptsResult, ListResourceTemplatesResult,
        ListResourcesResult, Prompt, PromptArgument, PromptMessage, ReadResourceRequestParams,
        ReadResourceResponse, ReadResourceResult, ResourceContents, ResourceTemplate, Role,
        ServerCapabilities, ServerConfig,
    },
    service::RequestContext,
    transport::stdio,
};
use serde_json::json;

#[derive(Clone)]
struct CompletionServer;

impl ServerHandler for CompletionServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_resources()
                .enable_prompts()
                .enable_completions()
                .build(),
        )
    }

    async fn complete(
        &self,
        request: CompleteRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CompleteResult, McpError> {
        let (argument_name, mut candidates) = match &request.r#ref {
            rmcp::model::Reference::Prompt(prompt) if prompt.name == "simple_prompt_input" => {
                ("username", vec!["user".to_owned()])
            }
            rmcp::model::Reference::Resource(resource) if resource.uri == "file:///{filename}" => {
                ("filename", vec!["1.txt".to_owned(), "2.png".to_owned()])
            }
            _ => ("", Vec::new()),
        };

        if request.argument.name == argument_name
            && let Some(previous) = request
                .context
                .as_ref()
                .and_then(|context| context.get_argument(argument_name))
            && !previous.is_empty()
        {
            candidates.push(previous.clone());
        }

        let needle = request.argument.value.to_lowercase();
        let values = candidates
            .into_iter()
            .filter(|candidate| candidate.to_lowercase().contains(&needle))
            .collect::<Vec<_>>();
        let completion =
            CompletionInfo::with_pagination(values.clone(), Some(values.len() as u32), false)
                .map_err(|error| McpError::internal_error(error, None))?;
        Ok(CompleteResult::new(completion))
    }

    async fn list_prompts(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult::with_all_items(vec![Prompt::new(
            "simple_prompt_input",
            Some("Greet a named user."),
            Some(vec![PromptArgument::new("username").with_required(true)]),
        )]))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, McpError> {
        if request.name != "simple_prompt_input" {
            return Err(McpError::invalid_params("unknown prompt", None));
        }
        let username = request
            .arguments
            .as_ref()
            .and_then(|arguments| arguments.get("username"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| McpError::invalid_params("missing username", None))?;
        Ok(GetPromptResult::new(vec![PromptMessage::new_text(
            Role::User,
            format!("Say hello to the user using their name: {username}"),
        )])
        .into())
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
            ResourceTemplate::new("file:///{filename}", "resource-file")
                .with_description("Read one of the bundled completion example files."),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        let content = match request.uri.as_str() {
            "file:///1.txt" => ResourceContents::text(
                include_str!("../assets/ch6/01_completions/1.txt"),
                &request.uri,
            ),
            "file:///2.png" => ResourceContents::blob(
                STANDARD.encode(include_bytes!("../assets/ch6/01_completions/2.png")),
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

pub async fn serve_stdio() -> anyhow::Result<()> {
    let service = CompletionServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
