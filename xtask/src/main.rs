// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
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

const PNPM: &str = if cfg!(windows) { "pnpm.cmd" } else { "pnpm" };

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
    println!("== frontend lockfile policy ==");
    run_in("apps/gui", PNPM, &["install", "--frozen-lockfile"])?;
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
    run(PNPM, &["--dir", "apps/gui", "tauri", "dev"])
}

const ALLOWED_LICENSE_ATOMS: &[&str] = &[
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
];

fn license_atoms(expr: &str) -> Vec<String> {
    let mut parts = vec![expr.to_string()];
    for sep in [" OR ", " or ", " AND ", " and "] {
        parts = parts
            .into_iter()
            .flat_map(|p| p.split(sep).map(str::to_string).collect::<Vec<_>>())
            .collect();
    }
    parts
        .into_iter()
        .map(|s| {
            s.trim()
                .trim_matches(|c| c == '(' || c == ')')
                .trim()
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

fn license_violations(json: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(obj) = json.as_object() {
        for key in obj.keys() {
            for atom in license_atoms(key) {
                if !ALLOWED_LICENSE_ATOMS.contains(&atom.as_str()) {
                    out.push(key.clone());
                    break;
                }
            }
        }
    }
    out
}

fn run_licenses() -> Result<()> {
    let output = Command::new(PNPM)
        .current_dir("apps/gui")
        .args(["licenses", "list", "--json"])
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
    let violations = license_violations(&json);
    if !violations.is_empty() {
        anyhow::bail!("disallowed frontend licenses: {}", violations.join(", "));
    }
    println!("licenses: OK");
    Ok(())
}

fn repo_root() -> Result<std::path::PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("git rev-parse failed")?;
    if !output.status.success() {
        anyhow::bail!("not a git repository");
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(std::path::PathBuf::from(path))
}

fn spdx_check() -> Result<()> {
    let root = repo_root()?;
    let output = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["ls-files"])
        .output()
        .context("git ls-files failed")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let exts = ["rs", "ts", "tsx", "css", "html"];
    for line in stdout.lines() {
        let path = root.join(line);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !exts.contains(&ext) {
            continue;
        }
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => anyhow::bail!("cannot read {}: {}", line, e),
        };
        if !content.contains("SPDX-License-Identifier") {
            anyhow::bail!("missing SPDX header: {}", line);
        }
    }
    Ok(())
}

fn url_check() -> Result<()> {
    let root = repo_root()?;
    let output = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["ls-files"])
        .output()
        .context("git ls-files failed")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let needle = concat!("hishammostafa-91", "/", "RepoWill");
    for line in stdout.lines() {
        if line == "assets/project.toml" {
            continue;
        }
        let path = root.join(line);
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn spdx_check_passes_on_repo() {
        spdx_check().expect("spdx");
    }

    #[test]
    fn url_check_passes_on_repo() {
        url_check().expect("url");
    }

    #[test]
    fn pnpm_binary_is_platform_appropriate() {
        if cfg!(windows) {
            assert_eq!(PNPM, "pnpm.cmd");
        } else {
            assert_eq!(PNPM, "pnpm");
        }
    }

    #[test]
    fn license_violations_empty_for_allowed() {
        let j = json!({"MIT": [], "Apache-2.0": []});
        assert!(license_violations(&j).is_empty());
    }

    #[test]
    fn license_violations_catches_disallowed() {
        let j = json!({"GPL-3.0-only": []});
        let v = license_violations(&j);
        assert_eq!(v, vec!["GPL-3.0-only".to_string()]);
    }

    #[test]
    fn license_violations_accepts_composite_atoms() {
        let j = json!({"MIT OR Apache-2.0": []});
        assert!(license_violations(&j).is_empty());
    }

    #[test]
    fn license_violations_rejects_reordered_unknown() {
        let j = json!({"MIT OR GPL-3.0-only": []});
        let v = license_violations(&j);
        assert_eq!(v, vec!["MIT OR GPL-3.0-only".to_string()]);
    }

    #[test]
    fn license_atoms_splits_or_and() {
        assert_eq!(
            license_atoms("MIT OR Apache-2.0"),
            vec!["MIT", "Apache-2.0"]
        );
    }
}
