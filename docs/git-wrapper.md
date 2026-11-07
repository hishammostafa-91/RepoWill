# Git wrapper

`crates/repowill-core/src/git.rs` provides the typed wrapper over the `git` CLI.

## Features

- Version check (requires git 2.40 or newer).
- Environment isolation for deterministic output.
- `-z` parsing helpers for NUL-delimited git output.
- OID validation and zero-OID handling.
- Ref-name validation.
- Discovery, common-dir, hooks-path, refs, rev-list, merge-base, ancestry, name-status, numstat, blob/tree, config, worktree, and trailer helpers.

## Testkit

`crates/repowill-testkit` provides `TempRepo`, including `with_main`, `with_main_sha256`, `commit`, `branch`, and `install_hooks`.
