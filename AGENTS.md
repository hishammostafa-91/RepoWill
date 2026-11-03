# AGENTS.md — RepoWill

Guidance for AI coding agents and human contributors.

## What this project is
RepoWill is a local-first desktop GUI + CLI that sets up git repositories with a strict, safe workflow and enforces
the owner's rules (checks, dependency admission, protected files, delete prevention, AI-agent guardrails) for humans
and AI agents. Rust workspace (core + CLI) with a Tauri v2 + React + TypeScript GUI. License: AGPL-3.0-or-later;
files RepoWill generates into users' repositories: 0BSD.

## Build and test (identical on Windows and Linux)
- `cargo xtask check` — formatting, lints, architecture rules, dependency policy, frontend checks
- `cargo xtask test` — all tests · `cargo xtask verify` — everything (run before asking for review)
- `cargo xtask gui-dev` — run the desktop app in development mode
- Never add OS-specific code outside `crates/repowill-core/src/platform`. Use `cargo xtask`, not shell scripts.

## Layout
`crates/repowill-model` (types, schemas) · `crates/repowill-core` (all behaviour) · `crates/repowill-cli`
(`repowill` binary) · `crates/repowill-testkit` (test helpers) · `apps/gui` (Tauri app) · `assets/` (presets,
adapters, templates as data) · `docs/` (user docs; `docs/reference` is generated) · `xtask/` (dev automation).

## Rules (enforced on this repository by RepoWill itself)
1. Never commit to `main`. Create branches from the tip of `main`: `<type>/<short-slug>`.
2. Conventional Commits, small and atomic; sign off (`git commit -s`, DCO).
3. Changes reach `main` only through a reviewed, approved `--no-ff` merge.
4. Never delete files/branches/tags or rewrite history; request deletions through RepoWill.
5. Dependencies: only discussed and approved ones, latest stable (≥ 7 days old), exact pins, licenses compatible
   with AGPL-3.0-or-later.
6. Every change includes tests and documentation; `cargo xtask verify` must pass.
7. Verify every API/flag against official docs for the pinned version; never invent.
8. No secrets in the repository; no telemetry; no network calls outside documented features.
