//! Chapter 4.6: resolve server connection details from the MCP Registry.

use std::{env, process::Stdio};

use anyhow::{Context, Result, bail};
use rmcp::{ServiceExt, transport::TokioChildProcess};
use serde_json::Value;
use tokio::process::Command;

const REGISTRY: &str = "https://registry.modelcontextprotocol.io";

#[tokio::main]
async fn main() -> Result<()> {
    let examples = [
        ("ai.anomalyarmor/armor-mcp", "streamable-http"),
        ("ai.anomalyarmor/armor-mcp", "stdio"),
        ("com.mcparmory/github", "stdio"),
    ];
    for (name, transport) in examples {
        match resolve_server(name, transport).await {
            Ok(settings) => println!("{name} ({transport}): {settings}"),
            Err(error) => eprintln!("{name} ({transport}): could not resolve: {error:#}"),
        }
    }

    if env::var("RUN_REGISTRY_CONNECT").ok().as_deref() != Some("1") {
        println!("\nSet RUN_REGISTRY_CONNECT=1 to run the resolved stdio package with uvx.");
        return Ok(());
    }

    let settings = resolve_server("ai.anomalyarmor/armor-mcp", "stdio").await?;
    let command = settings
        .get("command")
        .and_then(Value::as_str)
        .context("registry package did not specify a runtime command")?;
    let args = settings
        .get("server_args")
        .and_then(Value::as_array)
        .context("registry package did not specify runtime arguments")?;
    let executable = args.iter().filter_map(Value::as_str).collect::<Vec<_>>();
    let mut child = Command::new(command);
    child
        .args(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let client = ().serve(TokioChildProcess::new(child)?).await?;
    println!(
        "Connected; server offers {} tools.",
        client.list_all_tools().await?.len()
    );
    client.cancel().await?;
    Ok(())
}

async fn resolve_server(name: &str, transport: &str) -> Result<Value> {
    let encoded_name = name.replace('/', "%2F");
    let response = reqwest::Client::new()
        .get(format!(
            "{REGISTRY}/v0.1/servers/{encoded_name}/versions/latest"
        ))
        .send()
        .await?
        .error_for_status()?;
    let payload: Value = response.json().await?;
    let server = payload
        .get("server")
        .context("registry response has no server field")?;
    match transport {
        "streamable-http" => {
            let remotes = server
                .get("remotes")
                .and_then(Value::as_array)
                .context("server has no remote transports")?;
            let remote = remotes
                .iter()
                .find(|entry| entry.get("type").and_then(Value::as_str) == Some(transport))
                .context("server has no Streamable HTTP transport")?;
            Ok(serde_json::json!({"server_url": remote.get("url")}))
        }
        "stdio" => {
            let packages = server
                .get("packages")
                .and_then(Value::as_array)
                .context("server has no installable packages")?;
            let package = packages
                .iter()
                .find(|entry| {
                    entry.pointer("/transport/type").and_then(Value::as_str) == Some("stdio")
                })
                .context("server has no stdio package")?;
            let runtime = package
                .get("runtimeHint")
                .and_then(Value::as_str)
                .or_else(
                    || match package.get("registryType").and_then(Value::as_str) {
                        Some("pypi") => Some("uvx"),
                        Some("npm") => Some("npx"),
                        _ => None,
                    },
                )
                .context("no supported package runtime was declared")?;
            if !matches!(runtime, "uvx" | "npx") {
                bail!("unsupported runtime: {runtime}");
            }
            let mut args = package
                .get("runtimeArguments")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|argument| argument.get("value").and_then(Value::as_str))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            args.push(
                package
                    .get("identifier")
                    .and_then(Value::as_str)
                    .context("package has no identifier")?
                    .to_owned(),
            );
            Ok(serde_json::json!({"command": runtime, "server_args": args}))
        }
        other => bail!("unsupported transport: {other}"),
    }
}
