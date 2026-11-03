// SPDX-License-Identifier: AGPL-3.0-or-later
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn version_prints() {
    let mut cmd = Command::cargo_bin("repowill").unwrap();
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("repowill 0.1.0"));
}

#[test]
fn help_mentions_repowill() {
    let mut cmd = Command::cargo_bin("repowill").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("RepoWill"));
}
