## [Unreleased]

### Added
- Repository init/adopt/update with scaffold, atomic writes, backups, checkpoint resume, token-guarded commit.
- Stack-aware .gitignore templates (Node, Rust, Python, Go, .NET, Java).
- 0BSD SPDX header templates.
- JSON envelope for all CLI commands.

# Changelog

All notable changes to RepoWill are documented here.

## [Unreleased]

### Added
- WP-01.3 policy configuration engine in `crates/repowill-core/src/config`:
  layered loading and locking, semantic validation, weakening detection,
  policy and command trust hashing, typed TOML editing, and schema migrations.
- `docs/config-engine.md` reference documentation.
- WP-02.3 repository initialisation and adoption: `project.detect`, `project.plan`, `project.init`,
  `project.adopt`, `project.update`, projects registry; interactive CLI wizard and answers file;
  scaffold templates (gitattributes, gitignore, editorconfig, README, LICENSE, CHANGELOG, policy);
  GUI setup wizard and projects screen.
- WP-02.3.1 follow-up: stack-aware gitignores, real unified diff with @@ hunks, atomic writes with
  backups, checkpoint-based resume, token-guarded initial commit, JSON envelope for init/adopt/projects,
  comprehensive CLI integration tests, user documentation.

### Changed
- `repowill-model::Policy` now derives `PartialEq` to support engine tests.
- Built-in policy defaults corrected for `DocCoupling`, `Changelog`,
  `RemoteGithub`, and `GatePaths`.
