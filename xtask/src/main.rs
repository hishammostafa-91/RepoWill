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
    /// Generate JSON schema and bindings.
    Gen,
    /// Check CLI/policy operation parity.
    Parity,
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
            run_parity()?;
            run_test()?;
            println!("verify: OK");
            Ok(())
        }
        CommandKind::Licenses => run_licenses(),
        CommandKind::Gen => run_gen(),
        CommandKind::Parity => run_parity(),
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

fn run_gen() -> Result<()> {
    println!("== gen schema ==");
    std::fs::create_dir_all("schemas").context("create schemas dir")?;
    let output = Command::new("cargo")
        .args(["run", "-p", "repowill-model", "--example", "gen_schema"])
        .output()
        .context("cargo run gen_schema")?;
    if !output.status.success() {
        anyhow::bail!(
            "gen_schema failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    std::fs::write("schemas/policy.schema.json", &output.stdout).context("write schema")?;
    std::fs::create_dir_all("docs/reference").context("create docs/reference")?;
    std::fs::write(
        "docs/reference/cli.md",
        "# CLI reference\n\nImplemented operations are exposed through `repowill`.\n",
    )
    .context("write cli reference")?;
    std::fs::write(
        "docs/reference/policy.md",
        "# Policy reference\n\nSee `docs/config-engine.md` and `schemas/policy.schema.json`.\n",
    )
    .context("write policy reference")?;
    println!("gen: OK");
    Ok(())
}

fn run_parity() -> Result<()> {
    println!("== parity ==");
    for args in [
        vec!["--help"],
        vec!["config", "--help"],
        vec!["policy", "--help"],
        vec!["op", "--help"],
    ] {
        let status = Command::new("cargo")
            .args(["run", "-q", "-p", "repowill-cli", "--"])
            .args(&args)
            .status()
            .with_context(|| format!("cargo run repowill {}", args.join(" ")))?;
        if !status.success() {
            anyhow::bail!("cli help failed for {}", args.join(" "));
        }
    }
    let output = Command::new("cargo")
        .args([
            "run",
            "-q",
            "-p",
            "repowill-cli",
            "--",
            "op",
            "{\"op\":\"app.version\"}",
        ])
        .output()
        .context("cargo run repowill op app.version")?;
    if !output.status.success() {
        anyhow::bail!(
            "implemented op failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let status = Command::new("cargo")
        .args([
            "run",
            "-q",
            "-p",
            "repowill-cli",
            "--",
            "op",
            "{\"op\":\"project.detect\"}",
        ])
        .status()
        .context("cargo run repowill op project.detect")?;
    if status.code() != Some(2) {
        anyhow::bail!("planned op did not exit with code 2");
    }
    println!("parity: OK");
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
    } else {
        out.push("<invalid-license-report>".to_string());
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
    spdx_check_in(&repo_root()?)
}

fn spdx_check_in(root: &std::path::Path) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files"])
        .output()
        .context("git ls-files failed")?;
    if !output.status.success() {
        anyhow::bail!("git ls-files failed");
    }
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
    url_check_in(&repo_root()?)
}

fn url_check_in(root: &std::path::Path) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files"])
        .output()
        .context("git ls-files failed")?;
    if !output.status.success() {
        anyhow::bail!("git ls-files failed");
    }
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
    use std::fs;
    use std::process::Command;
    use tempfile::tempdir;

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

    #[test]
    fn license_violations_fail_closed_on_non_object() {
        let j = json!(["MIT"]);
        let v = license_violations(&j);
        assert!(!v.is_empty());
    }

    #[test]
    fn spdx_check_fails_on_missing_header() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        Command::new("git")
            .arg("init")
            .current_dir(root)
            .status()
            .unwrap();
        fs::write(root.join("bad.rs"), "fn main() {}").unwrap();
        Command::new("git")
            .args(["add", "bad.rs"])
            .current_dir(root)
            .status()
            .unwrap();
        let err = spdx_check_in(root).unwrap_err();
        assert!(err.to_string().contains("missing SPDX header"));
    }

    #[test]
    fn url_check_fails_on_rogue_url() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        Command::new("git")
            .arg("init")
            .current_dir(root)
            .status()
            .unwrap();
        let needle = concat!("hishammostafa-91", "/", "RepoWill");
        fs::write(root.join("bad.txt"), format!("see {}", needle)).unwrap();
        Command::new("git")
            .args(["add", "bad.txt"])
            .current_dir(root)
            .status()
            .unwrap();
        let err = url_check_in(root).unwrap_err();
        assert!(err.to_string().contains("repository URL literal"));
    }
}
