// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::Path;
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
    /// List frontend licences (AT-459).
    Licenses,
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
        CommandKind::Licenses => run_licenses(),
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
    println!("== biome ==");
    run_in(
        "apps/gui",
        "node",
        &["node_modules/@biomejs/biome/bin/biome", "check", "."],
    )?;
    println!("== tsc ==");
    run_in(
        "apps/gui",
        "node",
        &["node_modules/typescript/bin/tsc", "--noEmit"],
    )?;
    println!("== SPDX headers ==");
    spdx_check()?;
    println!("== repository URL scan ==");
    url_check()?;
    println!("== frontend licences (AT-459) ==");
    run_licenses()?;
    println!("check: OK");
    Ok(())
}

fn run_test() -> Result<()> {
    println!("== cargo test ==");
    run("cargo", &["test", "--all", "--locked"])?;
    println!("test: OK");
    Ok(())
}

fn run_gui_dev() -> Result<()> {
    println!("== gui-dev ==");
    run("pnpm.cmd", &["--dir", "apps/gui", "tauri", "dev"])
}

const ALLOWED_LICENSES: &[&str] = &[
    "MIT",
    "MIT-0",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "0BSD",
    "Zlib",
    "BSL-1.0",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "MPL-2.0",
    "CC0-1.0",
    "Unlicense",
    "BlueOak-1.0.0",
    "Python-2.0",
    "CDLA-Permissive-2.0",
    "CC-BY-4.0",
    "AGPL-3.0-or-later",
    "(MIT OR CC0-1.0)",
    "MIT OR Apache-2.0",
    "Apache-2.0 OR MIT",
];

fn run_licenses() -> Result<()> {
    let output = Command::new("pnpm.cmd")
        .args(["--dir", "apps/gui", "licenses", "list", "--json"])
        .output()
        .context("pnpm licenses failed")?;
    if !output.status.success() {
        anyhow::bail!(
            "pnpm licenses failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let json: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&output.stdout))
        .context("parse licenses json")?;
    if let Some(obj) = json.as_object() {
        for key in obj.keys() {
            if !ALLOWED_LICENSES.contains(&key.as_str()) {
                anyhow::bail!("disallowed frontend license: {}", key);
            }
        }
    }
    println!("licenses: OK");
    Ok(())
}

fn spdx_check() -> Result<()> {
    let output = Command::new("git")
        .args(["ls-files"])
        .output()
        .context("git ls-files failed")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let exts = ["rs", "ts", "tsx", "css", "html"];
    for line in stdout.lines() {
        let path = Path::new(line);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !exts.contains(&ext) {
            continue;
        }
        let content = fs::read_to_string(path).unwrap_or_default();
        if !content.contains("SPDX-License-Identifier") {
            anyhow::bail!("missing SPDX header: {}", line);
        }
    }
    Ok(())
}

fn url_check() -> Result<()> {
    let output = Command::new("git")
        .args(["ls-files"])
        .output()
        .context("git ls-files failed")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let needle = concat!("hishammostafa-91", "/", "RepoWill");
    for line in stdout.lines() {
        if line == "assets/project.toml" {
            continue;
        }
        let content = fs::read_to_string(line).unwrap_or_default();
        if content.contains(needle) {
            anyhow::bail!(
                "repository URL literal found outside assets/project.toml: {}",
                line
            );
        }
    }
    Ok(())
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

fn run_in(dir: &str, program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .current_dir(dir)
        .args(args)
        .status()
        .with_context(|| format!("failed to run {program} in {dir}"))?;
    if !status.success() {
        anyhow::bail!("{program} {:?} failed with {status}", args);
    }
    Ok(())
}
