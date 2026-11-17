//! Chapter 3.15: select MCP resources and use them alongside MCP tools.

use std::{
    env,
    io::{self, Write},
    process::Stdio,
};

use ai_agents_with_mcp::{
    agent_loop::{run_anthropic_tool_loop_with_message, to_model_tools},
    calculator_server::serve_stdio as serve_calculator,
    ch3_content_server::serve_stdio as serve_content,
    config::{anthropic_client, anthropic_model, load_dotenv},
    interactive_agent::{select_resources, user_message_with_resources},
};
use anyhow::Result;
use rmcp::{ServiceExt, transport::TokioChildProcess};
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

    let run_result: Result<()> = async {
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

            let context = select_resources(&model, &content, &resources, question).await;
            let user_message = user_message_with_resources(question, context);
            let answer = run_anthropic_tool_loop_with_message(
                &model,
                &calculator,
                &model_tools,
                user_message,
            )
            .await?;
            println!("AI Assistant: {answer}");
        }
        Ok(())
    }
    .await;

    let content_cancel_result = content.cancel().await;
    let calculator_cancel_result = calculator.cancel().await;
    run_result?;
    content_cancel_result?;
    calculator_cancel_result?;
    Ok(())
}

fn tool_names(tools: &[rmcp::model::Tool]) -> String {
    tools
        .iter()
        .map(|tool| tool.name.as_ref())
        .collect::<Vec<_>>()
        .join(", ")
}
