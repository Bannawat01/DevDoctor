use anyhow::Result;
use devdoctor_core::{diagnose, diagnose_environment, diagnose_ports, explain};
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
        report_with(&args.path, diagnose_environment)
    }
    #[tool(description = "Inspect only configured local project ports.")]
    fn check_ports(&self, Parameters(args): Parameters<PathArgs>) -> String {
        report_with(&args.path, diagnose_ports)
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
    report_with(path, diagnose)
}

fn report_with(path: &str, diagnostics: fn(&std::path::Path) -> devdoctor_core::Report) -> String {
    let path = PathBuf::from(path);
    if !path.is_dir() {
        return "path must be an existing directory".into();
    }
    serde_json::to_string(&diagnostics(&path))
        .unwrap_or_else(|_| "diagnostic serialization failed".into())
}

#[tokio::main]
async fn main() -> Result<()> {
    let service = DevDoctor.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn environment_report_contains_only_environment_findings() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname='fixture'").unwrap();
        fs::write(dir.path().join(".env.example"), "TOKEN=\n").unwrap();
        let report: devdoctor_core::Report = serde_json::from_str(&report_with(
            dir.path().to_str().unwrap(),
            diagnose_environment,
        ))
        .unwrap();
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].code, "ENV-MISSING-001");
    }

    #[test]
    fn domain_reports_are_empty_for_an_unconfigured_fixture() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        for report in [
            report_with(path, diagnose_environment),
            report_with(path, diagnose_ports),
        ] {
            let report: devdoctor_core::Report = serde_json::from_str(&report).unwrap();
            assert!(report.findings.is_empty());
        }
    }

    #[test]
    fn invalid_path_and_schema_are_deterministic() {
        assert_eq!(
            report_with("missing", diagnose_ports),
            "path must be an existing directory"
        );
        let dir = tempdir().unwrap();
        let report: serde_json::Value = serde_json::from_str(&report_with(
            dir.path().to_str().unwrap(),
            diagnose_environment,
        ))
        .unwrap();
        assert!(report["project"].is_object());
        assert!(report["findings"].is_array());
    }
}
