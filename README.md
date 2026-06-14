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
