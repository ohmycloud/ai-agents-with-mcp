//! Environment-backed configuration shared by independent examples.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, ensure};
use rig::providers::anthropic::{Anthropic, AnthropicConfig};

/// Loads a local `.env` file when present.
///
/// Shell-provided environment variables retain precedence over values in `.env`.
pub fn load_dotenv() {
    let _ = dotenvy::dotenv();
}

/// Creates the Anthropic provider using `ANTHROPIC_API_KEY` and optional
/// `ANTHROPIC_BASE_URL` environment variables.
pub fn anthropic_client() -> Result<Anthropic> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .context("set ANTHROPIC_API_KEY to configure the Anthropic provider")?;
    let api_key = api_key.trim();
    ensure!(!api_key.is_empty(), "ANTHROPIC_API_KEY must not be empty");
    let mut config = AnthropicConfig::new(api_key);

    match std::env::var("ANTHROPIC_BASE_URL") {
        Ok(base_url) if !base_url.trim().is_empty() => {
            config = config.with_base_url(base_url.trim());
        }
        Ok(_) | Err(std::env::VarError::NotPresent) => {}
        Err(error) => {
            return Err(error).context("ANTHROPIC_BASE_URL must be valid Unicode");
        }
    }

    Ok(config.client())
}

/// Returns the model identifier configured for the selected provider.
pub fn anthropic_model() -> Result<String> {
    std::env::var("ANTHROPIC_MODEL")
        .context("set ANTHROPIC_MODEL to a model identifier supported by the configured endpoint")
        .and_then(|model| {
            if model.trim().is_empty() {
                Err(anyhow!("ANTHROPIC_MODEL must not be empty"))
            } else {
                Ok(model.trim().to_owned())
            }
        })
}

/// Resolves the workspace directory exposed to calculator servers.
///
/// `MCP_ROOT` selects a directory; otherwise the process working directory is
/// used. A configured path is canonicalized so root checks compare real paths.
///
/// # Errors
///
/// Returns an error if the configured root does not exist, is not a directory,
/// or the current working directory cannot be read.
pub fn mcp_root() -> Result<PathBuf> {
    let root = match std::env::var_os("MCP_ROOT") {
        Some(root) => PathBuf::from(root)
            .canonicalize()
            .context("MCP_ROOT must point to an existing directory")?,
        None => std::env::current_dir()
            .context("could not read current directory")?
            .canonicalize()
            .context("could not canonicalize current directory")?,
    };
    ensure!(root.is_dir(), "MCP_ROOT must point to a directory");
    Ok(root)
}

/// Converts a canonical workspace path to a properly escaped file URI.
///
/// # Errors
///
/// Returns an error if the path cannot be represented as an absolute file URI.
pub fn mcp_root_uri(root: &Path) -> Result<String> {
    url::Url::from_file_path(root)
        .map(|uri| uri.into())
        .map_err(|()| anyhow!("workspace root cannot be represented as a file URI"))
}

/// Maximum number of model calls allowed in one tool loop.
///
/// The default bounds runaway tool cycles; set `MCP_MAX_MODEL_CALLS` to a
/// positive integer to tune it for a particular example or model.
///
/// # Errors
///
/// Returns an error if the environment value is not a positive integer.
pub fn max_model_calls() -> Result<usize> {
    let value = match std::env::var("MCP_MAX_MODEL_CALLS") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return Ok(32),
        Err(error) => return Err(error).context("MCP_MAX_MODEL_CALLS must be valid Unicode"),
    };
    let limit = value
        .parse::<usize>()
        .context("MCP_MAX_MODEL_CALLS must be a positive integer")?;
    ensure!(limit > 0, "MCP_MAX_MODEL_CALLS must be greater than zero");
    Ok(limit)
}
