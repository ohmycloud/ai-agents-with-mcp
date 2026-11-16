#![expect(
    deprecated,
    reason = "The callbacks intentionally implement the sampling, roots, and logging behavior used in the Python chapter clients."
)]
//! Shared MCP client callbacks used by the multi-model examples.

use std::{
    env,
    future::Future,
    io::{self, Write},
    process::Command,
};

use rig::completion::request::CompletionRequest;
use rmcp::{
    RoleClient,
    handler::client::ClientHandler,
    model::{
        ClientCapabilities, ClientConfig, CreateMessageRequestParams, CreateMessageResult,
        ElicitRequestParams, ElicitResult, ElicitationAction, Implementation, ListRootsResult,
        LoggingLevel, LoggingMessageNotificationParam, ProgressNotificationParam, ProtocolVersion,
        Root, SamplingMessage, SamplingMessageContentBlock,
    },
    service::{NotificationContext, RequestContext},
};

#[derive(Clone)]
pub struct CalculatorClientHandler {
    root_uri: String,
    default_model: String,
    features: CallbackFeatures,
    use_environment_defaults: bool,
    debug_logging: bool,
}

#[derive(Clone, Copy)]
pub struct CallbackFeatures {
    pub sampling: bool,
    pub roots: bool,
    pub elicitation: bool,
}

impl CallbackFeatures {
    pub const SAMPLING: Self = Self {
        sampling: true,
        roots: false,
        elicitation: false,
    };

    pub const SAMPLING_AND_ROOTS: Self = Self {
        sampling: true,
        roots: true,
        elicitation: false,
    };

    pub const ALL: Self = Self {
        sampling: true,
        roots: true,
        elicitation: true,
    };
}

impl CalculatorClientHandler {
    pub fn new(root_uri: impl Into<String>, default_model: impl Into<String>) -> Self {
        Self::for_chapter(root_uri, default_model, CallbackFeatures::ALL, true, false)
    }

    pub fn for_chapter(
        root_uri: impl Into<String>,
        default_model: impl Into<String>,
        features: CallbackFeatures,
        use_environment_defaults: bool,
        debug_logging: bool,
    ) -> Self {
        Self {
            root_uri: root_uri.into(),
            default_model: default_model.into(),
            features,
            use_environment_defaults,
            debug_logging,
        }
    }
}

impl ClientHandler for CalculatorClientHandler {
    fn get_info(&self) -> ClientConfig {
        let mut capabilities = ClientCapabilities::builder().build();
        if self.features.sampling {
            capabilities.sampling = ClientCapabilities::builder()
                .enable_sampling()
                .build()
                .sampling;
        }
        if self.features.roots {
            capabilities.roots = ClientCapabilities::builder().enable_roots().build().roots;
        }
        if self.features.elicitation {
            capabilities.elicitation = ClientCapabilities::builder()
                .enable_elicitation()
                .build()
                .elicitation;
        }
        ClientConfig::new(
            capabilities,
            Implementation::new("rust-multiple-models-host", env!("CARGO_PKG_VERSION")),
        )
        .with_protocol_version(ProtocolVersion::V_2025_11_25)
    }

    async fn create_message(
        &self,
        request: CreateMessageRequestParams,
        _context: RequestContext<RoleClient>,
    ) -> Result<CreateMessageResult, rmcp::ErrorData> {
        let prompt = request
            .messages
            .into_iter()
            .flat_map(|message| message.content.into_vec())
            .filter_map(|block| match block {
                SamplingMessageContentBlock::Text(text) => Some(text.text),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        let model_name = configured_model("ANTHROPIC_MODEL_SAMPLING", &self.default_model);
        let response = crate::config::anthropic_client()
            .map_err(|error| rmcp::ErrorData::internal_error(error.to_string(), None))?
            .completion(model_name.clone())
            .call(CompletionRequest::new(prompt).max_tokens(u64::from(request.max_tokens)))
            .await
            .map_err(|error| rmcp::ErrorData::internal_error(error.to_string(), None))?;
        Ok(CreateMessageResult::new(
            SamplingMessage::assistant_text(response.text()),
            model_name,
        ))
    }

    async fn list_roots(
        &self,
        _context: RequestContext<RoleClient>,
    ) -> Result<ListRootsResult, rmcp::ErrorData> {
        Ok(ListRootsResult::new(vec![
            Root::new(self.root_uri.clone()).with_name("workspace"),
        ]))
    }

    async fn create_elicitation(
        &self,
        request: ElicitRequestParams,
        _context: RequestContext<RoleClient>,
    ) -> Result<ElicitResult, rmcp::ErrorData> {
        handle_elicitation(request, self.use_environment_defaults)
    }

    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        let should_log = if self.debug_logging {
            matches!(
                params.level,
                LoggingLevel::Debug
                    | LoggingLevel::Error
                    | LoggingLevel::Critical
                    | LoggingLevel::Alert
                    | LoggingLevel::Emergency
            )
        } else {
            matches!(
                params.level,
                LoggingLevel::Info
                    | LoggingLevel::Error
                    | LoggingLevel::Critical
                    | LoggingLevel::Alert
                    | LoggingLevel::Emergency
            )
        };
        if should_log {
            eprintln!("[{}] - {}", logging_level(params.level), params.data);
        }
    }

    async fn on_tool_list_changed(&self, _context: NotificationContext<RoleClient>) {
        println!("Server tool list changed; it will be refreshed for the next turn.");
    }

    fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) -> impl Future<Output = ()> + Send + '_ {
        async move {
            if let Some(message) = params.message {
                println!("{message}");
            }
            if let Some(total) = params.total.filter(|total| *total > 0.0) {
                println!("{:.0}% complete", params.progress / total * 100.0);
            } else {
                println!("{} complete", params.progress);
            }
        }
    }
}

/// Handle a server elicitation. Environment shortcuts are useful for scripted
/// calculator demos; standalone examples can request the original user flow.
pub fn handle_elicitation(
    request: ElicitRequestParams,
    use_environment_defaults: bool,
) -> Result<ElicitResult, rmcp::ErrorData> {
    match request {
        ElicitRequestParams::FormElicitationParams {
            message,
            requested_schema,
            ..
        } => {
            println!("\nELICITATION REQUEST\n{message}");
            if use_environment_defaults {
                if let (Ok(name), Ok(email)) =
                    (env::var("MCP_SIGNUP_NAME"), env::var("MCP_SIGNUP_EMAIL"))
                {
                    let mut content = serde_json::json!({ "name": name, "email": email });
                    if let Ok(age) = env::var("MCP_SIGNUP_AGE")
                        && let Ok(age) = age.parse::<f64>()
                    {
                        content["age"] = age.into();
                    }
                    return Ok(ElicitResult::new(ElicitationAction::Accept).with_content(content));
                }
            }
            interactive_form_elicitation(&requested_schema)
        }
        ElicitRequestParams::UrlElicitationParams { message, url, .. } => {
            println!("\nELICITATION REQUEST\n{message}\n{url}");
            match confirm("Open this URL? (y/n): ")? {
                Some(true) => {
                    if let Err(error) = open_url(&url) {
                        eprintln!("Could not open the URL: {error}");
                        println!("Open it manually: {url}");
                    }
                    Ok(ElicitResult::new(ElicitationAction::Accept))
                }
                Some(false) => Ok(ElicitResult::new(ElicitationAction::Decline)),
                None => Ok(ElicitResult::new(ElicitationAction::Cancel)),
            }
        }
        _ => Ok(ElicitResult::new(ElicitationAction::Decline)),
    }
}

fn interactive_form_elicitation(
    schema: &rmcp::model::ElicitationSchema,
) -> Result<ElicitResult, rmcp::ErrorData> {
    println!("\n{}", "=".repeat(60));
    println!("FORM DATA REQUIRED");
    println!("{}", "=".repeat(60));
    println!(
        "{}",
        schema
            .description
            .as_deref()
            .unwrap_or("Please provide the requested fields.")
    );

    let schema_value = serde_json::to_value(schema)
        .map_err(|error| rmcp::ErrorData::internal_error(error.to_string(), None))?;
    let properties = schema_value
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();
    let required = schema_value
        .get("required")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .collect::<Vec<_>>();

    println!("Requested fields:");
    for (name, definition) in &properties {
        let field_type = definition
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("string");
        let description = definition
            .get("description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        println!(
            "  {name} ({field_type}){}: {description}",
            if required.contains(&name.as_str()) {
                " (required)"
            } else {
                " (optional)"
            }
        );
    }
    println!("{}", "=".repeat(60));

    match confirm("Accept this request? (y/n/c for cancel): ")? {
        None => return Ok(ElicitResult::new(ElicitationAction::Cancel)),
        Some(false) => return Ok(ElicitResult::new(ElicitationAction::Decline)),
        Some(true) => {}
    }

    let mut content = serde_json::Map::new();
    for (name, definition) in properties {
        let field_type = definition
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("string");
        let description = definition
            .get("description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let is_required = required.contains(&name.as_str());
        let optional_label = if is_required { "" } else { " [optional]" };

        loop {
            let label = if description.is_empty() {
                format!("Enter {name}{optional_label}: ")
            } else {
                format!("Enter {name} ({description}){optional_label}: ")
            };
            let Some(value) = read_input(&label)
                .map_err(|error| rmcp::ErrorData::internal_error(error.to_string(), None))?
            else {
                return Ok(ElicitResult::new(ElicitationAction::Cancel));
            };
            if value.is_empty() && !is_required {
                break;
            }
            if value.is_empty() {
                println!("Error: {name} is required.");
                continue;
            }
            match parse_form_value(&value, field_type, &definition) {
                Ok(value) => {
                    content.insert(name.clone(), value);
                    break;
                }
                Err(error) => println!("Error: {error}"),
            }
        }
    }

    Ok(ElicitResult::new(ElicitationAction::Accept).with_content(content.into()))
}

fn parse_form_value(
    value: &str,
    field_type: &str,
    definition: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let parsed = match field_type {
        "integer" => serde_json::Value::from(
            value
                .parse::<i64>()
                .map_err(|_| format!("Invalid integer value: {value}"))?,
        ),
        "number" => {
            let number = value
                .parse::<f64>()
                .map_err(|_| format!("Invalid number value: {value}"))?;
            serde_json::Number::from_f64(number)
                .map(serde_json::Value::Number)
                .ok_or_else(|| format!("Invalid number value: {value}"))?
        }
        "boolean" => match value.to_lowercase().as_str() {
            "true" | "yes" | "y" | "1" => serde_json::Value::Bool(true),
            "false" | "no" | "n" | "0" => serde_json::Value::Bool(false),
            _ => return Err(format!("Invalid boolean value: {value}")),
        },
        _ => serde_json::Value::String(value.to_owned()),
    };

    if let Some(number) = parsed.as_f64() {
        for bound in ["minimum", "maximum"] {
            let Some(limit) = definition.get(bound).and_then(serde_json::Value::as_f64) else {
                continue;
            };
            if (bound == "minimum" && number < limit) || (bound == "maximum" && number > limit) {
                return Err(format!("{field_type} value must satisfy {bound} {limit}"));
            }
        }
    }
    if let Some(choices) = definition.get("enum").and_then(serde_json::Value::as_array)
        && !choices.contains(&parsed)
    {
        return Err(format!("{value:?} is not one of the allowed values"));
    }
    Ok(parsed)
}

fn confirm(prompt: &str) -> Result<Option<bool>, rmcp::ErrorData> {
    loop {
        let Some(response) = read_input(prompt)
            .map_err(|error| rmcp::ErrorData::internal_error(error.to_string(), None))?
        else {
            return Ok(None);
        };
        match response.to_lowercase().as_str() {
            "y" | "yes" | "accept" | "open" => return Ok(Some(true)),
            "n" | "no" | "decline" => return Ok(Some(false)),
            "c" | "cancel" => return Ok(None),
            _ => println!("Please enter y, n, or c."),
        }
    }
}

fn read_input(prompt: &str) -> io::Result<Option<String>> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut value = String::new();
    if io::stdin().read_line(&mut value)? == 0 {
        return Ok(None);
    }
    Ok(Some(value.trim().to_owned()))
}

fn open_url(url: &str) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", ""]);
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = Command::new("xdg-open");
    command.arg(url).spawn().map(|_| ())
}

fn configured_model(variable: &str, fallback: &str) -> String {
    env::var(variable)
        .ok()
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

fn logging_level(level: LoggingLevel) -> &'static str {
    match level {
        LoggingLevel::Debug => "debug",
        LoggingLevel::Info => "info",
        LoggingLevel::Notice => "notice",
        LoggingLevel::Warning => "warning",
        LoggingLevel::Error => "error",
        LoggingLevel::Critical => "critical",
        LoggingLevel::Alert => "alert",
        LoggingLevel::Emergency => "emergency",
    }
}
