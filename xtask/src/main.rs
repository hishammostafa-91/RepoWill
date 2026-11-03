// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::process::Command;

#[derive(Parser, Debug)]
#[command(name = "xtask", version, about = "RepoWill development automation")]
struct Cli {
    #[command(subcommand)]
    command: CommandKind,
}

#[derive(Subcommand, Debug)]
enum CommandKind {
    /// Run formatting, lints and policy checks.
    Check,
    /// Run all tests.
    Test,
    /// Run the desktop app in development mode.
    GuiDev,
    /// Run everything (check + test).
    Verify,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        CommandKind::Check => run_check(),
        CommandKind::Test => run_test(),
        CommandKind::GuiDev => run_gui_dev(),
        CommandKind::Verify => {
            run_check()?;
            run_test()?;
            println!("verify: OK");
            Ok(())
        }
    }
}

fn run_check() -> Result<()> {
    println!("== cargo fmt --check ==");
    run("cargo", &["fmt", "--all", "--", "--check"])?;
    println!("== cargo clippy ==");
    run(
        "cargo",
        &[
            "clippy",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    println!("== cargo deny ==");
    run("cargo", &["deny", "check"])?;
    println!("check: OK");
    Ok(())
}

fn run_test() -> Result<()> {
    println!("== cargo test ==");
    run("cargo", &["test", "--all"])?;
    println!("test: OK");
    Ok(())
}

fn run_gui_dev() -> Result<()> {
    println!("== gui-dev ==");
    run("pnpm", &["--dir", "apps/gui", "tauri", "dev"])
}

fn run(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("failed to run {program}"))?;
    if !status.success() {
        anyhow::bail!("{program} {:?} failed with {status}", args);
    }
    Ok(())
}
