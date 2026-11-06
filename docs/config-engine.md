# Policy configuration engine

`crates/repowill-core/src/config` implements the policy engine described in
`plan/03-POLICY-AND-DATA.md`.

## Modules

- `loader.rs` - layered loading (built-in defaults, `extends`, project policy,
  local policy), id-aware merging, `[lock]` freezes, and local tighten-only
  enforcement.
- `validate.rs` - semantic validation with stable `RW-E2xxx` diagnostics,
  severity, rule, fix hints, and source positions.
- `strictness.rs` - weakening detection over effective policies.
- `editor.rs` - comment-preserving typed edits for policy TOML.
- `hash.rs` - canonical policy hashing and command trust hashing.
- `migrate.rs` - schema migration runner with backups and diffs.
- `value.rs` - JSON-shaped value trees and path resolution.

## Public entry points

- `load_policy_str`, `load_layers`, `load_policy_file`, `load_local`
- `merge_layer`, `merge_layer_checked`
- `validate`, `validate_with_source`, `validate_for_version`
- `weakening_changes`, `local_tighten_only`, `semantic_diff`
- `policy_hash`, `policy_hash_with_root`, `commands_hash`, `commands_hash_policy`
- `MigrationRunner`

## Engine version

`ENGINE_VERSION` is currently `1.0.0`. `min_repowill_version` is validated
against the running version through `validate_for_version`.
