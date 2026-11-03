# Architecture (dev)

Crate map (public developer documentation):

- `crates/repowill-model` — shared types and schemas.
- `crates/repowill-core` — all behaviour (Operation API, policy engine, guards, gates).
- `crates/repowill-cli` — `repowill` binary (CLI front-end over core).
- `crates/repowill-testkit` — test helpers and fixtures.
- `apps/gui` — Tauri v2 desktop app (front-end over core).
- `xtask` — dev automation (`check`, `test`, `gui-dev`, `verify`).
- `assets/` — presets, adapters, templates and `project.toml` (URLs live here only).