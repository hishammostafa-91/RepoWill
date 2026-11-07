// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

//! Typed wrapper around the `git` executable.

use crate::platform::{RunOptions, RunResult, run};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const MIN_GIT_MAJOR: u32 = 2;
pub const MIN_GIT_MINOR: u32 = 40;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub raw: String,
}

#[derive(Debug, Clone)]
pub struct Git {
    pub executable: PathBuf,
    pub env: Vec<(String, String)>,
}

impl Git {
    pub fn new() -> Result<Self, String> {
        let version = git_version()?;
        if (version.major, version.minor) < (MIN_GIT_MAJOR, MIN_GIT_MINOR) {
            return Err(format!(
                "git {} is too old; RepoWill requires git {}.{} or newer",
                version.raw, MIN_GIT_MAJOR, MIN_GIT_MINOR
            ));
        }
        let executable = crate::platform::resolve_bin("git")
            .ok_or_else(|| "git executable not found on PATH".to_string())?;
        Ok(Self {
            executable,
            env: isolated_env(),
        })
    }

    pub fn run(&self, args: &[&str], cwd: Option<&Path>) -> Result<RunResult, String> {
        let mut argv = vec![self.executable.to_string_lossy().into_owned()];
        if let Some(cwd) = cwd {
            argv.push("-C".to_string());
            argv.push(cwd.to_string_lossy().into_owned());
        }
        argv.extend(args.iter().map(|arg| (*arg).to_string()));
        let options = RunOptions {
            env: self.env.clone(),
            timeout: Some(Duration::from_secs(300)),
            ..RunOptions::default()
        };
        Ok(run(&argv, &options))
    }
}

pub fn git_version() -> Result<GitVersion, String> {
    let options = RunOptions::default();
    let argv = vec!["git".to_string(), "--version".to_string()];
    let result = run(&argv, &options);
    if result.code != Some(0) {
        return Err(format!("git --version failed: {}", result.stderr));
    }
    parse_git_version(&result.stdout)
}

fn parse_git_version(text: &str) -> Result<GitVersion, String> {
    let raw = text.trim().to_string();
    let version_text = raw
        .split_whitespace()
        .nth(2)
        .ok_or_else(|| format!("unexpected git version output: {raw}"))?;
    let mut parts = version_text.split('.');
    let major = parts
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .ok_or_else(|| format!("cannot parse git major version from {raw}"))?;
    let minor = parts
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .ok_or_else(|| format!("cannot parse git minor version from {raw}"))?;
    let patch = parts
        .next()
        .and_then(|part| {
            part.chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>()
                .parse::<u32>()
                .ok()
        })
        .unwrap_or(0);
    Ok(GitVersion {
        major,
        minor,
        patch,
        raw,
    })
}

fn isolated_env() -> Vec<(String, String)> {
    vec![
        ("GIT_TERMINAL_PROMPT".to_string(), "0".to_string()),
        ("GIT_PAGER".to_string(), "cat".to_string()),
        ("PAGER".to_string(), "cat".to_string()),
        ("LC_ALL".to_string(), "C".to_string()),
        ("LANG".to_string(), "C".to_string()),
    ]
}

pub fn discover_top_level(start: &Path) -> Result<PathBuf, String> {
    let git = Git::new()?;
    let result = git.run(&["rev-parse", "--show-toplevel"], Some(start))?;
    if result.code != Some(0) {
        return Err(result.stderr.trim().to_string());
    }
    Ok(PathBuf::from(result.stdout.trim()))
}

pub fn common_dir(start: &Path) -> Result<PathBuf, String> {
    let git = Git::new()?;
    let result = git.run(&["rev-parse", "--git-common-dir"], Some(start))?;
    if result.code != Some(0) {
        return Err(result.stderr.trim().to_string());
    }
    Ok(PathBuf::from(result.stdout.trim()))
}

pub fn hooks_path(start: &Path) -> Result<PathBuf, String> {
    let git = Git::new()?;
    let result = git.run(&["rev-parse", "--git-path", "hooks"], Some(start))?;
    if result.code != Some(0) {
        return Err(result.stderr.trim().to_string());
    }
    Ok(PathBuf::from(result.stdout.trim()))
}

pub fn split_nul(text: &str) -> Vec<String> {
    text.split('\0')
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn is_zero_oid(oid: &str) -> bool {
    !oid.is_empty() && oid.chars().all(|character| character == '0')
}

pub fn validate_oid(oid: &str) -> bool {
    (oid.len() == 40 || oid.len() == 64)
        && oid.chars().all(|character| character.is_ascii_hexdigit())
}

pub fn validate_ref_name(name: &str) -> bool {
    if name.is_empty() || name.starts_with('/') || name.ends_with('/') || name.contains("..") {
        return false;
    }
    if name.chars().any(|character| {
        character.is_control()
            || character == ' '
            || character == '~'
            || character == '^'
            || character == ':'
    }) {
        return false;
    }
    if name.contains("@{") || name == "@" || name.contains('\\') {
        return false;
    }
    true
}

pub fn rev_parse(git: &Git, repo: &Path, rev: &str) -> Result<String, String> {
    let result = git.run(&["rev-parse", rev], Some(repo))?;
    if result.code != Some(0) {
        return Err(result.stderr.trim().to_string());
    }
    Ok(result.stdout.trim().to_string())
}

pub fn merge_base(git: &Git, repo: &Path, left: &str, right: &str) -> Result<String, String> {
    let result = git.run(&["merge-base", left, right], Some(repo))?;
    if result.code != Some(0) {
        return Err(result.stderr.trim().to_string());
    }
    Ok(result.stdout.trim().to_string())
}

pub fn is_ancestor(
    git: &Git,
    repo: &Path,
    ancestor: &str,
    descendant: &str,
) -> Result<bool, String> {
    let result = git.run(
        &["merge-base", "--is-ancestor", ancestor, descendant],
        Some(repo),
    )?;
    match result.code {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(result.stderr.trim().to_string()),
    }
}

pub fn show_blob(git: &Git, repo: &Path, spec: &str) -> Result<String, String> {
    let result = git.run(&["show", spec], Some(repo))?;
    if result.code != Some(0) {
        return Err(result.stderr.trim().to_string());
    }
    Ok(result.stdout)
}

pub fn write_tree(git: &Git, repo: &Path) -> Result<String, String> {
    let result = git.run(&["write-tree"], Some(repo))?;
    if result.code != Some(0) {
        return Err(result.stderr.trim().to_string());
    }
    Ok(result.stdout.trim().to_string())
}

pub fn config_get(git: &Git, repo: &Path, key: &str) -> Result<Option<String>, String> {
    let result = git.run(&["config", "--get", key], Some(repo))?;
    match result.code {
        Some(0) => Ok(Some(result.stdout.trim().to_string())),
        Some(1) => Ok(None),
        _ => Err(result.stderr.trim().to_string()),
    }
}

pub fn config_set(git: &Git, repo: &Path, key: &str, value: &str) -> Result<(), String> {
    let result = git.run(&["config", key, value], Some(repo))?;
    if result.code != Some(0) {
        return Err(result.stderr.trim().to_string());
    }
    Ok(())
}

pub fn worktrees(git: &Git, repo: &Path) -> Result<Vec<PathBuf>, String> {
    let result = git.run(&["worktree", "list", "--porcelain"], Some(repo))?;
    if result.code != Some(0) {
        return Err(result.stderr.trim().to_string());
    }
    let mut out = Vec::new();
    for line in result.stdout.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            out.push(PathBuf::from(path));
        }
    }
    Ok(out)
}

pub fn commit_trailers(git: &Git, repo: &Path, rev: &str) -> Result<Vec<(String, String)>, String> {
    let result = git.run(&["show", "-s", "--format=%B", rev], Some(repo))?;
    if result.code != Some(0) {
        return Err(result.stderr.trim().to_string());
    }
    let mut trailers = Vec::new();
    for line in result.stdout.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            if !key.is_empty()
                && key
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
                && !value.is_empty()
            {
                trailers.push((key.to_string(), value.to_string()));
            }
        }
    }
    Ok(trailers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_git_version() {
        let version = parse_git_version("git version 2.43.0.windows.1").unwrap();
        assert_eq!((version.major, version.minor, version.patch), (2, 43, 0));
    }

    #[test]
    fn rejects_old_versions() {
        let version = parse_git_version("git version 2.39.1").unwrap();
        assert!((version.major, version.minor) < (MIN_GIT_MAJOR, MIN_GIT_MINOR));
    }

    #[test]
    fn splits_nul_output() {
        assert_eq!(split_nul("a\0b\0\0c"), vec!["a", "b", "c"]);
    }

    #[test]
    fn validates_oids() {
        assert!(validate_oid("0123456789abcdef0123456789abcdef01234567"));
        assert!(!validate_oid("not-an-oid"));
        assert!(is_zero_oid("0000000000000000000000000000000000000000"));
    }

    #[test]
    fn validates_ref_names() {
        assert!(validate_ref_name("feat/login"));
        assert!(!validate_ref_name("bad name"));
        assert!(!validate_ref_name("a..b"));
    }
}
