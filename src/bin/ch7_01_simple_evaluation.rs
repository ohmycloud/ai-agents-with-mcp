//! Chapter 7.1: score model tool selection against a small known-answer set.

use ai_agents_with_mcp::config::{anthropic_client, anthropic_model, load_dotenv};
use anyhow::Result;
use rig::completion::{
    message::{AssistantContent, ToolName},
    request::{CompletionRequest, ToolDefinition},
};

const CASES: [(&str, &str); 4] = [
    ("What is 2 + 2?", "add"),
    ("What is 2 - 2?", "subtract"),
    ("What is 2 * 2?", "multiply"),
    ("What is 2 / 2?", "divide"),
];

#[tokio::main]
async fn main() -> Result<()> {
    load_dotenv();
    let model = anthropic_client()?.completion(anthropic_model()?);
    let tools = arithmetic_tools()?;
    let mut score = 0;

    for (prompt, expected) in CASES {
        let response = model
            .call(
                CompletionRequest::new(prompt)
                    .tools(tools.iter().cloned())
                    .max_tokens(4096),
            )
            .await?;
        let selected = response
            .choice
            .iter()
            .filter_map(|item| match item {
                AssistantContent::ToolCall(call) => Some(call.function.name.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let correct = selected.len() == 1 && selected[0] == expected;
        score += usize::from(correct);
        println!(
            "{} -> expected {expected}, selected {:?}: {}",
            prompt,
            selected,
            if correct { "correct" } else { "incorrect" }
        );
    }

    println!(
        "Tool-choice score: {score}/{} ({:.0}%)",
        CASES.len(),
        score as f64 * 100.0 / CASES.len() as f64
    );
    Ok(())
}

fn arithmetic_tools() -> Result<Vec<ToolDefinition>> {
    let schema = || {
        serde_json::json!({
            "type": "object",
            "properties": {
                "a": {"type": "number", "description": "First operand"},
                "b": {"type": "number", "description": "Second operand"}
            },
            "required": ["a", "b"]
        })
    };
    [
        ("add", "Add two numbers."),
        ("subtract", "Subtract the second number from the first."),
        ("multiply", "Multiply two numbers."),
        ("divide", "Divide the first number by the second."),
    ]
    .into_iter()
    .map(|(name, description)| {
        Ok(ToolDefinition::new(
            ToolName::new(name)?,
            description,
            schema(),
        ))
    })
    .collect()
}
