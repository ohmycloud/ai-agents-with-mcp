//! Chapter 3.18: load an MCP prompt and answer it with MCP calculator tools.

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
use rmcp::{ServiceExt, model::GetPromptRequestParams, transport::TokioChildProcess};
use tokio::process::Command;

#[tokio::main]
async fn main() -> Result<()> {
    match env::args().nth(1).as_deref() {
        Some("--prompt-server") => return serve_content().await,
        Some("--calculator-server") => return serve_calculator().await,
        _ => {}
    }

    load_dotenv();
    let model_client = anthropic_client()?;
    let model = model_client.completion(anthropic_model()?);
    let executable = env::current_exe()?;

    let mut prompt_command = Command::new(&executable);
    prompt_command
        .arg("--prompt-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let prompt_server = ().serve(TokioChildProcess::new(prompt_command)?).await?;

    let mut calculator_command = Command::new(executable);
    calculator_command
        .arg("--calculator-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let calculator = ().serve(TokioChildProcess::new(calculator_command)?).await?;
    let model_tools = to_model_tools(&calculator.list_all_tools().await?)?;

    println!("MCP calculation prompt ready. Ask a question (or type 'goodbye').");
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

        let rendered_prompt = prompt_server
            .get_prompt(
                GetPromptRequestParams::new("calculate_operation")
                    .with_arguments(rmcp::object!({ "operation": question })),
            )
            .await?;
        let prompt = rendered_prompt
            .messages
            .into_iter()
            .filter_map(|message| message.content.as_text().map(|text| text.text.clone()))
            .collect::<Vec<_>>()
            .join("\n");
        let answer = run_anthropic_tool_loop(&model, &calculator, &model_tools, prompt).await?;
        println!("AI Assistant: {answer}");
    }

    prompt_server.cancel().await?;
    calculator.cancel().await?;
    Ok(())
}
