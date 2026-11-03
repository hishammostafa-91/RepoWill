# VERSIONS — pinned versions (WP-00.1, T-00.1.4)

> Note: Rust toolchain pinned to installed 1.94.0 per owner decision on 2026-11-03 (resolved latest stable was 1.99.1). Do not update without owner instruction.

Resolved 2026-11-03 per `12` §7: latest **stable** release, released **≥ 7 days ago**
(cutoff: 2026-10-26/27; releases on 2026-10-27 after 00:00 UTC were treated as too new).
Sources: crates.io API (`max_stable_version` + `/versions`), npm registry (`dist-tags.latest` +
`time`), rust-lang.org channel manifest, nodejs.org dist index. Licenses to be enforced by
`cargo deny` (WP-00.3); all listed crates/packages are from the pre-approved list (`12` §8).

Checked date for every row: **2026-11-03**. "Source" = canonical page used.

## Toolchains and runtimes

| Name | Version | Released | Source |
|---|---|---|---|
| Rust (stable, x86_64-pc-windows-msvc) | 1.99.1 | 2026-10-13 (channel 2026-10-15) | https://static.rust-lang.org/dist/channel-rust-stable.toml |
| Node.js (LTS "Lithium") | 26.12.1 | 2026-10-21 | https://nodejs.org/dist/index.json |
| pnpm | 12.16.0 | 2026-10-26 | https://registry.npmjs.org/pnpm |

Notes:
- Installed on this machine: rustc/cargo 1.94.0 (2026-03-02), node v26.13.0 (2026-10-28, inside
  the 7-day cooldown), pnpm 12.16.1 (2026-10-27, inside the cooldown). Owner should consider
  `rustup update stable` before WP-00.3; agents do not install system software.
- `rust-toolchain.toml` (WP-00.3) will pin the resolved stable.

## Rust crates (§8.1)

| Crate | Version | Released | Source |
|---|---|---|---|
| clap | 4.6.7 | 2026-09-14 | https://crates.io/crates/clap |
| clap_complete | 4.6.11 | 2026-09-15 | https://crates.io/crates/clap_complete |
| serde | 1.0.229 | 2026-07-18 | https://crates.io/crates/serde |
| serde_json | 1.0.154 | 2026-10-11 | https://crates.io/crates/serde_json |
| toml | 1.1.8 | 2026-10-09 | https://crates.io/crates/toml |
| toml_edit | 0.25.17 | 2026-10-09 | https://crates.io/crates/toml_edit |
| schemars | 1.2.3 | 2026-10-12 | https://crates.io/crates/schemars |
| ts-rs | 12.0.1 | 2026-01-31 | https://crates.io/crates/ts-rs |
| thiserror | 2.0.21 | 2026-09-23 | https://crates.io/crates/thiserror |
| anyhow | 1.0.104 | 2026-07-18 | https://crates.io/crates/anyhow |
| tracing | 0.1.44 | 2025-12-18 | https://crates.io/crates/tracing |
| tracing-subscriber | 0.3.23 | 2026-03-13 | https://crates.io/crates/tracing-subscriber |
| tracing-appender | 0.2.5 | 2026-04-17 | https://crates.io/crates/tracing-appender |
| directories | 6.0.0 | 2025-01-12 | https://crates.io/crates/directories |
| camino | 1.2.6 | 2026-09-15 | https://crates.io/crates/camino |
| dunce | 1.0.5 | 2024-08-04 | https://crates.io/crates/dunce |
| fd-lock | 4.0.4 | 2025-03-10 | https://crates.io/crates/fd-lock |
| tempfile | 3.27.0 | 2026-03-11 | https://crates.io/crates/tempfile |
| trash | 5.2.9 | 2026-09-13 | https://crates.io/crates/trash |
| globset | 0.4.20 | 2026-08-04 | https://crates.io/crates/globset (0.4.21 released 2026-10-27, in cooldown) |
| ignore | 0.4.33 | 2026-08-04 | https://crates.io/crates/ignore (0.4.34 released 2026-10-27, in cooldown) |
| which | 8.0.6 | 2026-08-26 | https://crates.io/crates/which |
| process-wrap | 10.0.1 | 2026-09-23 | https://crates.io/crates/process-wrap |
| shlex | 2.0.1 | 2026-05-17 | https://crates.io/crates/shlex |
| ureq | 3.5.0 | 2026-10-21 | https://crates.io/crates/ureq (3.5.1 released 2026-11-02, in cooldown) |
| rustls-platform-verifier | 0.7.1 | 2026-09-24 | https://crates.io/crates/rustls-platform-verifier |
| sha2 | 0.11.1 | 2026-10-12 | https://crates.io/crates/sha2 |
| hmac | 0.13.0 | 2026-03-29 | https://crates.io/crates/hmac |
| getrandom | 0.4.3 | 2026-06-17 | https://crates.io/crates/getrandom |
| hex | 0.4.3 | 2021-03-03 | https://crates.io/crates/hex |
| base64 | 0.23.1 | 2026-08-04 | https://crates.io/crates/base64 |
| uuid | 1.29.0 | 2026-10-23 | https://crates.io/crates/uuid |
| time | 0.3.55 | 2026-08-01 | https://crates.io/crates/time |
| semver | 1.0.28 | 2026-04-04 | https://crates.io/crates/semver |
| pep440_rs | 0.7.3 | 2024-12-04 | https://crates.io/crates/pep440_rs |
| quick-xml | 0.42.0 | 2026-08-22 | https://crates.io/crates/quick-xml |
| pulldown-cmark | 0.13.5 | 2026-10-21 | https://crates.io/crates/pulldown-cmark (0.13.6 released 2026-10-30, in cooldown) |
| spdx | 0.13.6 | 2026-10-02 | https://crates.io/crates/spdx |
| regex | 1.13.1 | 2026-07-15 | https://crates.io/crates/regex |
| saphyr | 0.3.1 | 2026-10-26 | https://crates.io/crates/saphyr (chosen over yaml-rust2; 0.3.2/0.3.3 released 2026-10-27, in cooldown) |
| keyring | 4.2.0 | 2026-08-29 | https://crates.io/crates/keyring |
| zip | 9.1.2 | 2026-10-26 | https://crates.io/crates/zip (9.1.3+ released 2026-10-27..11-03, in cooldown) |
| flate2 | 1.1.10 | 2026-08-28 | https://crates.io/crates/flate2 |
| tar | 0.4.46 | 2026-05-18 | https://crates.io/crates/tar |
| similar | 3.2.1 | 2026-10-24 | https://crates.io/crates/similar |
| dialoguer | 0.12.0 | 2025-08-23 | https://crates.io/crates/dialoguer |
| console | 0.16.6 | 2026-09-10 | https://crates.io/crates/console |
| indicatif | 0.18.6 | 2026-07-01 | https://crates.io/crates/indicatif |
| comfy-table | 8.0.1 | 2026-09-25 | https://crates.io/crates/comfy-table |
| tauri | 2.12.5 | 2026-10-21 | https://crates.io/crates/tauri |
| tauri-plugin-dialog | 2.8.3 | 2026-10-14 | https://crates.io/crates/tauri-plugin-dialog (2.8.4 released 2026-10-28, in cooldown) |
| tauri-plugin-notification | 2.7.0 | 2026-10-14 | https://crates.io/crates/tauri-plugin-notification (2.8.0 released 2026-10-28, in cooldown) |
| tauri-plugin-opener | 2.7.2 | 2026-10-14 | https://crates.io/crates/tauri-plugin-opener |
| tauri-plugin-window-state | 2.5.0 | 2026-09-26 | https://crates.io/crates/tauri-plugin-window-state |
| tauri-plugin-single-instance | 2.5.4 | 2026-10-14 | https://crates.io/crates/tauri-plugin-single-instance (2.5.5 released 2026-10-28, in cooldown) |
| assert_cmd (dev) | 2.2.2 | 2026-05-11 | https://crates.io/crates/assert_cmd |
| predicates (dev) | 3.1.4 | 2026-02-11 | https://crates.io/crates/predicates |
| insta (dev) | 1.49.0 | 2026-10-03 | https://crates.io/crates/insta |
| proptest (dev) | 1.12.0 | 2026-10-10 | https://crates.io/crates/proptest |
| httpmock (dev) | 0.9.0 | 2026-10-20 | https://crates.io/crates/httpmock |
| pretty_assertions (dev) | 1.4.1 | 2024-09-15 | https://crates.io/crates/pretty_assertions |

## Frontend packages (§8.2)

| Package | Version | Released | Source |
|---|---|---|---|
| react | 19.3.1 | 2026-10-14 | https://registry.npmjs.org/react |
| react-dom | 19.3.1 | 2026-10-14 | https://registry.npmjs.org/react-dom |
| typescript | 7.0.2 | 2026-07-08 | https://registry.npmjs.org/typescript (see note) |
| vite | 8.3.5 | 2026-10-21 | https://registry.npmjs.org/vite |
| @vitejs/plugin-react | 6.1.2 | 2026-10-05 | https://registry.npmjs.org/@vitejs/plugin-react (6.2.1 released 2026-11-02, in cooldown) |
| @tauri-apps/api | 2.12.4 | 2026-10-14 | https://registry.npmjs.org/@tauri-apps/api |
| @tauri-apps/cli | 2.12.4 | 2026-10-21 | https://registry.npmjs.org/@tauri-apps/cli |
| @tauri-apps/plugin-dialog | 2.8.3 | 2026-10-14 | https://registry.npmjs.org/@tauri-apps/plugin-dialog (2.8.4 in cooldown) |
| @tauri-apps/plugin-notification | 2.7.0 | 2026-10-14 | https://registry.npmjs.org/@tauri-apps/plugin-notification (2.8.0 in cooldown) |
| @tauri-apps/plugin-opener | 2.7.2 | 2026-10-14 | https://registry.npmjs.org/@tauri-apps/plugin-opener |
| @tauri-apps/plugin-window-state | 2.5.0 | 2026-09-26 | https://registry.npmjs.org/@tauri-apps/plugin-window-state |
| @mantine/core | 9.7.4 | 2026-10-25 | https://registry.npmjs.org/@mantine/core |
| @mantine/hooks | 9.7.4 | 2026-10-25 | https://registry.npmjs.org/@mantine/hooks |
| @mantine/form | 9.7.4 | 2026-10-25 | https://registry.npmjs.org/@mantine/form |
| @mantine/notifications | 9.7.4 | 2026-10-25 | https://registry.npmjs.org/@mantine/notifications |
| @tabler/icons-react | 3.49.0 | 2026-10-05 | https://registry.npmjs.org/@tabler/icons-react (3.50.0 released 2026-10-27, in cooldown) |
| react-router | 8.5.0 | 2026-10-19 | https://registry.npmjs.org/react-router (8.5.1 released 2026-10-27, in cooldown) |
| @tanstack/react-query | 5.105.0 | 2026-10-26 | https://registry.npmjs.org/@tanstack/react-query (5.105.3 released 2026-11-02, in cooldown) |
| i18next | 26.5.11 | 2026-10-25 | https://registry.npmjs.org/i18next (26.7.1 released 2026-11-01, in cooldown) |
| react-i18next | 17.1.9 | 2026-10-25 | https://registry.npmjs.org/react-i18next (17.1.12 released 2026-11-03, in cooldown) |
| @uiw/react-codemirror | 4.25.12 | 2026-09-24 | https://registry.npmjs.org/@uiw/react-codemirror |
| @codemirror/legacy-modes | 6.5.5 | 2026-10-07 | https://registry.npmjs.org/@codemirror/legacy-modes |
| @biomejs/biome | 2.6.3 | 2026-10-24 | https://registry.npmjs.org/@biomejs/biome (2.6.4 released 2026-10-28, in cooldown) |
| vitest | 4.1.12 | 2026-10-15 | https://registry.npmjs.org/vitest (5.1.0 released 2026-11-02, in cooldown) |
| @testing-library/react | 16.3.3 | 2026-08-27 | https://registry.npmjs.org/@testing-library/react |
| @testing-library/user-event | 14.6.8 | 2026-10-13 | https://registry.npmjs.org/@testing-library/user-event |
| happy-dom | 21.0.4 | 2026-10-20 | https://registry.npmjs.org/happy-dom |
| @wdio/cli | 10.1.0 | 2026-10-18 | https://registry.npmjs.org/@wdio/cli |
| axe-core | 4.14.0 | 2026-10-05 | https://registry.npmjs.org/axe-core |

Notes:
- TypeScript 7.0.2 is the current `latest` stable on npm (2026-07-08). If any pinned tool
  (Vite, Biome, template) does not yet support TS 7, fall back to the newest TS 5.x/6.x at
  scaffold time and record it here + in the WP notes; do not silently downgrade.
- WebdriverIO E2E stack (`@wdio/*` runner packages) will be resolved together with `@wdio/cli`
  at WP-00.3/WP-11 time; only `@wdio/cli` is pinned here.
- Tauri v2 = crate `tauri` 2.12.5 + npm `@tauri-apps/cli` 2.12.4 + `@tauri-apps/api` 2.12.4.

## Cooldown rule applied

Today 2026-11-03; "≥ 7 days old" ⇒ released on or before 2026-10-26 (releases on 2026-10-27
after 00:00 UTC were conservatively excluded). Anything newer was skipped in favour of the
newest qualifying stable; the skipped versions are noted inline.
