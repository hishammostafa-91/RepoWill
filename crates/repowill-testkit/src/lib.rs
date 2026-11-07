// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

//! Test helpers for RepoWill.

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

pub struct TempRepo {
    _dir: TempDir,
    path: PathBuf,
}

impl TempRepo {
    pub fn new() -> Self {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().to_path_buf();
        Self { _dir: dir, path }
    }

    pub fn with_main(mut self) -> Self {
        run_git(&self.path, &["init", "-b", "main"]);
        run_git(&self.path, &["config", "user.name", "RepoWill Test"]);
        run_git(&self.path, &["config", "user.email", "test@example.com"]);
        run_git(&self.path, &["config", "core.autocrlf", "false"]);
        self
    }

    pub fn commit(mut self, subject: &str, files: &[(&str, &str)]) -> Self {
        for (path, contents) in files {
            let full = self.path.join(path);
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent).expect("create parent");
            }
            std::fs::write(&full, contents).expect("write file");
        }
        run_git(&self.path, &["add", "-A"]);
        run_git(&self.path, &["commit", "-m", subject]);
        self
    }

    pub fn branch(mut self, name: &str) -> Self {
        run_git(&self.path, &["checkout", "-b", name]);
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn run_git(cwd: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .status()
        .expect("git");
    assert!(status.success(), "git {:?} failed", args);
}

#[derive(Debug, Clone)]
pub struct FakeClock {
    pub now: std::time::SystemTime,
}

impl FakeClock {
    pub fn new(now: std::time::SystemTime) -> Self {
        Self { now }
    }

    pub fn advance(&mut self, duration: std::time::Duration) {
        self.now += duration;
    }
}

#[derive(Debug, Clone)]
pub struct FakeTty {
    pub stdin_tty: bool,
    pub stdout_tty: bool,
}

impl FakeTty {
    pub fn interactive() -> Self {
        Self {
            stdin_tty: true,
            stdout_tty: true,
        }
    }

    pub fn non_interactive() -> Self {
        Self {
            stdin_tty: false,
            stdout_tty: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentEnv {
    pub is_agent: bool,
}

impl AgentEnv {
    pub fn agent() -> Self {
        Self { is_agent: true }
    }

    pub fn human() -> Self {
        Self { is_agent: false }
    }
}
