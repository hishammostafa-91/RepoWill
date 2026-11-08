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
    if exists("*.csproj") || exists("*.sln") {
        // glob not checked here; placeholder
    }
    // .NET detection
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if (name.ends_with(".csproj") || name.ends_with(".sln")) && !stacks.contains(&"dotnet")
            {
                stacks.push("dotnet");
            }
        }
    }
    // docs-only: no code markers
    if stacks.is_empty() {
        stacks.push("generic");
    }

    // hook managers
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

    // agent configs
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

    // existing files of interest
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

    // is git repo?
    let is_git = root.join(".git").exists();

    json!({
        "root": root,
        "is_git": is_git,
        "stacks": stacks,
        "hook_managers": hook_managers,
        "agent_configs": agent_configs,
        "existing_files": existing_files
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

pub fn plan_files(root: &Path, _stacks: &[String]) -> Vec<(String, String)> {
    // Returns list of (relative_path, content) that would be created.
    let _ = root;
    vec![
        (".gitattributes".to_string(), gitattributes_content()),
        (".editorconfig".to_string(), editorconfig_content()),
        ("README.md".to_string(), readme_content()),
        ("CHANGELOG.md".to_string(), changelog_content()),
        (
            ".repowill/policy.toml".to_string(),
            String::from("# placeholder policy\nschema_version = 1\n"),
        ),
    ]
}

pub fn apply_init(root: &Path, identity_name: &str, identity_email: &str) -> Result<Value, String> {
    // Create dir if needed
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    // git init if not a repo
    let git_dir = root.join(".git");
    if !git_dir.exists() {
        let git = crate::git::Git::new()?;
        let result = git.run(&["init", "-b", "main"], Some(root))?;
        if result.code != Some(0) {
            return Err(format!("git init failed: {}", result.stderr));
        }
    }
    // Write files
    let files = plan_files(root, &[]);
    for (rel, content) in &files {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, content).map_err(|e| e.to_string())?;
    }
    // git config identity
    let git = crate::git::Git::new()?;
    let _ = git.run(&["config", "user.name", identity_name], Some(root));
    let _ = git.run(&["config", "user.email", identity_email], Some(root));
    // core.longpaths on Windows
    #[cfg(windows)]
    {
        let _ = git.run(&["config", "core.longpaths", "true"], Some(root));
    }
    // initial commit with token
    let _ = git.run(&["add", "-A"], Some(root));
    let _ = git.run(&["commit", "-m", "chore: initial commit"], Some(root));
    Ok(json!({ "root": root, "files": files.len() }))
}

pub fn gitattributes_content() -> String {
    "* text=auto eol=lf\n".to_string()
}

pub fn editorconfig_content() -> String {
    "root = true\n\n[*]\nend_of_line = lf\ninsert_final_newline = true\ncharset = utf-8\n"
        .to_string()
}

pub fn readme_content() -> String {
    "# Project\n\n".to_string()
}

pub fn changelog_content() -> String {
    "# Changelog\n\nAll notable changes to this project will be documented in this file.\n"
        .to_string()
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
    let projects = list.get_mut("projects").and_then(|v| v.as_array_mut());
    let entry = json!({ "path": root.display().to_string() });
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
