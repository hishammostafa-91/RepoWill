// SPDX-License-Identifier: AGPL-3.0-or-later
#![allow(unsafe_code)]

//! Test helpers for RepoWill.

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_repo_builds() {
        let repo = TempRepo::new()
            .with_main()
            .commit("feat: initial", &[("README.md", "# demo\n")])
            .branch("feat/test");
        assert!(repo.path().join(".git").exists());
        assert!(repo.path().join("README.md").exists());
    }
}

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
        run_git(&self.path, &["config", "commit.gpgsign", "false"]);
        run_git(&self.path, &["config", "tag.gpgsign", "false"]);
        self
    }

    pub fn with_main_sha256(mut self) -> Self {
        run_git(
            &self.path,
            &["init", "-b", "main", "--object-format=sha256"],
        );
        run_git(&self.path, &["config", "user.name", "RepoWill Test"]);
        run_git(&self.path, &["config", "user.email", "test@example.com"]);
        run_git(&self.path, &["config", "core.autocrlf", "false"]);
        run_git(&self.path, &["config", "commit.gpgsign", "false"]);
        run_git(&self.path, &["config", "tag.gpgsign", "false"]);
        self
    }

    pub fn install_hooks(&self, repowill_binary: &Path) {
        let hooks = self.path.join(".git").join("hooks");
        std::fs::create_dir_all(&hooks).expect("hooks dir");
        let shim = format!(
            "#!/bin/sh\nexec \"{}\" hook dispatch \"$@\"\n",
            repowill_binary.display()
        );
        for name in [
            "pre-commit",
            "commit-msg",
            "post-commit",
            "pre-merge-commit",
            "pre-push",
            "pre-rebase",
            "post-checkout",
            "post-merge",
            "reference-transaction",
        ] {
            std::fs::write(hooks.join(name), &shim).expect("write hook");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut permissions = std::fs::metadata(hooks.join(name)).unwrap().permissions();
                permissions.set_mode(0o755);
                std::fs::set_permissions(hooks.join(name), permissions).unwrap();
            }
        }
    }

    pub fn install_hooks_auto(&self) {
        if let Ok(binary) = which::which("repowill") {
            self.install_hooks(&binary);
        } else if let Ok(binary) = std::env::var("CARGO_BIN_EXE_repowill") {
            self.install_hooks(Path::new(&binary));
        }
    }

    pub fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
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
        let env = Self { is_agent: true };
        env.apply();
        env
    }

    pub fn human() -> Self {
        let env = Self { is_agent: false };
        env.apply();
        env
    }

    pub fn apply(&self) {
        if self.is_agent {
            unsafe {
                std::env::set_var("REPOWILL_AGENT", "1");
            }
        } else {
            unsafe {
                std::env::remove_var("REPOWILL_AGENT");
            }
        }
    }
}
