# Soloist — a faithful, open clone of Solo for Ubuntu/Linux

> **Working name:** "Soloist" (rename freely — placeholder, not a trademark claim).
> **Goal:** Reproduce the functionality of [Solo](https://soloterm.com/) (`com.soloterm.solo`, v0.8.2)
> as a native Linux desktop app. Solo is macOS-only and closed-source, so this is a **clean-room
> rebuild** from its public documentation and observable behavior — no copied code, assets, or branding.

This directory holds **planning documents only** — no application code yet. The plan is grounded in a
real research pass over soloterm.com's public docs (a 183-page sitemap), blog, comparison pages, and
changelog; the verified facts live in [`plan/05-solo-reference-and-sources.md`](plan/05-solo-reference-and-sources.md)
with source URLs.

## ✅ Confirmed decisions

| # | Decision | Choice | Notes |
|---|----------|--------|-------|
| D1 | **Stack** | Tauri v2 + Rust core + React/TypeScript + xterm.js | Same as Solo → faithful + native Linux bundles |
| D2 | **Target** | **Ubuntu 20.04+, x86_64 only** | `.deb` + `.AppImage`; arm64 dropped |
| D3 | **Licensing** | **Dropped entirely** | No Free/Pro tiers, no license server, no analytics |
| D4 | MCP server | Separate `soloist-mcp` binary, **stdio transport** | Mirrors Solo's bundled `mcp` helper |
| D5 | `solo.yml` | **Byte-compatible** with Solo's documented schema | Real schema now known (ref §3) |
| D6 | Storage | **SQLite** for durable state; in-memory for runtime | See architecture §7 |

## Platform support

| | |
|---|---|
| ✅ **Supported** | **Ubuntu 22.04+, x86_64** — install the `.deb` (apt) or run the portable `.AppImage`. |
| ❌ **Not supported** | **Ubuntu 20.04** — Tauri v2 needs WebKitGTK 4.1 (absent on 20.04), so the bundle is built on 22.04 and its glibc 2.33+ artifacts will not run on 20.04's glibc 2.31. **arm64, macOS, Windows** — out of scope (D2). |

> D2 set a 20.04 floor expecting the self-contained `.AppImage` to cover it; Phase-12 testing
> proved that infeasible (see [`KNOWN-DIVERGENCES.md`](KNOWN-DIVERGENCES.md) D-11). The effective
> floor for both artifacts is **Ubuntu 22.04+**.

## The `solo.yml` project file

`solo.yml` lives at a project's root and declares the commands Soloist supervises. It is
**byte-compatible with Solo's schema** (D5). The file is optional: open a folder without one and
Soloist **auto-detects** a starting set of commands and writes a `solo.yml` for you — it never rewrites
an existing one. Maximum size **1 MB**; an empty or comment-only file is a valid, empty config. The
implementation source of truth is `crates/core/src/config/model.rs`; the cited Solo reference is
[`plan/05` §3](plan/05-solo-reference-and-sources.md).

### Full example

```yaml
name: storefront                 # optional — display name shown for the project
icon: assets/icon.png            # optional — image path, relative to the project root

processes:                       # a MAP keyed by each command's display name (not a list)
  web:
    command: npm run dev         # required — the shell command to run
    working_dir: web             # optional — relative to the project root (default: the root)
    auto_start: true             # optional — start when the project opens (default: true)
    auto_restart: true           # optional — relaunch after an unexpected exit (default: false)
    restart_when_changed:        # optional — globs (relative to root) that trigger a restart
      - src/**/*.ts
      - config/**
    env:                         # optional — per-process environment overrides
      PORT: "3000"
  build:
    command: npm run build
    auto_start: false            # a one-shot task — don't start it on open
```

### Fields

**Top level**

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `name` | string | no | — | Display name for the project. |
| `icon` | path | no | — | Image path, relative to the project root. |
| `processes` | map | no | `{}` | Commands keyed by display name; file order is preserved. |

**Each entry under `processes:`**

| Key | Type | Required | Default | Meaning |
|-----|------|----------|---------|---------|
| `command` | string | **yes** | — | The shell command to run. |
| `working_dir` | path | no | project root | Working directory, relative to the root. |
| `auto_start` | bool | no | `true` | Start this command when the project opens. |
| `auto_restart` | bool | no | `false` | Relaunch after an unexpected (crash) exit. |
| `restart_when_changed` | list of globs | no | `[]` | File globs (relative to root) that trigger a restart. |
| `env` | map | no | `{}` | Environment overrides for this command (highest precedence). |

Only `command` is required; every other key may be omitted and falls back to its default. When Soloist
writes a `solo.yml` for you it omits keys left at their defaults, so the file stays minimal.

### Behavior

- **Trust gate.** A command must be *trusted* before it can start by any path (manual, auto-start,
  restart, or file-watch). Trust is local to your machine and scoped to the project and the exact
  command *variant* (`command` + `working_dir` + `env`): renaming a command preserves its trust, while
  changing the command, directory, or environment requires re-trusting it.
- **Auto-detection (first open only).** A folder with no `solo.yml` is scanned — `package.json` scripts,
  `Procfile`, `Makefile`/`justfile`, `Cargo.toml`, `go.mod`, Docker Compose, … — and a `solo.yml` is
  written from what's found: dev/start/serve commands get `auto_start: true`, build/test are added
  without it. Detected commands start out untrusted, so nothing runs until you trust it.
- **`restart_when_changed` / `auto_restart`** are parsed today; their live file-watch and crash-restart
  behavior is built in Phase 6.

## Local HTTP API and CLI

When Soloist is running it serves a loopback HTTP API on `127.0.0.1:24678`, and the `soloist`
command-line tool drives that API from a shell. Both route to the same core commands as the desktop
UI. See [`docs/http-api.md`](docs/http-api.md) for the endpoints, their JSON payloads, the
`X-Soloist-Local-Auth` mutation header, and the CLI subcommands.

## How to read these docs

> **New session? Start with [`CLAUDE.md`](CLAUDE.md) and [`PROGRESS.md`](PROGRESS.md).** `CLAUDE.md` is
> the operating manual — the mandatory start-of-session protocol, the rules every phase must follow, and
> the load-bearing facts. `PROGRESS.md` is the state ledger — what's done and what's next (there's no
> git, so this *is* the memory). Then read the plan docs below.

1. [`plan/00-vision-and-scope.md`](plan/00-vision-and-scope.md) — what we are/aren't building, success criteria.
2. [`plan/05-solo-reference-and-sources.md`](plan/05-solo-reference-and-sources.md) — **ground truth**: how Solo actually works, cited. Read this early.
3. [`plan/01-architecture.md`](plan/01-architecture.md) — the system: binaries, modules, data flow.
4. [`plan/04-engineering-architecture-and-patterns.md`](plan/04-engineering-architecture-and-patterns.md) — **the engineering backbone**: domains, patterns, and the rules that keep it from breaking under continuous work. *(Your headline requirement.)*
5. [`plan/02-feature-parity-matrix.md`](plan/02-feature-parity-matrix.md) — every Solo feature → phase → v1/later → how we verify it.
6. [`plan/03-tech-stack-and-decisions.md`](plan/03-tech-stack-and-decisions.md) — stack, crates, approaches considered.
7. [`plan/glossary.md`](plan/glossary.md) — shared vocabulary.
8. [`plan/phases/`](plan/phases/) — the build, phase by phase (00 → 13).

## Phase map (14 phases)

| Phase | File | Outcome |
|------:|------|---------|
| 0 | phase-00-foundations.md | Tauri+Rust+React workspace; CI; `.deb` builds on Ubuntu |
| 1 | phase-01-walking-skeleton.md | Ports/adapters skeleton + event bus; one process spawned end-to-end through every layer |
| 2 | phase-02-config-and-projects.md | Real `solo.yml` schema, trust store, sync/hash, auto-detection, project registry |
| 3 | phase-03-process-supervisor.md | Command/Agent/Terminal subtypes; status FSM; start/stop/restart; orphan adoption |
| 4 | phase-04-pty-and-terminal-io.md | PTYs; rendered + raw buffers; interactive input; resize; OSC parsing |
| 5 | phase-05-dashboard-ui.md | Sidebar process tree, status dots, terminal pane, trust review dialog |
| 6 | phase-06-monitoring-restart-notifications.md | CPU/mem, crash restart (10/60s), file-watch restart, notifications, attention bell |
| 7 | phase-07-agents-idle-detection.md | Agent tool config, launching, 5-state idle FSM, optional auto-summarization |
| 8 | phase-08-mcp-server-core.md | `soloist-mcp` stdio; project scope + identity; process/output/services/bulk tools |
| 9 | phase-09-coordination-layer.md | Scratchpads (rev-guarded), todos (blockers/locks/comments), timers, leases, key-value + MCP tools |
| 10 | phase-10-http-api-and-cli.md | Loopback HTTP API (`127.0.0.1:24678`) + `soloist` CLI |
| 11 | phase-11-ux-polish-and-execution-profiles.md | Command/jump palette, `soloist://` deep links, themes, shortcuts, settings, execution profiles |
| 12 | phase-12-packaging-ubuntu.md | `.deb` + `.AppImage` (x86_64), desktop entry, icons, update channel |
| 13 | phase-13-parity-qa-testing.md | Parity walk + e2e/integration + **longevity/soak gate** |

**Orchestrator track (standalone, planned 2026-06-26):** [`plan/orchestrator/`](plan/orchestrator/) — a
user-directed track (`orch-00 … orch-05`) layered on the `Verified` Phase 7/8/9 coordination core, adding
the human-facing orchestration UI, the deferred coordination sub-tools, and the documented orchestrator
recipe. It is **UX + formalization + deferred tools, not new primitives** (the mechanism is the passing E7
test). Charter + the `O`-row matrix expansion: [`plan/orchestrator/README.md`](plan/orchestrator/README.md).

## Build order rationale

Config → skeleton → supervisor → I/O → UI → self-healing → agents → MCP → coordination → API/CLI →
polish → package → verify. The **architecture (Phase 1)** is built as a walking skeleton *before*
features, so every later phase drops into a proven ports/adapters structure. The riskiest logic
(supervisor P3, MCP P8, coordination P9) is built headless and tested against a deterministic clock
before any UI depends on it.

## Status

Planning draft **v2** (2026-06-10) — corrected and expanded after deep research into Solo's real docs.
All foundation docs + 14 phase files written. Decisions D1–D6 confirmed; the **coordination layer
(Phase 9) is v1 scope**; **auto-summarization defaults off**; **no git repo** (plain files). Session
continuity is now governed by [`CLAUDE.md`](CLAUDE.md) (operating manual) + [`PROGRESS.md`](PROGRESS.md)
(state ledger). Ready for your review, then the implementation-plan step.
