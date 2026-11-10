// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

//! Project detection, scaffolding, init and adopt.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub fn detect(root: &Path) -> Value {
    let mut stacks = Vec::new();
    let exists = |name: &str| root.join(name).exists();
    if exists("package.json") || exists("tsconfig.json") {
        stacks.push("node");
    }
    if exists("Cargo.toml") {
        stacks.push("rust");
    }
    if exists("pyproject.toml") || exists("requirements.txt") || exists("setup.py") {
        stacks.push("python");
    }
    if exists("go.mod") {
        stacks.push("go");
    }
    if exists("pom.xml") || exists("build.gradle") || exists("build.gradle.kts") {
        stacks.push("java");
    }
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if (name.ends_with(".csproj") || name.ends_with(".sln")) && !stacks.contains(&"dotnet")
            {
                stacks.push("dotnet");
            }
        }
    }
    if stacks.is_empty() {
        stacks.push("generic");
    }

    // Proposed preset based on stacks.
    let proposed_preset = if stacks.contains(&"rust") || stacks.contains(&"go") {
        "strict"
    } else {
        "recommended"
    };

    // Proposed gate categories based on stacks (actual gate IDs come from the catalog in WP-04).
    let mut proposed_gate_categories = vec!["format", "lint", "build"];
    if stacks.contains(&"rust") {
        proposed_gate_categories.push("test");
        proposed_gate_categories.push("clippy");
    }
    if stacks.contains(&"node") {
        proposed_gate_categories.push("eslint");
        proposed_gate_categories.push("typecheck");
        proposed_gate_categories.push("test");
    }
    if stacks.contains(&"python") {
        proposed_gate_categories.push("ruff");
        proposed_gate_categories.push("pytest");
    }

    // Proposed adapters based on stacks.
    let mut proposed_adapters = Vec::new();
    if stacks.contains(&"node") {
        proposed_adapters.push("biome");
    }
    if stacks.contains(&"rust") {
        proposed_adapters.push("cargo");
    }
    if stacks.contains(&"python") {
        proposed_adapters.push("ruff");
    }

    let mut hook_managers = Vec::new();
    if root.join(".husky").exists() {
        hook_managers.push("husky");
    }
    if root.join(".pre-commit-config.yaml").exists() || root.join(".pre-commit-config.yml").exists()
    {
        hook_managers.push("pre-commit");
    }
    if root.join("lefthook.yml").exists() || root.join("lefthook.yaml").exists() {
        hook_managers.push("lefthook");
    }
    if git_config_hooks_path(root).is_some() {
        hook_managers.push("core.hooksPath");
    }

    let mut agent_configs = Vec::new();
    if root.join("AGENTS.md").exists() {
        agent_configs.push("AGENTS.md");
    }
    if root.join("CLAUDE.md").exists() {
        agent_configs.push("CLAUDE.md");
    }
    if root.join(".opencode").exists() {
        agent_configs.push(".opencode");
    }
    if root.join(".claude").exists() {
        agent_configs.push(".claude");
    }

    let mut existing_files = Vec::new();
    for name in [
        ".gitattributes",
        ".gitignore",
        ".editorconfig",
        "README.md",
        "LICENSE",
        "CHANGELOG.md",
        ".repowill/policy.toml",
    ] {
        if root.join(name).exists() {
            existing_files.push(name.to_string());
        }
    }

    let is_git = root.join(".git").exists();

    json!({
        "root": root,
        "is_git": is_git,
        "stacks": stacks,
        "hook_managers": hook_managers,
        "agent_configs": agent_configs,
        "existing_files": existing_files,
        "proposed_preset": proposed_preset,
        "proposed_gate_categories": proposed_gate_categories,
        "proposed_adapters": proposed_adapters
    })
}

fn git_config_hooks_path(root: &Path) -> Option<String> {
    let git = crate::git::Git::new().ok()?;
    git.run(&["config", "--get", "core.hooksPath"], Some(root))
        .ok()
        .and_then(|r| {
            if r.code == Some(0) {
                Some(r.stdout.trim().to_string())
            } else {
                None
            }
        })
}

pub fn plan_files(root: &Path, stacks: &[String]) -> Vec<(String, String)> {
    let _ = root;
    let files = vec![
        (".gitattributes".to_string(), gitattributes_content()),
        (".gitignore".to_string(), gitignore_for_stacks(stacks)),
        (".editorconfig".to_string(), editorconfig_content()),
        ("README.md".to_string(), readme_content()),
        ("LICENSE".to_string(), license_content()),
        ("CHANGELOG.md".to_string(), changelog_content()),
        (".repowill/policy.toml".to_string(), policy_content()),
    ];
    // Add stack-specific gitignore entries by appending to the base .gitignore.
    files
}

pub fn gitignore_for_stacks(stacks: &[String]) -> String {
    let mut content = String::from("# RepoWill\n.repowill/local.toml\n.repowill/ops.lock\n");
    for stack in stacks {
        match stack.as_str() {
            "node" => content
                .push_str("\n# Node\nnode_modules/\n.next/\ndist/\n*.log\n.env\n.env.local\n"),
            "rust" => content.push_str("\n# Rust\ntarget/\n"),
            "python" => content.push_str("\n# Python\n__pycache__/\n*.pyc\n.venv/\nvenv/\n"),
            "go" => content.push_str("\n# Go\nbin/\n*.exe\n*.test\n*.out\n"),
            "dotnet" => content.push_str("\n# .NET\nbin/\nobj/\n*.user\n*.suo\n"),
            "java" => content.push_str("\n# Java\ntarget/\n*.class\n*.jar\n.gradle/\nbuild/\n"),
            _ => {}
        }
    }
    content
}

pub fn checkpoint_path(root: &Path) -> std::path::PathBuf {
    root.join(".repowill").join("init-checkpoint.json")
}

pub fn apply_init(root: &Path, identity_name: &str, identity_email: &str) -> Result<Value, String> {
    if identity_name.is_empty() || identity_email.is_empty() {
        return Err("identity name and email are required".to_string());
    }
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let git_dir = root.join(".git");
    if !git_dir.exists() {
        let git = crate::git::Git::new()?;
        let result = git.run(&["init", "-b", "main"], Some(root))?;
        if result.code != Some(0) {
            return Err(format!("git init failed: {}", result.stderr));
        }
    } else {
        // Existing repo: check if it has commits.
        let git = crate::git::Git::new()?;
        let result = git.run(&["rev-parse", "--verify", "HEAD"], Some(root));
        if let Ok(r) = result
            && r.code == Some(0)
        {
            // Already has commits; don't commit on main. Use adopt instead.
            return Err(
                "repository already has commits; use `adopt` instead of `init`".to_string(),
            );
        }
    }
    let detection = detect(root);
    let stacks: Vec<String> = detection
        .get("stacks")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let files = plan_files(root, &stacks);

    // Load checkpoint if resuming.
    let checkpoint = checkpoint_path(root);
    let done: std::collections::HashSet<String> = if checkpoint.exists() {
        std::fs::read_to_string(&checkpoint)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| v.get("written").cloned())
            .and_then(|v| v.as_array().cloned())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        std::collections::HashSet::new()
    };

    let mut written = Vec::new();
    let mut skipped = Vec::new();
    for (rel, content) in &files {
        if done.contains(rel) {
            skipped.push(rel.clone());
            continue;
        }
        let path = root.join(rel);
        if path.exists() {
            skipped.push(rel.clone());
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        // Backup existing file before overwriting (FR-INIT-007).
        let _ = crate::platform::fs::backup_file(&path);
        // Atomic write (NFR-REL-002).
        crate::platform::fs::atomic_write(&path, content.as_bytes()).map_err(|e| e.to_string())?;
        written.push(rel.clone());

        // Save checkpoint.
        let cp = serde_json::json!({ "written": written });
        let _ = crate::platform::fs::atomic_write(&checkpoint, cp.to_string().as_bytes());
    }

    // Clean up checkpoint on successful completion.
    let _ = std::fs::remove_file(&checkpoint);
    let git = crate::git::Git::new()?;
    let result = git.run(&["config", "user.name", identity_name], Some(root))?;
    if result.code != Some(0) {
        return Err(format!("git config user.name failed: {}", result.stderr));
    }
    let result = git.run(&["config", "user.email", identity_email], Some(root))?;
    if result.code != Some(0) {
        return Err(format!("git config user.email failed: {}", result.stderr));
    }
    #[cfg(windows)]
    {
        let _ = git.run(&["config", "core.longpaths", "true"], Some(root));
    }
    // Stage only the scaffold files we wrote, never pre-existing content.
    for rel in &written {
        let result = git.run(&["add", "--", rel], Some(root))?;
        if result.code != Some(0) {
            return Err(format!("git add failed: {}", result.stderr));
        }
    }
    if written.is_empty() {
        return Ok(
            json!({ "root": root, "written": written, "skipped": skipped, "committed": false }),
        );
    }
    // Create a one-time internal token for the guarded initial commit (FR-INIT-006).
    let store = crate::state::StateStore::from_common_dir(
        &crate::git::common_dir(root).unwrap_or_else(|_| root.join(".git")),
    );
    let _ = store.ensure();
    let token_id = store
        .create_token("init", root.to_string_lossy().as_ref(), None, None, 120)
        .map_err(|e| format!("failed to create init token: {e}"))?;

    let git = crate::git::Git::new()?;
    let mut env = git.env.clone();
    env.push(("REPOWILL_TOKEN".to_string(), token_id.clone()));
    let mut argv: Vec<String> = vec![git.executable.to_string_lossy().into_owned()];
    argv.push("-C".to_string());
    argv.push(root.to_string_lossy().into_owned());
    argv.push("commit".to_string());
    argv.push("-m".to_string());
    argv.push("chore: initial commit".to_string());
    argv.push("--".to_string());
    for rel in &written {
        argv.push(rel.clone());
    }
    let options = crate::platform::RunOptions {
        env,
        timeout: Some(std::time::Duration::from_secs(300)),
        ..crate::platform::RunOptions::default()
    };
    let result = crate::platform::run(&argv, &options);
    if result.code != Some(0) {
        return Err(format!("git commit failed: {}", result.stderr));
    }
    Ok(json!({ "root": root, "written": written, "skipped": skipped, "committed": true }))
}

pub fn gitattributes_content() -> String {
    include_str!("../../../assets/templates/gitattributes.txt").to_string()
}

pub fn editorconfig_content() -> String {
    include_str!("../../../assets/templates/editorconfig.txt").to_string()
}

pub fn readme_content() -> String {
    include_str!("../../../assets/templates/readme.md").to_string()
}

pub fn changelog_content() -> String {
    include_str!("../../../assets/templates/changelog.md").to_string()
}

pub fn gitignore_content() -> String {
    include_str!("../../../assets/templates/gitignore.txt").to_string()
}

pub fn license_content() -> String {
    include_str!("../../../assets/templates/license-mit.txt").to_string()
}

pub fn policy_content() -> String {
    "schema_version = 1\nmin_repowill_version = \"1.0.0\"\n".to_string()
}

pub fn project_list_path() -> PathBuf {
    if let Ok(home) = std::env::var("REPOWILL_HOME") {
        return PathBuf::from(home).join("projects.json");
    }
    if let Some(dirs) = directories::ProjectDirs::from("org", "AfterLeads", "RepoWill") {
        return dirs.data_dir().join("projects.json");
    }
    PathBuf::from("projects.json")
}

pub fn project_list() -> Result<Value, String> {
    let path = project_list_path();
    if !path.exists() {
        return Ok(json!({ "projects": [] }));
    }
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let value: Value = serde_json::from_str(&text).unwrap_or(json!({ "projects": [] }));
    Ok(value)
}

pub fn project_add(root: &Path) -> Result<Value, String> {
    let path = project_list_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut list = project_list().unwrap_or(json!({ "projects": [] }));
    let entry = json!({ "path": root.display().to_string() });
    let projects = list.get_mut("projects").and_then(|v| v.as_array_mut());
    if let Some(arr) = projects {
        if !arr.iter().any(|p| p.get("path") == Some(&entry["path"])) {
            arr.push(entry);
        }
    } else {
        list["projects"] = json!([entry]);
    }
    std::fs::write(&path, serde_json::to_string_pretty(&list).unwrap())
        .map_err(|e| e.to_string())?;
    Ok(list)
}

pub fn apply_update(
    root: &Path,
    identity_name: &str,
    identity_email: &str,
) -> Result<Value, String> {
    if !root.join(".git").exists() {
        return Err("not a git repository".to_string());
    }
    let detection = detect(root);
    let stacks: Vec<String> = detection
        .get("stacks")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let files = plan_files(root, &stacks);
    let mut updated = Vec::new();
    for (rel, content) in &files {
        let path = root.join(rel);
        let needs_write = !path.exists();
        if needs_write {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let _ = crate::platform::fs::backup_file(&path);
            crate::platform::fs::atomic_write(&path, content.as_bytes())
                .map_err(|e| e.to_string())?;
            updated.push(rel.clone());
        }
    }
    if updated.is_empty() {
        return Ok(json!({ "root": root, "updated": updated, "committed": false }));
    }
    let git = crate::git::Git::new()?;
    for rel in &updated {
        let result = git.run(&["add", "--", rel], Some(root))?;
        if result.code != Some(0) {
            return Err(format!("git add failed: {}", result.stderr));
        }
    }
    let mut commit_args: Vec<&str> = vec!["commit", "-m", "chore: update project scaffold", "--"];
    for rel in &updated {
        commit_args.push(rel.as_str());
    }
    let result = git.run(&commit_args, Some(root))?;
    if result.code != Some(0)
        && !result.stdout.contains("nothing to commit")
        && !result.stderr.contains("nothing to commit")
    {
        return Err(format!(
            "git commit failed: {} (stdout: {})",
            result.stderr, result.stdout
        ));
    }
    let committed = result.code == Some(0);
    let _ = (identity_name, identity_email);
    Ok(json!({ "root": root, "updated": updated, "committed": committed }))
}

pub fn unified_diff(old: &str, new: &str, path: &str) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let mut out = String::new();
    out.push_str(&format!("--- a/{path}\n"));
    out.push_str(&format!("+++ b/{path}\n"));
    // Compute LCS.
    let m = old_lines.len();
    let n = new_lines.len();
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in (0..m).rev() {
        for j in (0..n).rev() {
            if old_lines[i] == new_lines[j] {
                dp[i][j] = dp[i + 1][j + 1] + 1;
            } else {
                dp[i][j] = dp[i + 1][j].max(dp[i][j + 1]);
            }
        }
    }
    // Walk the LCS to produce hunks.
    let mut i = 0;
    let mut j = 0;
    type Hunk = (usize, usize, Vec<(char, String)>);
    let mut hunks: Vec<Hunk> = Vec::new();
    let mut current: Vec<(char, String)> = Vec::new();
    let mut old_start = 1;
    let mut new_start = 1;
    while i < m || j < n {
        if i < m && j < n && old_lines[i] == new_lines[j] {
            current.push((' ', old_lines[i].to_string()));
            i += 1;
            j += 1;
        } else if j < n && (i == m || dp[i][j + 1] >= dp[i + 1][j]) {
            if current.is_empty() {
                new_start = j + 1;
                old_start = i + 1;
            }
            current.push(('+', new_lines[j].to_string()));
            j += 1;
        } else if i < m {
            if current.is_empty() {
                new_start = j + 1;
                old_start = i + 1;
            }
            current.push(('-', old_lines[i].to_string()));
            i += 1;
        } else {
            break;
        }
    }
    if !current.is_empty() {
        hunks.push((old_start, new_start, current));
    }
    for (os, ns, lines) in &hunks {
        let old_count = lines.iter().filter(|(c, _)| *c != '+').count();
        let new_count = lines.iter().filter(|(c, _)| *c != '-').count();
        out.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            os, old_count, ns, new_count
        ));
        for (c, line) in lines {
            out.push_str(&format!("{c}{line}\n"));
        }
    }
    out
}

pub fn project_forget(root: &Path) -> Result<Value, String> {
    let path = project_list_path();
    if !path.exists() {
        return Ok(json!({ "projects": [] }));
    }
    let mut list = project_list().unwrap_or(json!({ "projects": [] }));
    if let Some(arr) = list.get_mut("projects").and_then(|v| v.as_array_mut()) {
        arr.retain(|p| {
            p.get("path").and_then(Value::as_str) != Some(root.to_string_lossy().as_ref())
        });
    }
    std::fs::write(&path, serde_json::to_string_pretty(&list).unwrap())
        .map_err(|e| e.to_string())?;
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn detect_finds_rust_stack() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
        let result = detect(dir.path());
        let stacks = result["stacks"].as_array().unwrap();
        assert!(stacks.iter().any(|s| s == "rust"));
    }

    #[test]
    fn detect_generic_when_empty() {
        let dir = tempdir().unwrap();
        let result = detect(dir.path());
        let stacks = result["stacks"].as_array().unwrap();
        assert!(stacks.iter().any(|s| s == "generic"));
    }

    #[test]
    fn plan_files_not_empty() {
        let dir = tempdir().unwrap();
        let files = plan_files(dir.path(), &[]);
        assert!(!files.is_empty());
        assert!(files.iter().any(|(p, _)| p == ".gitattributes"));
    }

    #[test]
    fn apply_init_requires_identity() {
        let dir = tempdir().unwrap();
        let result = apply_init(dir.path(), "", "");
        assert!(result.is_err());
    }

    #[test]
    fn apply_init_creates_repo_and_files() {
        let dir = tempdir().unwrap();
        let result = apply_init(dir.path(), "Test", "test@example.com").unwrap();
        assert!(dir.path().join(".git").exists());
        assert!(dir.path().join("README.md").exists());
        assert!(dir.path().join(".gitattributes").exists());
        assert_eq!(result["committed"], true);
    }

    #[test]
    fn apply_init_idempotent() {
        let dir = tempdir().unwrap();
        apply_init(dir.path(), "Test", "test@example.com").unwrap();
        // Re-running init on an existing repo should not commit on main.
        let result = apply_init(dir.path(), "Test", "test@example.com");
        assert!(result.is_err());
    }

    #[test]
    fn apply_update_adds_missing_files() {
        let dir = tempdir().unwrap();
        apply_init(dir.path(), "Test", "test@example.com").unwrap();
        // remove a file
        std::fs::remove_file(dir.path().join("README.md")).unwrap();
        let _result = apply_update(dir.path(), "Test", "test@example.com").unwrap();
        // File is restored (content matches scaffold).
        assert!(dir.path().join("README.md").exists());
    }
}
