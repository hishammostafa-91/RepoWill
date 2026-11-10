// SPDX-License-Identifier: AGPL-3.0-or-later
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

// AT-003: detection on empty directory
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
fn detect_finds_rust_stack() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
    Command::cargo_bin("repowill")
        .unwrap()
        .arg("detect")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"rust\""));
}

// AT-001: new repository creation via wizard/flags
// AT-001: new repository creation
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
    assert!(path.join(".gitattributes").exists());
    assert!(path.join(".gitignore").exists());
    assert!(path.join("CHANGELOG.md").exists());
}

// AT-002: dry-run shows plan without writing
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
    assert!(!path.join("README.md").exists());
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

#[test]
fn init_idempotent() {
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
    // Re-running should fail (already has commits).
    Command::cargo_bin("repowill")
        .unwrap()
        .arg("init")
        .arg(&path)
        .arg("--name")
        .arg("Test")
        .arg("--email")
        .arg("test@example.com")
        .assert()
        .failure();
}

// AT-007: adopt existing repository
#[test]
fn adopt_on_existing_repo() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("proj");
    // Create a git repo first.
    std::process::Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .arg(&path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .arg("-C")
        .arg(&path)
        .args(["config", "user.name", "Test"])
        .output()
        .unwrap();
    std::process::Command::new("git")
        .arg("-C")
        .arg(&path)
        .args(["config", "user.email", "test@example.com"])
        .output()
        .unwrap();
    fs::write(path.join("hello.txt"), "hello").unwrap();
    std::process::Command::new("git")
        .arg("-C")
        .arg(&path)
        .args(["add", "-A"])
        .output()
        .unwrap();
    std::process::Command::new("git")
        .arg("-C")
        .arg(&path)
        .args(["commit", "-m", "init"])
        .output()
        .unwrap();

    Command::cargo_bin("repowill")
        .unwrap()
        .arg("adopt")
        .arg(&path)
        .arg("--name")
        .arg("Test")
        .arg("--email")
        .arg("test@example.com")
        .assert()
        .success();
    assert!(path.join("README.md").exists());
}

#[test]
fn adopt_requires_git() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("nogit");
    fs::create_dir_all(&path).unwrap();
    Command::cargo_bin("repowill")
        .unwrap()
        .arg("adopt")
        .arg(&path)
        .arg("--name")
        .arg("Test")
        .arg("--email")
        .arg("test@example.com")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn detect_json_output() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
    Command::cargo_bin("repowill")
        .unwrap()
        .arg("detect")
        .arg(dir.path())
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"rust\""));
}

// AT-008: interrupted init + resume
#[test]
fn update_on_fresh_project() {
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
    // Remove a file and update should restore it.
    fs::remove_file(path.join("README.md")).unwrap();
    Command::cargo_bin("repowill")
        .unwrap()
        .arg("update")
        .arg(&path)
        .assert()
        .success();
    assert!(path.join("README.md").exists());
}

// AT-009: answers file init
#[test]
fn answers_file_init() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("proj");
    let answers = dir.path().join("answers.toml");
    let content = format!(
        "path = \"{}\"\nname = \"AnsTest\"\nmode = \"new\"\nidentity = {{ name = \"AnsTest\", email = \"ans@test.com\" }}\n",
        path.display().to_string().replace('\\', "/")
    );
    fs::write(&answers, content).unwrap();
    Command::cargo_bin("repowill")
        .unwrap()
        .arg("init")
        .arg("--answers")
        .arg(&answers)
        .assert()
        .success();
    assert!(path.join("README.md").exists());
}
