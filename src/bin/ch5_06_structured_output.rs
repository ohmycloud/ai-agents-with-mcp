//! Chapter 5.6: return a typed report card and an image content block.

use anyhow::Result;
use base64::{Engine, engine::general_purpose::STANDARD};
use rmcp::{
    ErrorData as McpError, ServiceExt,
    handler::server::wrapper::{Json, Parameters},
    model::ContentBlock,
    schemars::JsonSchema,
    tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
struct ReportCardServer;

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
struct ReportCard {
    name: String,
    grades: Vec<(String, i64)>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GenerateReportCardRequest {
    name: String,
    grades: Vec<(String, i64)>,
}

#[tool_router]
impl ReportCardServer {
    #[tool(description = "Generate a report card for a student.")]
    fn generate_report_card(
        &self,
        Parameters(request): Parameters<GenerateReportCardRequest>,
    ) -> Json<ReportCard> {
        Json(ReportCard {
            name: request.name,
            grades: request.grades,
        })
    }

    #[tool(description = "Render a report card as an image.")]
    fn generate_report_card_image(
        &self,
        Parameters(report_card): Parameters<ReportCard>,
    ) -> Result<ContentBlock, McpError> {
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"400\" height=\"200\" viewBox=\"0 0 400 200\"><rect width=\"400\" height=\"200\" fill=\"white\"/><text x=\"100\" y=\"110\" font-family=\"sans-serif\" font-size=\"10\" fill=\"black\">{}</text></svg>",
            escape_xml(&report_card.name)
        );
        let options = resvg::usvg::Options::default();
        let tree = resvg::usvg::Tree::from_str(&svg, &options)
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        let size = tree.size().to_int_size();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
            .ok_or_else(|| McpError::internal_error("invalid report card image size", None))?;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::default(),
            &mut pixmap.as_mut(),
        );
        let png = pixmap
            .encode_png()
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        Ok(ContentBlock::image(STANDARD.encode(png), "image/png"))
    }
}

#[tool_handler(name = "structured-output-server", version = "0.1.0")]
impl rmcp::ServerHandler for ReportCardServer {}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[tokio::main]
async fn main() -> Result<()> {
    let service = ReportCardServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
