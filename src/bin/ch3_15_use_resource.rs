//! Chapter 3.15: select MCP resources and use them alongside MCP tools.

use std::{
    env,
    io::{self, Write},
    process::Stdio,
};

use ai_agents_with_mcp::{
    agent_loop::{run_anthropic_tool_loop, to_model_tools},
    calculator_server::serve_stdio as serve_calculator,
    ch3_content_server::serve_stdio as serve_content,
    config::{anthropic_client, anthropic_model, load_dotenv},
};
use anyhow::Result;
use rig::completion::request::CompletionRequest;
use rmcp::{ServiceExt, model::ReadResourceRequestParams, transport::TokioChildProcess};
use tokio::process::Command;

#[tokio::main]
async fn main() -> Result<()> {
    match env::args().nth(1).as_deref() {
        Some("--content-server") => return serve_content().await,
        Some("--calculator-server") => return serve_calculator().await,
        _ => {}
    }

    load_dotenv();
    let model_client = anthropic_client()?;
    let model = model_client.completion(anthropic_model()?);
    let executable = env::current_exe()?;

    let mut content_command = Command::new(&executable);
    content_command
        .arg("--content-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let content = ().serve(TokioChildProcess::new(content_command)?).await?;

    let mut calculator_command = Command::new(executable);
    calculator_command
        .arg("--calculator-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let calculator = ().serve(TokioChildProcess::new(calculator_command)?).await?;

    let resources = content.list_all_resources().await?;
    let mcp_tools = calculator.list_all_tools().await?;
    let model_tools = to_model_tools(&mcp_tools)?;
    println!("Connected tools: {}", tool_names(&mcp_tools));

    println!("Ask a question (type 'goodbye' to quit).");
    loop {
        print!("You: ");
        io::stdout().flush()?;
        let mut question = String::new();
        if io::stdin().read_line(&mut question)? == 0 {
            break;
        }
        let question = question.trim();
        if question.eq_ignore_ascii_case("goodbye") {
            println!("AI Assistant: Goodbye!");
            break;
        }
        if question.is_empty() {
            continue;
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
            serde_json::to_string_pretty(&descriptions)?
        );
        let selected = match model
            .call(CompletionRequest::new(selection_prompt).max_tokens(200))
            .await
        {
            Ok(response) => {
                let answer = response.text();
                match (answer.find('['), answer.rfind(']')) {
                    (Some(start), Some(end)) if end >= start => {
                        serde_json::from_str::<Vec<String>>(&answer[start..=end])
                            .unwrap_or_default()
                    }
                    _ => Vec::new(),
                }
            }
            Err(error) => {
                eprintln!("Failed to select resources with model: {error}");
                Vec::new()
            }
        };

        let mut resource_context = Vec::new();
        for resource in resources
            .iter()
            .filter(|resource| selected.iter().any(|name| name == &resource.name))
        {
            println!("LLM selected resource: {}", resource.name);
            let result = content
                .read_resource(ReadResourceRequestParams::new(resource.uri.clone()))
                .await?;
            for item in result.contents {
                if let rmcp::model::ResourceContents::TextResourceContents { text, .. } = item {
                    resource_context.push(format!("[Resource: {}]\n{text}", resource.name));
                }
            }
        }
        let user_content = if resource_context.is_empty() {
            question.to_owned()
        } else {
            format!(
                "Question: {question}\n\nRelevant MCP resources:\n{}",
                resource_context.join("\n\n")
            )
        };
        let answer =
            run_anthropic_tool_loop(&model, &calculator, &model_tools, user_content).await?;
        println!("AI Assistant: {answer}");
    }

    content.cancel().await?;
    calculator.cancel().await?;
    Ok(())
}

fn tool_names(tools: &[rmcp::model::Tool]) -> String {
    tools
        .iter()
        .map(|tool| tool.name.as_ref())
        .collect::<Vec<_>>()
        .join(", ")
}
