//! Six-operation calculator shared by Chapter 7 deployment examples.

use rmcp::{
    ServerHandler, handler::server::wrapper::Parameters, schemars::JsonSchema, tool, tool_handler,
    tool_router,
};
use serde::Deserialize;

#[derive(Clone)]
pub struct DeploymentCalculator;

#[derive(Debug, Deserialize, JsonSchema)]
struct BinaryOperation {
    a: f64,
    b: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PowerOperation {
    base: f64,
    exponent: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SquareRootOperation {
    number: f64,
}

#[tool_router]
impl DeploymentCalculator {
    #[tool(description = "Add two numbers together.")]
    fn add(&self, Parameters(input): Parameters<BinaryOperation>) -> String {
        format!("{} + {} = {}", input.a, input.b, input.a + input.b)
    }

    #[tool(description = "Subtract the second number from the first.")]
    fn subtract(&self, Parameters(input): Parameters<BinaryOperation>) -> String {
        format!("{} - {} = {}", input.a, input.b, input.a - input.b)
    }

    #[tool(description = "Multiply two numbers together.")]
    fn multiply(&self, Parameters(input): Parameters<BinaryOperation>) -> String {
        format!("{} × {} = {}", input.a, input.b, input.a * input.b)
    }

    #[tool(description = "Divide the first number by the second.")]
    fn divide(&self, Parameters(input): Parameters<BinaryOperation>) -> String {
        if input.b == 0.0 {
            return "Error: Division by zero is not allowed".to_owned();
        }
        format!("{} ÷ {} = {}", input.a, input.b, input.a / input.b)
    }

    #[tool(description = "Raise a number to a power.")]
    fn power(&self, Parameters(input): Parameters<PowerOperation>) -> String {
        let result = input.base.powf(input.exponent);
        if result.is_nan() {
            return "Error calculating power: result is not a real number".to_owned();
        }
        format!("{}^{} = {result}", input.base, input.exponent)
    }

    #[tool(description = "Calculate the square root of a number.")]
    fn square_root(&self, Parameters(input): Parameters<SquareRootOperation>) -> String {
        if input.number < 0.0 {
            return "Error: Cannot calculate square root of negative number".to_owned();
        }
        format!("√{} = {}", input.number, input.number.sqrt())
    }
}

#[tool_handler(name = "calculator", version = "0.1.0")]
impl ServerHandler for DeploymentCalculator {}
