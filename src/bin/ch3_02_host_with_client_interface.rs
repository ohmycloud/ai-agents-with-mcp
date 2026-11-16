//! Chapter 3.2: keep the host's chat loop independent from MCP client details.

use std::io::{self, Write};

use ai_agents_with_mcp::config::{anthropic_client, anthropic_model, load_dotenv};
use anyhow::Result;
use rig::completion::message::AssistantContent;

#[tokio::main]
async fn main() -> Result<()> {
    load_dotenv();
    let model = anthropic_client()?.completion(anthropic_model()?);
    println!("Welcome to your AI Assistant. Type 'goodbye' to quit.");

    loop {
        print!("You: ");
        io::stdout().flush()?;
        let mut prompt = String::new();
        if io::stdin().read_line(&mut prompt)? == 0 {
            break;
        }
        let prompt = prompt.trim();
        if prompt.eq_ignore_ascii_case("goodbye") {
            println!("AI Assistant: Goodbye!");
            break;
        }
        if prompt.is_empty() {
            continue;
        }

        let response = model.call(prompt.to_owned()).await?;
        for item in response.choice {
            if let AssistantContent::Text(text) = item {
                println!("AI Assistant: {}", text.text);
            }
        }
    }
    Ok(())
}
