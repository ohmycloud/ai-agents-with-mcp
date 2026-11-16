//! Environment-backed configuration shared by independent examples.

use anyhow::{Context, Result, anyhow};
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
    let mut config = AnthropicConfig::new(api_key);

    match std::env::var("ANTHROPIC_BASE_URL") {
        Ok(base_url) if !base_url.trim().is_empty() => {
            config = config.with_base_url(base_url);
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
                Ok(model)
            }
        })
}
