// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use repowill_core::ops::{self, OpContext};
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "repowill",
    version,
    about = "RepoWill: repository rules, gates and guardrails"
)]
struct Cli {
    #[arg(short = 'C', long = "repo", global = true)]
    repo: Option<PathBuf>,
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true)]
    progress: bool,
    #[arg(short = 'y', long = "yes", global = true)]
    yes: bool,
    #[arg(long, global = true)]
    non_interactive: bool,
    #[arg(long, global = true)]
    profile: Option<String>,
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,
    #[arg(long, default_value = "auto", global = true)]
    color: String,
    #[arg(long, global = true)]
    no_network: bool,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Print version information.
    #[command(after_help = "Examples:\n  repowill version")]
    Version,
    /// Manage RepoWill configuration.
    #[command(
        after_help = "Examples:\n  repowill config path\n  repowill config get --key theme\n  repowill config set --key theme --value dark"
    )]
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Inspect and edit repository policy.
    #[command(
        after_help = "Examples:\n  repowill policy validate\n  repowill policy show\n  repowill policy schema"
    )]
    Policy {
        #[command(subcommand)]
        command: PolicyCommand,
    },
    /// Inspect repository audit log.
    #[command(after_help = "Examples:\n  repowill audit verify")]
    Audit {
        #[command(subcommand)]
        command: AuditCommand,
    },
    /// Detect project stack and existing configuration.
    #[command(after_help = "Examples:\n  repowill detect\n  repowill detect /path/to/repo")]
    Detect {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Initialise a new repository.
    #[command(
        after_help = "Examples:\n  repowill init /path/to/repo --name Foo --email foo@bar.com"
    )]
    Init {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        email: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        answers: Option<PathBuf>,
    },
    /// Adopt an existing repository.
    #[command(after_help = "Examples:\n  repowill adopt .")]
    Adopt {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        email: Option<String>,
    },
    /// Manage the projects registry.
    #[command(after_help = "Examples:\n  repowill projects list")]
    Projects {
        #[command(subcommand)]
        command: ProjectsCommand,
    },
    /// Run a raw operation from JSON.
    #[command(after_help = "Examples:\n  repowill op '{\"op\":\"app.version\"}'")]
    Op {
        /// JSON request, or @path to read it from a file.
        request: String,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigCommand {
    #[command(after_help = "Examples:\n  repowill config get --key theme")]
    Get {
        #[arg(long)]
        key: String,
    },
    #[command(after_help = "Examples:\n  repowill config set --key theme --value dark")]
    Set {
        #[arg(long)]
        key: String,
        #[arg(long)]
        value: String,
    },
    #[command(after_help = "Examples:\n  repowill config list")]
    List,
    #[command(after_help = "Examples:\n  repowill config path")]
    Path,
}

#[derive(Subcommand, Debug)]
enum PolicyCommand {
    #[command(after_help = "Examples:\n  repowill policy show")]
    Show {
        #[arg(long)]
        effective: bool,
        #[arg(long)]
        layers: bool,
        #[arg(long)]
        origin: bool,
    },
    #[command(
        after_help = "Examples:\n  repowill policy validate\n  repowill policy validate --path policy.toml"
    )]
    Validate {
        #[arg(long)]
        path: Option<PathBuf>,
        #[arg(long)]
        source: Option<String>,
    },
    #[command(after_help = "Examples:\n  repowill policy schema")]
    Schema,
    #[command(after_help = "Examples:\n  repowill policy explain --key guards.main_protection")]
    Explain {
        #[arg(long)]
        key: String,
    },
    #[command(after_help = "Examples:\n  repowill policy set --key project.name --value demo")]
    Set {
        #[arg(long)]
        key: String,
        #[arg(long)]
        value: String,
    },
    #[command(after_help = "Examples:\n  repowill policy diff")]
    Diff {
        #[arg(long)]
        from: Option<PathBuf>,
        #[arg(long)]
        to: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum ProjectsCommand {
    #[command(after_help = "Examples:\n  repowill projects list")]
    List,
    #[command(after_help = "Examples:\n  repowill projects add /path")]
    Add { path: PathBuf },
    #[command(after_help = "Examples:\n  repowill projects forget /path")]
    Forget { path: PathBuf },
}

#[derive(Subcommand, Debug)]
enum AuditCommand {
    #[command(after_help = "Examples:\n  repowill audit verify")]
    Verify,
    #[command(after_help = "Examples:\n  repowill audit log")]
    Log,
    #[command(after_help = "Examples:\n  repowill audit export --out audit.jsonl")]
    Export {
        #[arg(long)]
        out: PathBuf,
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
    if cli.json && cli.progress {
        ctx = ctx.with_progress(|event| {
            let line = serde_json::to_string(&event).unwrap_or_default();
            eprintln!("{line}");
        });
    }

    let command = match cli.command {
        Some(command) => command,
        None => {
            use clap::CommandFactory;
            Cli::command().print_help().ok();
            println!();
            return 0;
        }
    };

    if let Commands::Op { request } = command {
        let text = if let Some(path) = request.strip_prefix('@') {
            match std::fs::read_to_string(path) {
                Ok(text) => text,
                Err(error) => {
                    eprintln!("RW-E1004: cannot read request file: {error}");
                    return 2;
                }
            }
        } else {
            request.clone()
        };
        let value: Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("RW-E1004: invalid JSON request: {error}");
                return 2;
            }
        };
        let op_str = match value.get("op").and_then(Value::as_str) {
            Some(op) => op,
            None => {
                eprintln!("RW-E1004: request is missing `op`");
                return 2;
            }
        };
        let op: repowill_core::Operation = match serde_json::from_str(&format!("\"{op_str}\"")) {
            Ok(op) => op,
            Err(_) => {
                eprintln!("RW-E1004: unknown operation `{op_str}`");
                return 2;
            }
        };
        let params = value.get("params").cloned().unwrap_or_else(|| json!({}));
        let mut ctx = OpContext::new(repo_root.clone());
        if cli.json && cli.progress {
            ctx = ctx.with_progress(|event| {
                let line = serde_json::to_string(&event).unwrap_or_default();
                eprintln!("{line}");
            });
        }
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
                return 0;
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
                }
                return error.exit_code();
            }
        }
    }

    if let Commands::Detect { path } = command {
        let params = json!({ "path": path });
        let mut ctx = OpContext::new(repo_root.clone());
        if cli.json && cli.progress {
            ctx = ctx.with_progress(|event| {
                let line = serde_json::to_string(&event).unwrap_or_default();
                eprintln!("{line}");
            });
        }
        match ops::execute(repowill_core::Operation::ProjectDetect, params, &ctx) {
            Ok(data) => {
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &json!({"ok":true,"op":"project.detect","data":data})
                        )
                        .unwrap_or_default()
                    );
                } else {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&data).unwrap_or_default()
                    );
                }
                return 0;
            }
            Err(error) => {
                eprintln!("{}: {}", error.code, error.message);
                return error.exit_code();
            }
        }
    }

    if let Commands::Init {
        path,
        name,
        email,
        dry_run,
        answers,
    } = command
    {
        let has_answers = answers.is_some();
        let (path, name, email) = if let Some(answers_path) = answers {
            let text = std::fs::read_to_string(&answers_path).unwrap_or_default();
            let text = text.trim_start_matches('\u{feff}');
            let parsed: toml::Value =
                toml::from_str(text).unwrap_or(toml::Value::Table(Default::default()));
            let p = parsed
                .get("path")
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
                .unwrap_or(path);
            let n = parsed
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .or(name);
            let e = parsed
                .get("identity")
                .and_then(|v| v.get("email"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .or(email);
            (p, n, e)
        } else {
            (path, name, email)
        };
        if dry_run {
            let params = json!({ "path": path });
            let ctx = OpContext::new(repo_root.clone());
            match ops::execute(repowill_core::Operation::ProjectPlan, params, &ctx) {
                Ok(data) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&data).unwrap_or_default()
                    );
                    return 0;
                }
                Err(error) => {
                    eprintln!("{}: {}", error.code, error.message);
                    return error.exit_code();
                }
            }
        }
        let (path, name, email) = if name.is_none() && !has_answers && !cli.non_interactive {
            use dialoguer::{Confirm, Input, Select};
            let path_str: String = Input::new()
                .with_prompt("Project path")
                .default(path.to_string_lossy().to_string())
                .interact_text()
                .unwrap_or_default();
            let name: String = Input::new()
                .with_prompt("Project name")
                .default("my-project".to_string())
                .interact_text()
                .unwrap_or_default();
            let email: String = Input::new()
                .with_prompt("Owner email")
                .default("user@example.com".to_string())
                .interact_text()
                .unwrap_or_default();
            let detect_params = json!({ "path": path_str.clone() });
            let ctx = OpContext::new(repo_root.clone());
            if let Ok(d) =
                ops::execute(repowill_core::Operation::ProjectDetect, detect_params, &ctx)
            {
                println!("Detected stacks: {:?}", d.get("stacks"));
            }
            let presets = ["starter", "recommended", "strict", "maximum"];
            let preset_idx = Select::new()
                .with_prompt("Preset")
                .items(presets)
                .default(1)
                .interact()
                .unwrap_or(1);
            let _preset = presets[preset_idx];
            let apply = Confirm::new()
                .with_prompt("Apply now?")
                .default(true)
                .interact()
                .unwrap_or(false);
            if !apply {
                return 0;
            }
            (PathBuf::from(path_str), Some(name), Some(email))
        } else {
            (path, name, email)
        };

        let params = json!({
            "path": path,
            "name": name.unwrap_or_default(),
            "email": email.unwrap_or_default()
        });
        let ctx = OpContext::new(repo_root.clone());
        match ops::execute(repowill_core::Operation::ProjectInit, params, &ctx) {
            Ok(data) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&data).unwrap_or_default()
                );
                return 0;
            }
            Err(error) => {
                eprintln!("{}: {}", error.code, error.message);
                return error.exit_code();
            }
        }
    }

    if let Commands::Adopt { path, name, email } = command {
        let params = json!({ "path": path, "name": name.unwrap_or_default(), "email": email.unwrap_or_default() });
        let ctx = OpContext::new(repo_root.clone());
        match ops::execute(repowill_core::Operation::ProjectAdopt, params, &ctx) {
            Ok(data) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&data).unwrap_or_default()
                );
                return 0;
            }
            Err(error) => {
                eprintln!("{}: {}", error.code, error.message);
                return error.exit_code();
            }
        }
    }

    if let Commands::Projects { command } = command {
        let (op, params) = match command {
            ProjectsCommand::List => (repowill_core::Operation::ProjectList, json!({})),
            ProjectsCommand::Add { path } => (
                repowill_core::Operation::ProjectAdd,
                json!({ "path": path }),
            ),
            ProjectsCommand::Forget { path } => (
                repowill_core::Operation::ProjectForget,
                json!({ "path": path }),
            ),
        };
        let ctx = OpContext::new(repo_root.clone());
        match ops::execute(op, params, &ctx) {
            Ok(data) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&data).unwrap_or_default()
                );
                return 0;
            }
            Err(error) => {
                eprintln!("{}: {}", error.code, error.message);
                return error.exit_code();
            }
        }
    }

    if let Commands::Audit { command } = command {
        let (op, params) = match command {
            AuditCommand::Verify => (repowill_core::Operation::AuditVerify, json!({})),
            AuditCommand::Log => (repowill_core::Operation::AuditLog, json!({})),
            AuditCommand::Export { out } => {
                (repowill_core::Operation::AuditExport, json!({ "out": out }))
            }
        };
        let mut ctx = OpContext::new(repo_root.clone());
        if cli.json && cli.progress {
            ctx = ctx.with_progress(|event| {
                let line = serde_json::to_string(&event).unwrap_or_default();
                eprintln!("{line}");
            });
        }
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
                return 0;
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
                }
                return error.exit_code();
            }
        }
    }

    let (op, params) = match command {
        Commands::Version => (repowill_core::Operation::AppVersion, json!({})),
        Commands::Op { .. } => unreachable!(),
        Commands::Audit { .. } => unreachable!(),
        Commands::Detect { .. } => unreachable!(),
        Commands::Init { .. } => unreachable!(),
        Commands::Adopt { .. } => unreachable!(),
        Commands::Projects { .. } => unreachable!(),
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
            PolicyCommand::Show {
                effective,
                layers,
                origin,
            } => (
                repowill_core::Operation::PolicyShow,
                json!({
                    "effective": effective,
                    "layers": layers,
                    "origin": origin
                }),
            ),
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
