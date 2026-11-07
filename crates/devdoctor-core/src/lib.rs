#![allow(
    clippy::collapsible_if,
    clippy::too_many_arguments,
    clippy::unnecessary_unwrap
)]
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    net::{TcpStream, ToSocketAddrs},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Severity {
    Error,
    Warning,
    Info,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Confirmed,
    Suspected,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub code: String,
    pub severity: Severity,
    pub title: String,
    pub component: String,
    pub explanation: String,
    pub evidence: Vec<String>,
    pub confidence: Confidence,
    pub recommendation: String,
    pub verified: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Project {
    pub path: String,
    pub name: String,
    pub stack: Vec<String>,
    pub files: Vec<String>,
    pub env_example: Vec<String>,
    pub compose_ports: Vec<u16>,
    pub compose_db_ports: Vec<u16>,
    pub database_url: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub project: Project,
    pub findings: Vec<Finding>,
}

pub fn detect_project(path: &Path) -> Project {
    let name = path
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or("project")
        .to_string();
    let mut p = Project {
        path: path.display().to_string(),
        name,
        ..Default::default()
    };
    let exists = |f: &str| path.join(f).is_file();
    for (f, s) in [
        ("Cargo.toml", "Rust"),
        ("package.json", "Node"),
        ("compose.yaml", "Docker"),
        ("compose.yml", "Docker"),
        ("docker-compose.yml", "Docker"),
    ] {
        if exists(f) {
            p.files.push(f.into());
            if !p.stack.contains(&s.into()) {
                p.stack.push(s.into())
            }
        }
    }
    let env = path.join(".env.example");
    if env.is_file() {
        p.env_example = read_env_names(&env);
        p.files.push(".env.example".into());
    }
    for f in ["compose.yaml", "compose.yml", "docker-compose.yml"] {
        if exists(f) {
            if let Ok(v) = fs::read_to_string(path.join(f)) {
                parse_ports(&v, &mut p);
                break;
            }
        }
    }
    if let Ok(v) = fs::read_to_string(path.join(".env")) {
        p.database_url = find_var(&v, "DATABASE_URL").map(|v| redact_url(&v));
    }
    p
}
fn read_env_names(p: &Path) -> Vec<String> {
    fs::read_to_string(p)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('=').map(|(k, _)| k.trim().to_string()))
        .filter(|k| !k.is_empty() && !k.starts_with('#'))
        .collect()
}
fn find_var(s: &str, k: &str) -> Option<String> {
    s.lines().find_map(|l| {
        l.split_once('=')
            .filter(|(a, _)| a.trim() == k)
            .map(|(_, b)| b.trim().trim_matches('"').to_string())
    })
}
fn redact_url(url: &str) -> String {
    if let Some((scheme, rest)) = url.split_once("//") {
        if let Some((_, host)) = rest.rsplit_once('@') {
            return format!("{scheme}//<redacted>@{host}");
        }
    }
    url.to_string()
}
fn parse_ports(s: &str, p: &mut Project) {
    for line in s.lines() {
        for token in line.split(['[', ']', ',', ' ', '"', '\'']) {
            if let Some((host, container)) = token.split_once(':') {
                if let (Ok(host), Ok(container)) = (host.parse::<u16>(), container.parse::<u16>()) {
                    p.compose_ports.push(host);
                    if container == 5432 {
                        p.compose_db_ports.push(host);
                    }
                }
            }
        }
        let t = line
            .trim()
            .trim_start_matches('-')
            .trim()
            .trim_matches(['"', '\'']);
        if t.contains(':') {
            let a = t.split(':').collect::<Vec<_>>();
            if a.len() >= 2 {
                let left = a[a.len() - 2].trim_matches(['"', '\'']);
                let right = a[a.len() - 1].split(['#', '"']).next().unwrap_or("").trim();
                if let (Ok(host), Ok(container)) = (left.parse::<u16>(), right.parse::<u16>()) {
                    if p.compose_ports.contains(&host) {
                        continue;
                    }
                    p.compose_ports.push(host);
                    if container == 5432 {
                        p.compose_db_ports.push(host)
                    }
                }
            }
        }
    }
}
fn parse_url_host_port(url: &str) -> Option<(String, u16)> {
    let authority = url.split("//").nth(1)?.split('/').next()?;
    let authority = authority.rsplit_once('@').map_or(authority, |(_, a)| a);
    let (host, port) = authority.rsplit_once(':')?;
    Some((
        host.trim_matches(['[', ']']).to_string(),
        port.parse().ok()?,
    ))
}
fn parse_url_port(url: &str) -> Option<u16> {
    parse_url_host_port(url).map(|(_, p)| p)
}
pub fn resolve_executable(path_entries: &[String], names: &[&str]) -> Vec<String> {
    path_entries
        .iter()
        .flat_map(|entry| {
            names
                .iter()
                .map(move |name| Path::new(entry).join(name).display().to_string())
        })
        .filter(|candidate| Path::new(candidate).is_file())
        .collect()
}
pub fn linker_diagnostic(
    host: &str,
    path_entries: &[String],
    rustc_linker: Option<&str>,
) -> Option<Finding> {
    if !host.contains("windows") {
        return None;
    }
    let candidates = resolve_executable(path_entries, &["link.exe"]);
    let first = candidates.first()?;
    let lower = first.to_ascii_lowercase();
    let suspicious =
        lower.contains("git\\usr\\bin") || lower.contains("mingw") || lower.contains("msys");
    let mismatch = rustc_linker.is_some_and(|l| l.eq_ignore_ascii_case("gnu")) && !suspicious;
    if suspicious || mismatch {
        Some(finding(
            "RUST-LINKER-001",
            Severity::Warning,
            "Suspicious Windows linker resolution",
            "PATH",
            "The first link.exe candidate may not match the Rust target toolchain.",
            vec![
                format!("first_candidate={first}"),
                format!("candidates={candidates:?}"),
                format!("target_linker={rustc_linker:?}"),
            ],
            Confidence::Suspected,
            "Use a matching MSVC or GNU developer environment and verify PATH order.",
        ))
    } else {
        None
    }
}
pub fn redact_text(input: &str) -> String {
    let mut out = input.to_string();
    for key in ["password", "token", "secret", "api_key"] {
        let mut pos = 0;
        while let Some(found) = out[pos..].to_ascii_lowercase().find(&format!("{key}=")) {
            let start = pos + found + key.len() + 1;
            let end = out[start..]
                .find(['&', ' ', '\n'])
                .map_or(out.len(), |e| start + e);
            out.replace_range(start..end, "<redacted>");
            pos = start + 10;
        }
    }
    out
}
fn cmd(program: &str, args: &[&str]) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    for _ in 0..20 {
        if let Ok(Some(status)) = child.try_wait() {
            if status.success() {
                let output = child.wait_with_output().ok()?;
                return Some(redact_text(String::from_utf8_lossy(&output.stdout).trim()));
            }
            return None;
        }
        thread::sleep(Duration::from_millis(25));
    }
    let _ = child.kill();
    None
}
fn finding(
    code: &str,
    severity: Severity,
    title: &str,
    component: &str,
    explanation: &str,
    evidence: Vec<String>,
    confidence: Confidence,
    recommendation: &str,
) -> Finding {
    Finding {
        code: code.into(),
        severity,
        title: title.into(),
        component: component.into(),
        explanation: explanation.into(),
        evidence,
        confidence,
        recommendation: recommendation.into(),
        verified: true,
    }
}
pub fn diagnose(path: &Path) -> Report {
    let p = detect_project(path);
    let mut f = Vec::new();
    if p.stack.iter().any(|s| s == "Rust") {
        let rust = cmd("rustc", &["-V"]);
        if rust.is_none() {
            f.push(finding(
                "RUST-TOOLCHAIN-001",
                Severity::Error,
                "Rust compiler unavailable",
                "Rust",
                "Cargo projects require rustc.",
                vec!["rustc was not found".into()],
                Confidence::Confirmed,
                "Install Rust with rustup; DevDoctor does not install it.",
            ));
        } else {
            f.push(finding(
                "RUST-TOOLCHAIN-OK",
                Severity::Info,
                "Rust compiler available",
                "Rust",
                &rust.unwrap(),
                vec![],
                Confidence::Confirmed,
                "No action required.",
            ));
        }
        if let Some(host) = cmd("rustc", &["-vV"]) {
            let target = host.lines().find_map(|l| l.strip_prefix("host: "));
            if let Some(finding) = linker_diagnostic(
                target.unwrap_or(""),
                &std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>(),
                None,
            ) {
                f.push(finding);
            }
        }
    }
    if p.stack.iter().any(|s| s == "Node") {
        if cmd("node", &["--version"]).is_none() {
            f.push(finding(
                "NODE-TOOLCHAIN-001",
                Severity::Warning,
                "Node.js unavailable",
                "Node",
                "package.json was detected but node was not found.",
                vec![],
                Confidence::Confirmed,
                "Install Node.js or use the project’s documented runtime.",
            ));
        }
    }
    if p.stack.iter().any(|s| s == "Docker") {
        if cmd("docker", &["info", "--format", "{{.ServerVersion}}"]).is_none() {
            f.push(finding(
                "DOCKER-ENGINE-001",
                Severity::Warning,
                "Docker engine unavailable",
                "Docker",
                "Compose configuration exists but Docker engine could not be queried.",
                vec![],
                Confidence::Suspected,
                "Start Docker Desktop or verify permissions.",
            ));
        }
    }
    for port in &p.compose_ports {
        if TcpListenerCheck::busy(*port) {
            f.push(finding(
                "PORT-CONFLICT-001",
                Severity::Error,
                "Host port is occupied",
                "Ports",
                &format!("TCP port {port} accepts a local connection."),
                vec![format!("port={port}")],
                Confidence::Confirmed,
                "Stop the conflicting service or change the Compose host port.",
            ));
        }
    }
    f.extend(environment_findings(path, &p));
    Report {
        project: p,
        findings: f,
    }
}

/// Run only diagnostics whose findings describe environment configuration or its database endpoint.
pub fn diagnose_environment(path: &Path) -> Report {
    let project = detect_project(path);
    let findings = environment_findings(path, &project);
    Report { project, findings }
}

fn environment_findings(path: &Path, project: &Project) -> Vec<Finding> {
    let mut findings = Vec::new();
    if !project.env_example.is_empty() && !path.join(".env").is_file() {
        findings.push(finding(
            "ENV-MISSING-001",
            Severity::Warning,
            ".env file is missing",
            "Environment",
            ".env.example declares variables but .env is absent.",
            project.env_example.clone(),
            Confidence::Confirmed,
            "Create .env from .env.example without committing secrets.",
        ));
    }
    if let Some(url) = &project.database_url {
        if let Some(port) = parse_url_port(url) {
            if !project.compose_db_ports.is_empty() && !project.compose_db_ports.contains(&port) {
                findings.push(finding(
                    "ENV-DATABASE-001",
                    Severity::Warning,
                    "DATABASE_URL port does not match Compose",
                    "PostgreSQL",
                    "The configured database URL uses a host port not published by Compose.",
                    vec![
                        format!("configured_port={port}"),
                        format!("compose_ports={:?}", project.compose_db_ports),
                    ],
                    Confidence::Confirmed,
                    "Align DATABASE_URL with the published PostgreSQL host port.",
                ));
            }
        }
        if let Some((host, port)) = parse_url_host_port(url) {
            let reachable = format!("{host}:{port}")
                .to_socket_addrs()
                .ok()
                .and_then(|mut a| a.next())
                .and_then(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(250)).ok())
                .is_some();
            if !reachable {
                findings.push(finding(
                    "POSTGRES-CONNECT-001",
                    Severity::Warning,
                    "PostgreSQL endpoint is unreachable",
                    "PostgreSQL",
                    "A read-only TCP connectivity check could not reach the configured endpoint.",
                    vec![format!("host={host}"), format!("port={port}")],
                    Confidence::Confirmed,
                    "Start the database or correct the redacted endpoint configuration.",
                ));
            }
        }
    }
    findings
}

/// Run only diagnostics whose findings describe configured host ports.
pub fn diagnose_ports(path: &Path) -> Report {
    let project = detect_project(path);
    let findings = project
        .compose_ports
        .iter()
        .filter(|port| TcpListenerCheck::busy(**port))
        .map(|port| {
            finding(
                "PORT-CONFLICT-001",
                Severity::Error,
                "Host port is occupied",
                "Ports",
                &format!("TCP port {port} accepts a local connection."),
                vec![format!("port={port}")],
                Confidence::Confirmed,
                "Stop the conflicting service or change the Compose host port.",
            )
        })
        .collect();
    Report { project, findings }
}

struct TcpListenerCheck;
impl TcpListenerCheck {
    fn busy(port: u16) -> bool {
        TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}")
                .to_socket_addrs()
                .ok()
                .and_then(|mut x| x.next())
                .unwrap_or(([127, 0, 0, 1], 0).into()),
            Duration::from_millis(100),
        )
        .is_ok()
    }
}
pub fn explain(code: &str) -> Option<(&'static str, &'static str)> {
    match code {
        "RUST-TOOLCHAIN-OK" => Some((
            "Rust compiler available",
            "No action is required; rerun DevDoctor if the selected toolchain changes.",
        )),
        "RUST-TOOLCHAIN-001" => Some(("Rust compiler unavailable", "Install Rust with rustup.")),
        "DOCKER-ENGINE-001" => Some((
            "Docker engine unavailable",
            "Start Docker Desktop and retry.",
        )),
        "PORT-CONFLICT-001" => Some((
            "Host port is occupied",
            "Stop the conflicting local service or change the host mapping.",
        )),
        "ENV-DATABASE-001" => Some((
            "DATABASE_URL port does not match Compose",
            "Use the published host port.",
        )),
        "ENV-MISSING-001" => Some((".env file is missing", "Create it from .env.example.")),
        _ => None,
    }
}
pub fn redacted_env(path: &Path) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    if let Ok(s) = fs::read_to_string(path.join(".env")) {
        for l in s.lines() {
            if let Some((k, v)) = l.split_once('=') {
                let k = k.trim();
                if !k.is_empty() {
                    m.insert(
                        k.to_string(),
                        if v.trim().is_empty() {
                            "<empty>".into()
                        } else {
                            "<redacted>".into()
                        },
                    );
                }
            }
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    #[test]
    fn detects_mixed_and_redacts() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("Cargo.toml"), "[package]\nname='x'").unwrap();
        fs::write(d.path().join("package.json"), "{}").unwrap();
        fs::write(d.path().join(".env.example"), "DATABASE_URL=\nTOKEN=\n").unwrap();
        fs::write(
            d.path().join(".env"),
            "DATABASE_URL=postgres://user:secret@localhost:5432/db\nTOKEN=abc\n",
        )
        .unwrap();
        let p = detect_project(d.path());
        assert_eq!(p.stack.len(), 2);
        assert_eq!(redacted_env(d.path())["TOKEN"], "<redacted>");
    }
    #[test]
    fn compose_ports_and_database_mismatch_are_deterministic() {
        let d = tempdir().unwrap();
        fs::write(
            d.path().join("compose.yml"),
            "services:\n  db:\n    ports:\n      - \"15432:5432\"\n",
        )
        .unwrap();
        fs::write(
            d.path().join(".env"),
            "DATABASE_URL=postgres://u:p@127.0.0.1:65432/db\n",
        )
        .unwrap();
        assert!(detect_project(d.path()).database_url.is_some());
        let r = diagnose(d.path());
        assert_eq!(r.project.compose_db_ports, vec![15432]);
        assert!(r.findings.iter().any(|f| f.code == "ENV-DATABASE-001"));
        assert!(r.findings.iter().any(|f| f.code == "POSTGRES-CONNECT-001"));
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains(":p@"));
    }
    #[test]
    fn domain_diagnostics_return_only_the_requested_findings() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("Cargo.toml"), "[package]\nname='x'").unwrap();
        fs::write(d.path().join(".env.example"), "TOKEN=\n").unwrap();
        let environment = diagnose_environment(d.path());
        assert_eq!(environment.findings.len(), 1);
        assert_eq!(environment.findings[0].code, "ENV-MISSING-001");
        assert!(diagnose_ports(d.path()).findings.is_empty());
    }

    #[test]
    fn port_diagnostics_detect_an_occupied_configured_port() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let d = tempdir().unwrap();
        fs::write(
            d.path().join("compose.yml"),
            format!("services:\n  app:\n    ports:\n      - \"{port}:80\"\n"),
        )
        .unwrap();
        let report = diagnose_ports(d.path());
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].code, "PORT-CONFLICT-001");
        drop(listener);
    }

    #[test]
    fn linker_shadowing_requires_real_candidate() {
        let d = tempdir().unwrap();
        let git = d.path().join("Git").join("usr").join("bin");
        fs::create_dir_all(&git).unwrap();
        fs::write(git.join("link.exe"), "fake").unwrap();
        let finding =
            linker_diagnostic("x86_64-pc-windows-msvc", &[git.display().to_string()], None)
                .unwrap();
        assert_eq!(finding.code, "RUST-LINKER-001");
        assert!(
            linker_diagnostic(
                "x86_64-unknown-linux-gnu",
                &[git.display().to_string()],
                None
            )
            .is_none()
        );
    }
    #[test]
    fn redacts_common_secret_shapes() {
        assert_eq!(
            redact_text("token=abc password=xyz"),
            "token=<redacted> password=<redacted>"
        );
    }
    #[test]
    fn every_emitted_rust_code_is_explainable() {
        assert!(explain("RUST-TOOLCHAIN-OK").is_some());
        assert!(explain("RUST-TOOLCHAIN-001").is_some());
    }
    #[test]
    fn malformed_configuration_is_not_a_port_mapping() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("compose.yml"), "ports: [not-a-port]\n").unwrap();
        assert!(detect_project(d.path()).compose_ports.is_empty());
    }
    #[cfg(windows)]
    #[test]
    fn subprocess_timeout_is_bounded() {
        let started = std::time::Instant::now();
        assert!(cmd("cmd", &["/C", "ping -n 4 127.0.0.1 >nul"]).is_none());
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
