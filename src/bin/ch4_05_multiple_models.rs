//! Chapter 4.5: use separate draft and review models with a full MCP client.

use std::{
    env,
    io::{self, Write},
    path::PathBuf,
    process::Stdio,
};

use ai_agents_with_mcp::{
    agent_loop::{run_anthropic_tool_loop, to_model_tools},
    calculator_server::serve_stdio,
    client_callbacks::CalculatorClientHandler,
    config::{anthropic_client, anthropic_model, load_dotenv},
};
use anyhow::{Context, Result, ensure};
use rig::completion::request::CompletionRequest;
use rmcp::{
    RoleClient, ServiceExt, handler::client::ClientHandler, model::Prompt,
    transport::TokioChildProcess,
};
use tokio::process::Command;

#[tokio::main]
async fn main() -> Result<()> {
    if env::args().nth(1).as_deref() == Some("--server") {
        return serve_stdio().await;
    }

    load_dotenv();
    let provider = anthropic_client()?;
    let default_model = anthropic_model()?;
    let draft_model_name = configured_model("ANTHROPIC_MODEL_FAST", &default_model);
    let review_model_name = configured_model("ANTHROPIC_MODEL_REVIEW", &default_model);
    let selector_model_name = configured_model("ANTHROPIC_MODEL_RESOURCE_SELECTOR", &default_model);
    let draft_model = provider.completion(draft_model_name.clone());
    let review_model = provider.completion(review_model_name.clone());
    let selector_model = provider.completion(selector_model_name);

    let root = env::var_os("MCP_ROOT")
        .map(PathBuf::from)
        .unwrap_or(env::current_dir()?);
    let root = root
        .canonicalize()
        .context("MCP_ROOT must point to an existing directory")?;
    ensure!(root.is_dir(), "MCP_ROOT must point to a directory");
    let handler = CalculatorClientHandler::new(format!("file://{}", root.display()), default_model);
    let executable = env::current_exe()?;
    let mut command = Command::new(executable);
    command
        .arg("--server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let mcp_client = handler
        .serve(TokioChildProcess::new(command)?)
        .await
        .context("could not start the local calculator MCP server")?;

    let mut tools = mcp_client.list_all_tools().await?;
    let mut model_tools = to_model_tools(&tools)?;
    let mut resources = mcp_client.list_all_resources().await?;
    let mut prompts = mcp_client.list_all_prompts().await?;

    println!("Draft model: {draft_model_name}; review model: {review_model_name}");
    println!("Calculator MCP tools: {}", tool_names(&tools));
    println!("Workspace root exposed to MCP: {}", root.display());
    println!("Type 'refresh', 'prompts', or 'goodbye'.");
    loop {
        print!("You: ");
        io::stdout().flush()?;
        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break;
        }
        let input = input.trim();
        if input.eq_ignore_ascii_case("goodbye") {
            break;
        }
        if input.is_empty() {
            continue;
        }
        if input.eq_ignore_ascii_case("refresh") {
            resources = mcp_client.list_all_resources().await?;
            prompts = mcp_client.list_all_prompts().await?;
            println!(
                "Refreshed {} resources and {} prompts.",
                resources.len(),
                prompts.len()
            );
            continue;
        }

        let prompt = if input.eq_ignore_ascii_case("prompts") {
            match collect_prompt(&mcp_client, &prompts).await? {
                Some(prompt) => prompt,
                None => continue,
            }
        } else {
            let context = select_resources(&selector_model, &mcp_client, &resources, input).await;
            if context.is_empty() {
                input.to_owned()
            } else {
                format!("Question: {input}\n\nRelevant MCP resources:\n{context}")
            }
        };

        let draft =
            run_anthropic_tool_loop(&draft_model, &mcp_client, &model_tools, prompt.clone())
                .await?;
        let reviewed = review_model
            .call(format!(
                "Improve this draft for accuracy and clarity. Return only the final answer.\n\nRequest: {prompt}\n\nDraft:\n{draft}"
            ))
            .await?;
        println!("AI Assistant: {}", reviewed.text());

        let refreshed_tools = mcp_client.list_all_tools().await?;
        if tool_names(&refreshed_tools) != tool_names(&tools) {
            println!(
                "Calculator MCP tools updated: {}",
                tool_names(&refreshed_tools)
            );
            model_tools = to_model_tools(&refreshed_tools)?;
            tools = refreshed_tools;
        }
    }

    mcp_client.cancel().await?;
    Ok(())
}

async fn select_resources<H: ClientHandler>(
    model: &rig::Model<rig::providers::anthropic::wire::Messages>,
    client: &rmcp::service::RunningService<RoleClient, H>,
    resources: &[rmcp::model::Resource],
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
            .read_resource(rmcp::model::ReadResourceRequestParams::new(
                resource.uri.clone(),
            ))
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

fn parse_resource_names(answer: &str) -> Vec<String> {
    match (answer.find('['), answer.rfind(']')) {
        (Some(start), Some(end)) if end >= start => {
            serde_json::from_str::<Vec<String>>(&answer[start..=end]).unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

async fn collect_prompt<H: ClientHandler>(
    client: &rmcp::service::RunningService<RoleClient, H>,
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
    let response = client
        .get_prompt(rmcp::model::GetPromptRequestParams::new(name).with_arguments(arguments))
        .await?;
    let rendered = response
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

fn configured_model(variable: &str, fallback: &str) -> String {
    env::var(variable)
        .ok()
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

fn tool_names(tools: &[rmcp::model::Tool]) -> String {
    tools
        .iter()
        .map(|tool| tool.name.as_ref())
        .collect::<Vec<_>>()
        .join(", ")
}
