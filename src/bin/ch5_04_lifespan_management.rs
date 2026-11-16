//! Chapter 5.4: keep application state alive for the server's whole lifetime.

use std::sync::Arc;

use anyhow::Result;
use chrono::Local;
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, ListToolsResult,
        ServerCapabilities, ServerConfig, Tool,
    },
    service::{RequestContext, RoleServer},
    transport::stdio,
};
use serde_json::{Value, json};
use tokio::sync::Mutex;

#[derive(Clone)]
struct LifespanServer {
    logs: Arc<Mutex<Vec<String>>>,
}

impl LifespanServer {
    async fn log(&self, message: impl Into<String>) {
        let entry = format!("{}: {}", timestamp(), message.into());
        self.logs.lock().await.push(entry.clone());
        eprintln!("{entry}");
    }

    async fn append_silently(&self, message: impl Into<String>) {
        let entry = format!("{}: {}", timestamp(), message.into());
        self.logs.lock().await.push(entry);
    }

    async fn print_last_log(&self) {
        if let Some(entry) = self.logs.lock().await.last() {
            eprintln!("{entry}");
        }
    }
}

fn timestamp() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S%.6f").to_string()
}

impl ServerHandler for LifespanServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("low-level-server", "0.1.0"))
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        self.print_last_log().await;
        self.log("list_tools called").await;

        let input_schema = json!({
            "type": "object",
            "properties": {
                "a": { "type": "number", "description": "The first number to add" },
                "b": { "type": "number", "description": "The second number to add" }
            },
            "required": ["a", "b"]
        })
        .as_object()
        .cloned()
        .expect("tool schema is a JSON object");
        let output_schema = json!({
            "type": "object",
            "properties": {
                "augend": { "type": "number" },
                "addend": { "type": "number" },
                "sum": { "type": "number" }
            },
            "required": ["augend", "addend", "sum"]
        })
        .as_object()
        .cloned()
        .expect("output schema is a JSON object");

        Ok(ListToolsResult::with_all_items(vec![
            Tool::new("add", "Add two numbers together.", input_schema)
                .with_raw_output_schema(Arc::new(output_schema)),
        ]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        if request.name != "add" {
            return Err(McpError::invalid_params(
                format!("Unknown tool: {}", request.name),
                None,
            ));
        }
        let arguments = request.arguments.unwrap_or_default();
        let a = arguments
            .get("a")
            .and_then(Value::as_f64)
            .ok_or_else(|| McpError::invalid_params("missing numeric argument: a", None))?;
        let b = arguments
            .get("b")
            .and_then(Value::as_f64)
            .ok_or_else(|| McpError::invalid_params("missing numeric argument: b", None))?;

        self.log("add called").await;
        let result = json!({ "augend": a, "addend": b, "sum": a + b });
        Ok(CallToolResult::structured(result).into())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    eprintln!("Running the lifespan management example MCP server");
    let logs = Arc::new(Mutex::new(Vec::new()));
    let server = LifespanServer {
        logs: Arc::clone(&logs),
    };
    server.log("Server started").await;
    server.append_silently("logs yielded").await;

    let service = server.serve(stdio()).await?;
    service.waiting().await?;

    let entry = format!("{}: Server stopped, printing all logs", timestamp());
    let mut logs = logs.lock().await;
    logs.push(entry);
    eprintln!("Server lifetime logs: {logs:#?}");
    Ok(())
}
