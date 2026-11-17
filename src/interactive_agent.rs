//! Shared resource selection and prompt loading for interactive MCP hosts.

use std::io::{self, Write};

use crate::agent_loop::{run_anthropic_tool_loop_with_message, to_model_tools};
use anyhow::Result;
use rig::{
    Model,
    completion::{
        message::{DocumentMediaType, ImageMediaType, Message, MimeType, UserContent},
        request::CompletionRequest,
    },
    providers::anthropic::wire::Messages,
};
use rmcp::{
    RoleClient,
    handler::client::ClientHandler,
    model::{GetPromptRequestParams, Prompt, ReadResourceRequestParams, Resource},
    service::RunningService,
};

/// Run the interactive behavior shared by the first three Chapter 4 agents.
///
/// The Python clients load tools once at startup and refresh resources and
/// prompts on demand, so this loop follows the same lifecycle.
pub async fn run_interactive_agent<H: ClientHandler>(
    model: &Model<Messages>,
    resource_selector: &Model<Messages>,
    client: &RunningService<RoleClient, H>,
) -> Result<()> {
    println!(
        "Welcome to your AI Assistant. Type 'goodbye' to quit or 'refresh' to reload and redisplay available resources."
    );
    let available_tools = client.list_all_tools().await?;
    let tools = to_model_tools(&available_tools)?;
    let mut resources = client.list_all_resources().await?;
    let mut prompts = client.list_all_prompts().await?;

    loop {
        print!("You: ");
        io::stdout().flush()?;
        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break;
        }
        let input = input.trim();

        if input.eq_ignore_ascii_case("goodbye") {
            println!("AI Assistant: Goodbye!");
            break;
        }
        if input.eq_ignore_ascii_case("refresh") {
            resources = client.list_all_resources().await?;
            prompts = client.list_all_prompts().await?;
            println!(
                "Refreshed {} resources and {} prompts.",
                resources.len(),
                prompts.len()
            );
            continue;
        }
        if input.is_empty() {
            continue;
        }

        let user_message = if input.eq_ignore_ascii_case("prompts") {
            match collect_prompt(client, &prompts).await? {
                Some(prompt) => Message::user(prompt),
                None => continue,
            }
        } else {
            let context = select_resources(resource_selector, client, &resources, input).await;
            user_message_with_resources(input, context)
        };

        let answer =
            run_anthropic_tool_loop_with_message(model, client, &tools, user_message).await?;
        println!("Assistant: {answer}");
    }
    Ok(())
}

/// Build a user turn with its original question and selected resource blocks.
pub fn user_message_with_resources(question: &str, resources: Vec<UserContent>) -> Message {
    let mut content = Vec::with_capacity(resources.len() + 1);
    content.push(UserContent::text(question));
    content.extend(resources);
    Message::from(content)
}

/// Ask the model which resources are relevant and load supported content.
pub async fn select_resources<H: ClientHandler>(
    model: &Model<Messages>,
    client: &RunningService<RoleClient, H>,
    resources: &[Resource],
    question: &str,
) -> Vec<UserContent> {
    if resources.is_empty() {
        return Vec::new();
    }

    let descriptions = resources
        .iter()
        .map(|resource| {
            (
                resource.name.clone(),
                serde_json::Value::String(
                    resource
                        .description
                        .clone()
                        .unwrap_or_else(|| format!("Resource: {}", resource.name)),
                ),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let selection_prompt = format!(
        "Given this user question: {question:?}\n\nAvailable MCP resources:\n{}\n\nReturn only a JSON array of resource names that are directly relevant, or an empty array if none are relevant.",
        serde_json::to_string_pretty(&descriptions).unwrap_or_default()
    );
    let selected = match model
        .call(CompletionRequest::new(selection_prompt).max_tokens(200))
        .await
    {
        Ok(response) => parse_resource_names(&response.text()),
        Err(error) => {
            eprintln!("Failed to select resources with model: {error}");
            Vec::new()
        }
    };

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
            Ok(result) => {
                for content in result.contents {
                    match content {
                        resource_block => context.extend(resource_content_as_user_content(
                            &resource.name,
                            resource_block,
                        )),
                    }
                }
            }
            Err(error) => eprintln!("Could not load resource {}: {error}", resource.name),
        }
    }
    context
}

/// Convert an MCP resource block into provider-typed user content. Text and
/// common Anthropic image formats are retained; PDFs are also passed through.
pub fn resource_content_as_user_content(
    resource_name: &str,
    content: rmcp::model::ResourceContents,
) -> Vec<UserContent> {
    match content {
        rmcp::model::ResourceContents::TextResourceContents { text, .. } => {
            vec![UserContent::text(format!(
                "[Resource: {resource_name}]\n{text}"
            ))]
        }
        rmcp::model::ResourceContents::BlobResourceContents {
            mime_type, blob, ..
        } => {
            let mime_type = mime_type.unwrap_or_default();
            let normalized_mime = mime_type.to_ascii_lowercase();
            if let Some(image_type) = ImageMediaType::from_mime_type(&normalized_mime)
                && matches!(
                    &image_type,
                    ImageMediaType::JPEG
                        | ImageMediaType::PNG
                        | ImageMediaType::GIF
                        | ImageMediaType::WEBP
                )
            {
                return vec![
                    UserContent::text(format!("[Resource: {resource_name} image]")),
                    UserContent::image_base64(blob, Some(image_type), None),
                ];
            }
            if normalized_mime == "application/pdf"
                && let Some(document_type) = DocumentMediaType::from_mime_type(&normalized_mime)
            {
                return vec![
                    UserContent::text(format!("[Resource: {resource_name} document]")),
                    UserContent::document_base64(blob, Some(document_type)),
                ];
            }
            eprintln!("Skipping unsupported resource {resource_name} ({mime_type:?}).");
            Vec::new()
        }
        _ => {
            eprintln!("Skipping an unsupported resource block for {resource_name}.");
            Vec::new()
        }
    }
}

/// Display server prompts, collect their declared arguments, and render one.
pub async fn collect_prompt<H: ClientHandler>(
    client: &RunningService<RoleClient, H>,
    prompts: &[Prompt],
) -> Result<Option<String>> {
    if prompts.is_empty() {
        println!("No prompts available on this server.");
        return Ok(None);
    }

    println!("Available prompts:");
    for prompt in prompts {
        println!(
            "  {}: {}",
            prompt.name,
            prompt.description.as_deref().unwrap_or("(no description)")
        );
        for argument in prompt.arguments.as_deref().unwrap_or_default() {
            let required = if argument.required.unwrap_or(false) {
                "required"
            } else {
                "optional"
            };
            println!(
                "      - {} ({required}) {}",
                argument.name,
                argument.description.as_deref().unwrap_or("")
            );
        }
    }

    print!("Select a prompt by name (Enter to cancel): ");
    io::stdout().flush()?;
    let mut name = String::new();
    io::stdin().read_line(&mut name)?;
    let name = name.trim();
    if name.is_empty() {
        return Ok(None);
    }
    let Some(prompt) = prompts.iter().find(|prompt| prompt.name == name) else {
        println!("No prompt named {name:?}.");
        return Ok(None);
    };

    let mut arguments = serde_json::Map::new();
    for argument in prompt.arguments.as_deref().unwrap_or_default() {
        let label = match argument.description.as_deref() {
            Some(description) if !description.is_empty() => {
                format!("{} ({description})", argument.name)
            }
            _ => argument.name.clone(),
        };
        let required = argument.required.unwrap_or(false);
        loop {
            print!("  {label}: ");
            io::stdout().flush()?;
            let mut value = String::new();
            if io::stdin().read_line(&mut value)? == 0 {
                println!("Input closed; prompt was cancelled.");
                return Ok(None);
            }
            let value = value.trim();
            if value.is_empty() {
                if required {
                    println!("  {} is required.", argument.name);
                    continue;
                }
                break;
            }
            arguments.insert(argument.name.clone(), value.to_owned().into());
            break;
        }
    }

    let rendered = client
        .get_prompt(GetPromptRequestParams::new(name).with_arguments(arguments))
        .await?
        .messages
        .into_iter()
        .filter_map(|message| message.content.as_text().map(|text| text.text.clone()))
        .collect::<Vec<_>>()
        .join("\n");
    if rendered.is_empty() {
        println!("The selected prompt returned no text messages.");
        Ok(None)
    } else {
        Ok(Some(rendered))
    }
}

fn parse_resource_names(answer: &str) -> Vec<String> {
    match (answer.find('['), answer.rfind(']')) {
        (Some(start), Some(end)) if end >= start => {
            serde_json::from_str::<Vec<String>>(&answer[start..=end]).unwrap_or_default()
        }
        _ => Vec::new(),
    }
}
