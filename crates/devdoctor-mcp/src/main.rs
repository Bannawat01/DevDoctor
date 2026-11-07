use anyhow::Result;
use devdoctor_core::{diagnose, explain};
use rmcp::{
    ServerHandler, ServiceExt, handler::server::wrapper::Parameters, schemars, tool, tool_handler,
    tool_router, transport::stdio,
};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct PathArgs {
    /// Existing local directory to inspect read-only.
    path: String,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ExplainArgs {
    /// Stable DevDoctor diagnostic code.
    code: String,
}
#[derive(Clone)]
struct DevDoctor;

#[tool_router]
impl DevDoctor {
    #[tool(description = "Run all read-only diagnostics for a local project.")]
    fn diagnose_project(&self, Parameters(args): Parameters<PathArgs>) -> String {
        report(&args.path)
    }
    #[tool(description = "Inspect Rust and Node toolchain diagnostics for a local project.")]
    fn check_toolchain(&self, Parameters(args): Parameters<PathArgs>) -> String {
        report(&args.path)
    }
    #[tool(description = "Inspect declared environment configuration without revealing values.")]
    fn check_environment(&self, Parameters(args): Parameters<PathArgs>) -> String {
        report(&args.path)
    }
    #[tool(description = "Inspect only configured local project ports.")]
    fn check_ports(&self, Parameters(args): Parameters<PathArgs>) -> String {
        report(&args.path)
    }
    #[tool(description = "Explain a stable DevDoctor finding code.")]
    fn explain_finding(&self, Parameters(args): Parameters<ExplainArgs>) -> String {
        explain(&args.code).map_or_else(
            || "Unknown finding code".to_string(),
            |(title, action)| format!("{}: {title}\n{action}", args.code),
        )
    }
}
#[tool_handler(
    name = "devdoctor",
    version = "0.1.0",
    instructions = "Read-only local developer environment diagnostics. Never modifies project files or environment state."
)]
impl ServerHandler for DevDoctor {}
fn report(path: &str) -> String {
    let path = PathBuf::from(path);
    if !path.is_dir() {
        return "path must be an existing directory".into();
    }
    serde_json::to_string(&diagnose(&path))
        .unwrap_or_else(|_| "diagnostic serialization failed".into())
}
#[tokio::main]
async fn main() -> Result<()> {
    let service = DevDoctor.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
