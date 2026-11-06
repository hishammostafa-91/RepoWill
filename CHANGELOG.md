# Changelog

All notable changes to RepoWill are documented here.

## [Unreleased]

### Added
- WP-01.3 policy configuration engine in `crates/repowill-core/src/config`:
  layered loading and locking, semantic validation, weakening detection,
  policy and command trust hashing, typed TOML editing, and schema migrations.
- `docs/config-engine.md` reference documentation.

### Changed
- `repowill-model::Policy` now derives `PartialEq` to support engine tests.
- Built-in policy defaults corrected for `DocCoupling`, `Changelog`,
  `RemoteGithub`, and `GatePaths`.
