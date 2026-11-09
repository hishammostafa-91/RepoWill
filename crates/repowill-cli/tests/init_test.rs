// SPDX-License-Identifier: AGPL-3.0-or-later
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn detect_runs_on_empty_dir() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("repowill")
        .unwrap()
        .arg("detect")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"stacks\""));
}

#[test]
fn init_creates_project() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("proj");
    Command::cargo_bin("repowill")
        .unwrap()
        .arg("init")
        .arg(&path)
        .arg("--name")
        .arg("Test")
        .arg("--email")
        .arg("test@example.com")
        .assert()
        .success();
    assert!(path.join("README.md").exists());
    assert!(path.join(".git").exists());
}

#[test]
fn init_dry_run_does_not_write() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("proj");
    Command::cargo_bin("repowill")
        .unwrap()
        .arg("init")
        .arg(&path)
        .arg("--dry-run")
        .assert()
        .success();
    assert!(!path.exists() || !path.join("README.md").exists());
}

#[test]
fn init_requires_identity() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("proj");
    Command::cargo_bin("repowill")
        .unwrap()
        .arg("init")
        .arg(&path)
        .arg("--non-interactive")
        .assert()
        .failure()
        .code(2);
}
