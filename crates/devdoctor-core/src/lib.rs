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
    process::Command,
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
        p.database_url = find_var(&v, "DATABASE_URL");
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
fn parse_ports(s: &str, p: &mut Project) {
    for line in s.lines() {
        let t = line.trim().trim_matches(['"', '\'']);
        if t.contains(':') {
            let a = t.split(':').collect::<Vec<_>>();
            if a.len() >= 2 {
                let left = a[a.len() - 2].trim_matches(['"', '\'']);
                let right = a[a.len() - 1].split(['#', '"']).next().unwrap_or("").trim();
                if let (Ok(host), Ok(container)) = (left.parse::<u16>(), right.parse::<u16>()) {
                    p.compose_ports.push(host);
                    if container == 5432 {
                        p.compose_db_ports.push(host)
                    }
                }
            }
        }
    }
}
fn parse_url_port(url: &str) -> Option<u16> {
    url.split('@')
        .next_back()?
        .split(':')
        .next_back()?
        .split('/')
        .next()?
        .parse()
        .ok()
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
fn cmd(program: &str, args: &[&str]) -> Option<String> {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
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
    if !p.env_example.is_empty() && !path.join(".env").is_file() {
        f.push(finding(
            "ENV-MISSING-001",
            Severity::Warning,
            ".env file is missing",
            "Environment",
            ".env.example declares variables but .env is absent.",
            p.env_example.clone(),
            Confidence::Confirmed,
            "Create .env from .env.example without committing secrets.",
        ));
    }
    if let Some(url) = &p.database_url {
        if let Some(port) = parse_url_port(url) {
            if !p.compose_db_ports.is_empty() && !p.compose_db_ports.contains(&port) {
                f.push(finding(
                    "ENV-DATABASE-001",
                    Severity::Warning,
                    "DATABASE_URL port does not match Compose",
                    "PostgreSQL",
                    "The configured database URL uses a host port not published by Compose.",
                    vec![
                        format!("configured_port={port}"),
                        format!("compose_ports={:?}", p.compose_db_ports),
                    ],
                    Confidence::Confirmed,
                    "Align DATABASE_URL with the published PostgreSQL host port.",
                ));
            }
        }
    }
    Report {
        project: p,
        findings: f,
    }
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
}
