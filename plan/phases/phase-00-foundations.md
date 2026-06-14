# Phase 0 — Foundations & Scaffolding

**Goal:** The chassis. A Cargo workspace + Tauri v2 + React/Vite skeleton that opens a window on
Ubuntu, has the empty crate/module structure from [`04` §10](../04-engineering-architecture-and-patterns.md),
and builds a `.deb`. No features.

**Delivers:** none directly; unblocks all. **Architecture:** establishes the crate boundaries and the
CI dependency-direction guard that protect §1 of `04`.

## Scope
**In:** workspace + crates (`core`,`store`,`pty`,`app`,`mcp`,`httpapi`,`cli`,`ipc`); Tauri app + Vite/
React/TS UI shell; tooling (rustfmt/clippy/ESLint/tsc); CI on `ubuntu-22.04`; one `.deb` build; the
**dependency-direction lint**. **Out:** all behavior (Phases 1+).

## Prerequisites
- Decisions D1–D6 (confirmed). Build host Ubuntu 22.04 with `webkit2gtk-4.1`, `libgtk-3-dev`,
  `librsvg2-dev`, `libayatana-appindicator3-dev`, `build-essential`, Rust stable, Node 20+, pnpm.

## Tasks
1. **Workspace + crates** exactly per `04` §10. Each `core` context module (`config`,`projects`,
   `trust`,`supervisor`,`terminal`,`agents`,`idle`,`metrics`,`ports`,`coordination`,`notify`,`facade`,
   `identity`,`events`) gets a `//!` doc comment + a placeholder test so the harness is wired day one.
2. **Tauri app** in `crates/app` (id `dev.soloist.app`, title "Soloist") + Vite/React/TS UI in
   `crates/app/ui`; a stub `app_info()` command rendered in the window (proves the bridge).
3. **`api.ts`** wrapper (only `invoke`/`listen`); CSS-variable theme tokens (light/dark) plumbed.
4. **Lints:** `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` in `core`;
   `clippy -D warnings` workspace; rustfmt; ESLint/Prettier; `tsc --noEmit`. `justfile` with
   `dev|test|lint|bundle`.
5. **Dependency-direction guard (`04` §1/§10):** a CI step (script or `cargo-deny`/`x-tasks`) that
   **fails** if `crates/core` references `tauri`,`rmcp`,`axum`,`rusqlite`,`notify-rust`. This guard is
   permanent.
6. **CI** on `ubuntu-22.04`: system deps → `cargo clippy`+`cargo test` → frontend lint/test →
   `tauri build` (deb). Cache cargo+pnpm.
7. **First bundle:** emit a `.deb`; install in a clean container (xvfb) and confirm the window opens.
8. **Docs:** `CONTRIBUTING.md` with exact `apt-get` lines for Ubuntu 20.04 (webkit 4.0) **and** 22.04
   (4.1).

## Acceptance criteria
- `cargo test` + `vitest` run; CI green on `ubuntu-22.04`.
- `just bundle` produces a `.deb`; installing it in a clean container launches a window showing the
  version via `invoke('app_info')` (full Rust↔WebKit bridge proven).
- `clippy -D warnings`, `tsc --noEmit`, and the **dependency-direction guard** all pass.

## Test plan
- **Automated:** CI builds + both suites; smoke job installs the `.deb` headless and asserts startup.
- **Manual:** launch on a real Ubuntu 22.04 desktop.

## Risks & mitigations
- **webkit 4.0 vs 4.1 across Ubuntu versions** → document both; add a 20.04 CI runner.
- **Tauri v2 API churn** → pin exact versions; verify against current docs (context7), not memory.

## Effort
~2–3 days.
