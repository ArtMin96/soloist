# PROGRESS.md — Soloist State Ledger

> **This file is the shared memory across sessions.** Git history complements it, but this ledger is
> where a session reads what's done and what's next. **Read it at the start of every session** (per
> `CLAUDE.md` §1) and **update it at the end of every session** (per `CLAUDE.md` §10–§11). Keep it
> factual and evidence-backed — never mark `Verified` what you didn't verify.

---

## Current state

- **Overall:** **Phase 0 (Foundations) complete & Verified.** The Cargo workspace, Tauri v2 +
  React/TS shell, lints, CI, the dependency-direction guard, and `.deb` packaging are in place and green.
- **Active phase:** **Phase 1 (Walking skeleton) is next.**
- **Last session:** 2026-06-14 — built Phase 0 end to end (details under "Decisions / changes").

---

## Phase status

Status vocabulary: `Not started` · `In progress` · `Done — pending verify` · `Verified`.

| Phase | Name | Status | Evidence / notes |
|------:|------|--------|------------------|
| — | Planning (foundation + 14 phase docs) | **Done** | 22 plan files in `plan/`; decisions D1–D6 locked; coordination=v1; summarization off; under git |
| 0 | Foundations (workspace, CI, `.deb` build) | **Verified** | 8-crate workspace builds; `just lint` + `just test` green (clippy -D warnings, rustfmt, ESLint, Prettier, tsc, vitest 2/2, Rust placeholder tests); dependency-direction guard passes (detection verified against `soloist-app`); `Soloist_0.1.0_amd64.deb` (2.3 MB) builds; app launches on a real desktop and renders `app_info` → "version 0.1.0" (user-confirmed). Clean-container dpkg-install smoke not run (substituted by real-desktop launch); CI `bundle` job builds the `.deb`. |
| 1 | Walking skeleton (ports/adapters + event bus) | Not started | **Next.** Spawn one process end-to-end through every layer; proves the architecture before features |
| 2 | Config & projects (real `solo.yml`, trust, sync, detect) | Not started | |
| 3 | Process supervisor (3 subtypes, status FSM, orphans) | Not started | Highest-risk; budget extra test time |
| 4 | PTY & terminal I/O (rendered+raw, input, resize, OSC) | Not started | |
| 5 | Dashboard UI (sidebar tree, status dots, terminal pane, trust dialog) | Not started | Playwright e2e starts here; drive UI through `/impeccable`; seed `DESIGN.md` first |
| 6 | Monitoring, restart (10/60s), file-watch, notifications | Not started | **Nightly soak test starts running from here** |
| 7 | Agents & idle detection (5-state FSM, optional summarization) | Not started | Summarization OFF by default |
| 8 | MCP server core (`soloist-mcp` stdio, scope+identity, tools) | Not started | High-risk |
| 9 | Coordination layer (scratchpads/todos/timers/leases/kv) | Not started | **v1 scope.** Sequence: durable store → leases/locks → timers/idle-watchers → scratchpads/todos → key-value. High-risk |
| 10 | HTTP API & CLI (`127.0.0.1:24678` + `soloist` CLI) | Not started | |
| 11 | UX polish & execution profiles (palettes, deep links, themes) | Not started | |
| 12 | Packaging (`.deb` + `.AppImage`, x86_64) | Not started | Add containerized 20.04 AppImage smoke (webkit 4.0 runtime) here |
| 13 | Parity QA + longevity gate | Not started | The v1 definition-of-done; runs the soak/leak gate and parity walk |

Estimated v1 critical path: **~14–18 focused weeks** (one experienced Rust+TS dev); Phases 3, 8, 9 carry
the most risk. See `plan/phases/phase-13-parity-qa-testing.md` appendix for the per-phase breakdown.

---

## Decisions / changes this session

### Phase 0 build (2026-06-14)
- Stood up the **8-crate Cargo workspace** (`core/store/pty/app/mcp/httpapi/cli/ipc`): a pure `core`
  with the 14 bounded-context modules, a Tauri v2 desktop shell + Vite/React/TS UI, the `app_info()`
  Rust↔WebKit bridge, a `justfile` (dev/test/lint/bundle), the **dependency-direction guard**
  (`scripts/check-core-deps.sh`), GitHub Actions CI (`.github/workflows/ci.yml`, `ubuntu-22.04`), and a
  `.deb` bundle. All gates green; `CLAUDE.md` §14 filled with verified commands; `CONTRIBUTING.md` added.
- **Frontend stack change (user instruction):** adopted **shadcn/ui (Radix + Tailwind CSS v4)** for
  components; `plan/03` updated. React is **19** (resolver picked latest, not 18). Theme tokens are
  shadcn's OKLCH light/dark, OS-followed via a `prefers-color-scheme` class toggle. Visual design still
  goes through `/impeccable` (Phase 5); shadcn supplies primitives, not the visual identity.
- **Comment policy (user instruction):** source carries docblocks + genuinely important comments only —
  **no phase numbers, plan citations, or changelog notes in code.** Scaffolding cleaned to match.
- **Toolchain:** Rust 1.96 stable, pnpm 11.6, tauri-cli 2.11.2, just (all installed). `Cargo.lock` pins
  `brotli-decompressor` 5.0.0 + `alloc-stdlib` 0.2.2 to resolve a Tauri-transitive `alloc-no-stdlib`
  2↔3 conflict (upstream brotli 8.0.3 packaging bug). **Unpin when brotli fixes it.**
- **Build host = Ubuntu 22.04+** (Tauri v2 needs WebKitGTK 4.1; 20.04 ships only 4.0). 20.04 is a
  *runtime* target via the AppImage. This corrects the Phase 0 doc's assumption that 20.04 could build
  with 4.0. GitHub removed `ubuntu-20.04` hosted runners, so CI is 22.04-only.
- Fixed two build-tooling gotchas worth remembering: Vite 8 dropped bundled esbuild (use a boolean
  `minify`, not `"esbuild"`); TS 6 deprecates `baseUrl` (use `paths` alone); Tauri runs
  `beforeBuildCommand` from the frontend dir, so it is `pnpm build` (not `pnpm -C ui build`).
- Doc fixes: corrected stale "no git" lines in `SESSION-START-PROMPT.md` and `plan/03`.

### Planning session (2026-06-14)
- Propagated **coordination layer = v1** across matrix (G1–G11, E7), Phase 9, decisions, estimate, README.
  **Summarization off by default** confirmed.
- Added `CLAUDE.md` (operating manual) + this ledger; later extended `CLAUDE.md` with §4 (authoritative
  external sources), §5 (required skills), §6 (performance/size budget).
- Mandated all UI/UX through the project-local **`/impeccable`** skill; ran `/impeccable init` → wrote
  `PRODUCT.md`. `DESIGN.md` deferred by the user.
- Confirmed the project-local `tauri-*` skill suite is the Tauri authority (backed by official docs).
- **Git initialized** + private GitHub remote **`ArtMin96/soloist`** created and pushed.
- Added `SESSION-START-PROMPT.md`.

---

## Open threads / unresolved

- **Plan review:** user may still skim `plan/05` (Solo behavior), `plan/04` (architecture), `plan/02`
  (parity) and confirm before deep feature work — not blocking Phase 1.
- **`DESIGN.md` pending** (color/type/components/motion). shadcn's OKLCH tokens are the current base;
  seed `DESIGN.md` via `/impeccable document` before the first real UI work (Phase 5). When seeding,
  `WebFetch` soloterm.com as a clean-room *reference* (feel, not assets); confirm color-blind-safe
  status encoding.
- **`KNOWN-DIVERGENCES.md`** not created yet (introduced in Phase 13; start it the moment a real
  Solo-behavior divergence is implemented — not needed for the build/toolchain decisions above).
- **Clean-container `.deb` smoke** not run (docker is available); the real-desktop launch + the CI
  `bundle` job cover it. A containerized dpkg-install + xvfb smoke can be added to CI for full automation.
- **Placeholder app icon** (`crates/app/app-icon.png` → generated `crates/app/icons/`): a simple "S"
  glyph; replace with real art in Phase 11/12.

---

## Next session should start with

1. **Phase 1 — Walking skeleton.** Read `plan/phases/phase-01-walking-skeleton.md` end to end and re-read
   its parity context. Goal: spawn one process end-to-end through every layer (core ports/adapters +
   event bus, surfaced through a real Tauri command + event), proving the hexagonal architecture before
   any feature lands. Keep `crates/core` framework-free (the guard will catch violations).
2. Run `just lint && just test` first to confirm the Phase 0 baseline is still green.
3. Optional hardening: add a containerized `.deb` install + xvfb smoke job to CI (docker available).
