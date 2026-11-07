use clap::{Parser, Subcommand};
use devdoctor_core::{diagnose, explain};
use std::path::PathBuf;
#[derive(Parser)]
#[command(
    name = "devdoctor",
    version,
    about = "Local-first developer environment diagnostics"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Subcommand)]
enum Command {
    Doctor {
        #[arg(long)]
        path: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Check {
        area: String,
    },
    Explain {
        code: String,
    },
}
fn main() {
    let cli = Cli::parse();
    let result = match cli.command.unwrap_or(Command::Doctor {
        path: None,
        json: false,
    }) {
        Command::Doctor { path, json } => {
            let p = path.unwrap_or_else(|| std::env::current_dir().unwrap());
            let r = diagnose(&p);
            if json {
                println!("{}", serde_json::to_string_pretty(&r).unwrap());
            } else {
                println!(
                    "DevDoctor\nProject: {}\nStack: {}",
                    r.project.name,
                    if r.project.stack.is_empty() {
                        "unknown".into()
                    } else {
                        r.project.stack.join(" + ")
                    }
                );
                if r.findings.is_empty() {
                    println!("\nSummary: healthy");
                } else {
                    for f in &r.findings {
                        println!("{:?} {}: {}", f.severity, f.code, f.title);
                    }
                    println!("\nSummary: {} finding(s)", r.findings.len());
                }
            }
            if r.findings.iter().any(|f| {
                matches!(
                    f.severity,
                    devdoctor_core::Severity::Error | devdoctor_core::Severity::Warning
                )
            }) {
                1
            } else {
                0
            }
        }
        Command::Check { area } => {
            let p = std::env::current_dir().unwrap();
            let r = diagnose(&p);
            for f in r
                .findings
                .iter()
                .filter(|f| f.component.to_lowercase() == area.to_lowercase())
            {
                println!("{}: {}", f.code, f.title);
            }
            0
        }
        Command::Explain { code } => match explain(&code) {
            Some((t, e)) => {
                println!("{code}: {t}\n{e}");
                0
            }
            None => {
                eprintln!("Unknown finding code: {code}");
                2
            }
        },
    };
    std::process::exit(result)
}
