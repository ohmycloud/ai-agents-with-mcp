//! Chapter 5.7: combine typed inputs, outputs, and related tools.

use anyhow::Result;
use rand::Rng;
use rmcp::{
    ServiceExt,
    handler::server::wrapper::{Json, Parameters},
    schemars::JsonSchema,
    tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
struct GraderServer;

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
struct Course {
    title: String,
    grade: i64,
    instructor: String,
    credits: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ReportCardRequest {
    name: String,
    courses: Option<Vec<Course>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GpaRequest {
    courses: Vec<Course>,
    #[serde(default = "default_weighted")]
    weighted: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ReportCard {
    name: String,
    grades: Vec<Course>,
    weighted_gpa: f64,
    unweighted_gpa: f64,
}

#[tool_router]
impl GraderServer {
    #[tool(
        title = "Calculate GPA",
        description = "Calculate weighted or unweighted GPA from a course list.",
        annotations(read_only_hint = true)
    )]
    fn grader_calculate_gpa(&self, Parameters(request): Parameters<GpaRequest>) -> String {
        calculate_gpa(&request.courses, request.weighted).to_string()
    }

    #[tool(
        title = "Generate Report Card",
        description = "Generate a report card with weighted and unweighted GPA."
    )]
    fn grader_generate_report_card(
        &self,
        Parameters(request): Parameters<ReportCardRequest>,
    ) -> Json<ReportCard> {
        let grades = request
            .courses
            .filter(|courses| !courses.is_empty())
            .unwrap_or_else(generate_courses);
        Json(ReportCard {
            name: request.name,
            weighted_gpa: calculate_gpa(&grades, true),
            unweighted_gpa: calculate_gpa(&grades, false),
            grades,
        })
    }
}

#[tool_handler(name = "full-tool-server", version = "0.1.0")]
impl rmcp::ServerHandler for GraderServer {}

fn default_weighted() -> bool {
    true
}

fn generate_courses() -> Vec<Course> {
    let mut rng = rand::thread_rng();
    vec![
        Course {
            title: "Math".into(),
            grade: rng.gen_range(0..=100),
            instructor: "Mr. Smith".into(),
            credits: rng.gen_range(1..=4),
        },
        Course {
            title: "Science".into(),
            grade: rng.gen_range(0..=100),
            instructor: "Mrs. Johnson".into(),
            credits: rng.gen_range(1..=4),
        },
        Course {
            title: "History".into(),
            grade: rng.gen_range(0..=100),
            instructor: "Mr. Brown".into(),
            credits: rng.gen_range(1..=4),
        },
    ]
}

fn calculate_gpa(courses: &[Course], weighted: bool) -> f64 {
    if courses.is_empty() {
        return 0.0;
    }
    if weighted {
        let credits = courses.iter().map(|course| course.credits).sum::<i64>();
        if credits <= 0 {
            return 0.0;
        }
        courses
            .iter()
            .map(|course| course.grade * course.credits)
            .sum::<i64>() as f64
            / credits as f64
    } else {
        courses.iter().map(|course| course.grade).sum::<i64>() as f64 / courses.len() as f64
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let service = GraderServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
