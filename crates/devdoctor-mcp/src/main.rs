use anyhow::Result;
use devdoctor_core::{diagnose, diagnose_environment, diagnose_ports, explain};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
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
    fn diagnose_project(&self, Parameters(args): Parameters<PathArgs>) -> CallToolResult {
        report(&args.path)
    }
    #[tool(description = "Inspect Rust and Node toolchain diagnostics for a local project.")]
    fn check_toolchain(&self, Parameters(args): Parameters<PathArgs>) -> CallToolResult {
        report(&args.path)
    }
    #[tool(description = "Inspect declared environment configuration without revealing values.")]
    fn check_environment(&self, Parameters(args): Parameters<PathArgs>) -> CallToolResult {
        report_with(&args.path, diagnose_environment)
    }
    #[tool(description = "Inspect only configured local project ports.")]
    fn check_ports(&self, Parameters(args): Parameters<PathArgs>) -> CallToolResult {
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
fn report(path: &str) -> CallToolResult {
    report_with(path, diagnose)
}

fn report_with(
    path: &str,
    diagnostics: fn(&std::path::Path) -> devdoctor_core::Report,
) -> CallToolResult {
    let path = PathBuf::from(path);
    if !path.is_dir() {
        return CallToolResult::error(vec![ContentBlock::text(
            "path must be an existing directory",
        )]);
    }
    match serde_json::to_string(&diagnostics(&path)) {
        Ok(report) => CallToolResult::success(vec![ContentBlock::text(report)]),
        Err(_) => {
            CallToolResult::error(vec![ContentBlock::text("diagnostic serialization failed")])
        }
    }
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
    use rmcp::model::CallToolRequestParams;
    use std::fs;
    use tempfile::tempdir;

    fn arguments(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        value.as_object().unwrap().clone()
    }

    fn text(result: &CallToolResult) -> &str {
        result
            .content
            .first()
            .and_then(ContentBlock::as_text)
            .map(|content| content.text.as_str())
            .unwrap()
    }

    fn report_json(result: CallToolResult) -> devdoctor_core::Report {
        serde_json::from_str(text(&result)).unwrap()
    }

    #[test]
    fn environment_report_contains_only_environment_findings() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname='fixture'").unwrap();
        fs::write(dir.path().join(".env.example"), "TOKEN=\n").unwrap();
        let report = report_json(report_with(
            dir.path().to_str().unwrap(),
            diagnose_environment,
        ));
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].code, "ENV-MISSING-001");
    }

    #[test]
    fn domain_reports_are_empty_for_an_unconfigured_fixture() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        for result in [
            report_with(path, diagnose_environment),
            report_with(path, diagnose_ports),
        ] {
            let report = report_json(result);
            assert!(report.findings.is_empty());
        }
    }

    #[test]
    fn invalid_path_and_schema_are_deterministic() {
        let invalid = report_with("missing", diagnose_ports);
        assert_eq!(invalid.is_error, Some(true));
        let dir = tempdir().unwrap();
        let report = serde_json::to_value(report_json(report_with(
            dir.path().to_str().unwrap(),
            diagnose_environment,
        )))
        .unwrap();
        assert!(report["project"].is_object());
        assert!(report["findings"].is_array());
    }

    #[tokio::test]
    async fn domain_tools_dispatch_through_rmcp_protocol() -> anyhow::Result<()> {
        let (server_transport, client_transport) = tokio::io::duplex(64 * 1024);
        let server = tokio::spawn(async move {
            DevDoctor.serve(server_transport).await?.waiting().await?;
            anyhow::Ok(())
        });
        let client = ().serve(client_transport).await?;

        let tools = client.list_all_tools().await?;
        for name in [
            "diagnose_project",
            "check_toolchain",
            "check_environment",
            "check_ports",
            "explain_finding",
        ] {
            assert!(tools.iter().any(|tool| tool.name == name));
        }

        let environment = tempdir()?;
        fs::write(
            environment.path().join("Cargo.toml"),
            "[package]\nname='fixture'",
        )?;
        fs::write(environment.path().join(".env.example"), "TOKEN=\n")?;
        let environment_path = environment.path().display().to_string();
        let result = client
            .call_tool(
                CallToolRequestParams::new("check_environment")
                    .with_arguments(arguments(serde_json::json!({"path": environment_path}))),
            )
            .await?;
        assert_eq!(result.is_error, Some(false));
        let environment_report = report_json(result);
        assert!(
            environment_report
                .findings
                .iter()
                .any(|finding| finding.code == "ENV-MISSING-001")
        );
        assert!(
            !environment_report
                .findings
                .iter()
                .any(|finding| finding.code == "RUST-TOOLCHAIN-OK")
        );

        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let ports = tempdir()?;
        fs::write(
            ports.path().join("compose.yml"),
            format!("services:\n  app:\n    ports:\n      - \"{port}:80\"\n"),
        )?;
        let result = client
            .call_tool(
                CallToolRequestParams::new("check_ports").with_arguments(arguments(
                    serde_json::json!({"path": ports.path().display().to_string()}),
                )),
            )
            .await?;
        assert_eq!(result.is_error, Some(false));
        let ports_report = report_json(result);
        assert!(
            ports_report
                .findings
                .iter()
                .any(|finding| finding.code == "PORT-CONFLICT-001")
        );
        assert!(
            !ports_report
                .findings
                .iter()
                .any(|finding| finding.code == "ENV-MISSING-001")
        );
        assert!(
            ports_report
                .findings
                .iter()
                .flat_map(|finding| &finding.evidence)
                .any(|evidence| evidence.starts_with("owner_pid=")),
            "expected port ownership evidence: {:?}",
            ports_report.findings
        );
        drop(listener);

        let clean = tempdir()?;
        for tool in ["check_environment", "check_ports"] {
            let result = client
                .call_tool(CallToolRequestParams::new(tool).with_arguments(arguments(
                    serde_json::json!({"path": clean.path().display().to_string()}),
                )))
                .await?;
            assert_eq!(result.is_error, Some(false));
            assert!(report_json(result).findings.is_empty());
        }

        for tool in ["check_environment", "check_ports"] {
            let result = client
                .call_tool(CallToolRequestParams::new(tool).with_arguments(arguments(
                    serde_json::json!({"path": "missing-protocol-fixture"}),
                )))
                .await?;
            assert_eq!(result.is_error, Some(true));
            assert_eq!(text(&result), "path must be an existing directory");
        }
        let result = client
            .call_tool(
                CallToolRequestParams::new("check_environment")
                    .with_arguments(arguments(serde_json::json!({"path": 7}))),
            )
            .await?;
        assert_eq!(result.is_error, Some(true));

        let result = client
            .call_tool(
                CallToolRequestParams::new("diagnose_project").with_arguments(arguments(
                    serde_json::json!({"path": environment.path().display().to_string()}),
                )),
            )
            .await?;
        assert_eq!(result.is_error, Some(false));
        assert!(
            report_json(result)
                .findings
                .iter()
                .any(|finding| finding.code == "ENV-MISSING-001")
        );

        let result = client
            .call_tool(
                CallToolRequestParams::new("check_toolchain").with_arguments(arguments(
                    serde_json::json!({"path": environment.path().display().to_string()}),
                )),
            )
            .await?;
        assert_eq!(result.is_error, Some(false));
        assert!(
            report_json(result)
                .findings
                .iter()
                .any(|finding| finding.code.starts_with("RUST-TOOLCHAIN-"))
        );

        let result = client
            .call_tool(
                CallToolRequestParams::new("explain_finding")
                    .with_arguments(arguments(serde_json::json!({"code": "RUST-TOOLCHAIN-OK"}))),
            )
            .await?;
        assert_eq!(result.is_error, Some(false));
        assert!(text(&result).contains("Rust compiler available"));

        client.cancel().await?;
        server.await??;
        Ok(())
    }
}
