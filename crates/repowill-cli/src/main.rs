// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use repowill_core::ops::{self, OpContext};
use serde_json::json;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "repowill",
    version,
    about = "RepoWill: repository rules, gates and guardrails"
)]
struct Cli {
    #[arg(short = 'C', long = "repo")]
    repo: Option<PathBuf>,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    progress: bool,
    #[arg(short = 'y', long = "yes")]
    yes: bool,
    #[arg(long)]
    non_interactive: bool,
    #[arg(long)]
    profile: Option<String>,
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
    #[arg(long, default_value = "auto")]
    color: String,
    #[arg(long)]
    no_network: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Print version information.
    Version,
    /// Manage RepoWill configuration.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Inspect and edit repository policy.
    Policy {
        #[command(subcommand)]
        command: PolicyCommand,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigCommand {
    Get {
        #[arg(long)]
        key: String,
    },
    Set {
        #[arg(long)]
        key: String,
        #[arg(long)]
        value: String,
    },
    List,
    Path,
}

#[derive(Subcommand, Debug)]
enum PolicyCommand {
    Show,
    Validate {
        #[arg(long)]
        path: Option<PathBuf>,
        #[arg(long)]
        source: Option<String>,
    },
    Schema,
    Explain {
        #[arg(long)]
        key: String,
    },
    Set {
        #[arg(long)]
        key: String,
        #[arg(long)]
        value: String,
    },
    Diff {
        #[arg(long)]
        from: Option<PathBuf>,
        #[arg(long)]
        to: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();
    let exit_code = run(cli);
    std::process::exit(exit_code);
}

fn run(cli: Cli) -> i32 {
    let repo_root = cli.repo.clone().or_else(|| std::env::current_dir().ok());
    let mut ctx = OpContext::new(repo_root.clone());
    if cli.progress {
        ctx = ctx.with_progress(|event| {
            let line = serde_json::to_string(&event).unwrap_or_default();
            eprintln!("{line}");
        });
    }

    let (op, params) = match cli.command {
        Commands::Version => (repowill_core::Operation::AppVersion, json!({})),
        Commands::Config { command } => match command {
            ConfigCommand::Get { key } => {
                (repowill_core::Operation::ConfigGet, json!({ "key": key }))
            }
            ConfigCommand::Set { key, value } => (
                repowill_core::Operation::ConfigSet,
                json!({ "key": key, "value": value }),
            ),
            ConfigCommand::List => (repowill_core::Operation::ConfigList, json!({})),
            ConfigCommand::Path => (repowill_core::Operation::ConfigPath, json!({})),
        },
        Commands::Policy { command } => match command {
            PolicyCommand::Show => (repowill_core::Operation::PolicyShow, json!({})),
            PolicyCommand::Validate { path, source } => {
                let mut params = json!({});
                if let Some(path) = path {
                    params["path"] = json!(path);
                }
                if let Some(source) = source {
                    params["source"] = json!(source);
                }
                (repowill_core::Operation::PolicyValidate, params)
            }
            PolicyCommand::Schema => (repowill_core::Operation::PolicySchema, json!({})),
            PolicyCommand::Explain { key } => (
                repowill_core::Operation::PolicyExplain,
                json!({ "key": key }),
            ),
            PolicyCommand::Set { key, value } => (
                repowill_core::Operation::PolicySet,
                json!({ "key": key, "value": value }),
            ),
            PolicyCommand::Diff { from, to } => {
                let mut params = json!({});
                if let Some(from) = from {
                    params["from"] = json!(from);
                }
                if let Some(to) = to {
                    params["to"] = json!(to);
                }
                (repowill_core::Operation::PolicyDiff, params)
            }
        },
    };

    match ops::execute(op, params, &ctx) {
        Ok(data) => {
            if cli.json {
                let envelope = json!({
                    "ok": true,
                    "op": op.as_str(),
                    "repowill": env!("CARGO_PKG_VERSION"),
                    "api": 1,
                    "data": data,
                    "warnings": [],
                    "error": null
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&envelope).unwrap_or_default()
                );
            } else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&data).unwrap_or_default()
                );
            }
            0
        }
        Err(error) => {
            if cli.json {
                let envelope = json!({
                    "ok": false,
                    "op": op.as_str(),
                    "repowill": env!("CARGO_PKG_VERSION"),
                    "api": 1,
                    "data": null,
                    "warnings": [],
                    "error": error
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&envelope).unwrap_or_default()
                );
            } else {
                eprintln!("{}: {}", error.code, error.message);
                if let Some(details) = &error.details
                    && let Some(diagnostics) = details
                        .get("diagnostics")
                        .and_then(|value| value.as_array())
                {
                    for diagnostic in diagnostics {
                        let code = diagnostic
                            .get("code")
                            .and_then(|value| value.as_str())
                            .unwrap_or("RW-E2000");
                        let message = diagnostic
                            .get("message")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        let line = diagnostic.get("line").and_then(|value| value.as_u64());
                        let column = diagnostic.get("column").and_then(|value| value.as_u64());
                        let fix = diagnostic.get("fix").and_then(|value| value.as_str());
                        match (line, column) {
                            (Some(line), Some(column)) => {
                                eprintln!("{code}: {message} (line {line}, column {column})")
                            }
                            (Some(line), None) => eprintln!("{code}: {message} (line {line})"),
                            _ => eprintln!("{code}: {message}"),
                        }
                        if let Some(fix) = fix {
                            eprintln!("  hint: {fix}");
                        }
                    }
                } else if let Some(ref hint) = error.hint {
                    eprintln!("hint: {hint}");
                }
            }
            error.exit_code()
        }
    }
}
