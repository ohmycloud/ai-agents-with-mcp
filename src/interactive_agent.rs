//! Shared resource selection and prompt loading for interactive MCP hosts.

use std::io::{self, Write};

use crate::agent_loop::{run_anthropic_tool_loop, to_model_tools};
use anyhow::Result;
use rig::{Model, completion::request::CompletionRequest, providers::anthropic::wire::Messages};
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

        let prompt = if input.eq_ignore_ascii_case("prompts") {
            match collect_prompt(client, &prompts).await? {
                Some(prompt) => prompt,
                None => continue,
            }
        } else {
            let context = select_resources(resource_selector, client, &resources, input).await;
            if context.is_empty() {
                input.to_owned()
            } else {
                format!("Question: {input}\n\nRelevant MCP resources:\n{context}")
            }
        };

        let answer = run_anthropic_tool_loop(model, client, &tools, prompt).await?;
        println!("Assistant: {answer}");
    }
    Ok(())
}

/// Ask the model which advertised resources are relevant and load their text.
pub async fn select_resources<H: ClientHandler>(
    model: &Model<Messages>,
    client: &RunningService<RoleClient, H>,
    resources: &[Resource],
    question: &str,
) -> String {
    if resources.is_empty() {
        return String::new();
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
                        rmcp::model::ResourceContents::TextResourceContents { text, .. } => {
                            context.push(format!("[Resource: {}]\n{text}", resource.name));
                        }
                        rmcp::model::ResourceContents::BlobResourceContents {
                            mime_type, ..
                        } => {
                            eprintln!(
                                "Skipping binary resource {} ({mime_type:?}).",
                                resource.name
                            );
                        }
                        _ => eprintln!("Skipping an unsupported resource content block."),
                    }
                }
            }
            Err(error) => eprintln!("Could not load resource {}: {error}", resource.name),
        }
    }
    context.join("\n\n")
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
        print!("  {label}: ");
        io::stdout().flush()?;
        let mut value = String::new();
        io::stdin().read_line(&mut value)?;
        if value.trim().is_empty() && argument.required.unwrap_or(false) {
            print!("  {} is required; enter a value: ", argument.name);
            io::stdout().flush()?;
            io::stdin().read_line(&mut value)?;
        }
        if !value.trim().is_empty() {
            arguments.insert(argument.name.clone(), value.trim().to_owned().into());
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
