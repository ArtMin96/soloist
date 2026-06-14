# PROGRESS.md — Soloist State Ledger

> **This file is the shared memory across sessions.** Git history complements it, but this ledger is
> where a session reads what's done and what's next. **Read it at the start of every session** (per
> `CLAUDE.md` §1) and **update it at the end of every session** (per `CLAUDE.md` §10–§11). Keep it
> factual and evidence-backed — never mark `Verified` what you didn't verify.

---

## Current state

- **Overall:** Planning complete (draft **v2**). **No application code written yet.**
- **Active phase:** none yet — **Phase 0 (Foundations) is next.**
- **Last session:** 2026-06-14 — authored `CLAUDE.md` (session operating manual) + this ledger, then
  added standing rules: **always consult official docs** (`code.claude.com/llms.txt`,
  `tauri.app/llms.txt`, `context7` MCP) and never fabricate APIs; **drive all UI/UX through the
  `/impeccable` skill before UI work** (north star: soloterm.com feel, clean-room); Tauri authority =
  the project-local `tauri-*` skill suite + official docs; **performance/size/responsiveness budget** is now a
  first-class gate. Then ran **`/impeccable init`** → created `PRODUCT.md`.
- **Awaiting user:** approval to move from plan → implementation. *(Git resolved this session — private
  repo `ArtMin96/soloist`.)*

---

## Phase status

Status vocabulary: `Not started` · `In progress` · `Done — pending verify` · `Verified`.

| Phase | Name | Status | Evidence / notes |
|------:|------|--------|------------------|
| — | Planning (foundation + 14 phase docs) | **Done** | 22 plan files in `plan/`; decisions D1–D6 locked; coordination=v1; summarization off; under git |
| 0 | Foundations (workspace, CI, `.deb` build) | Not started | **Next.** Stand up Tauri+Rust+React workspace, crate layout (`core/store/pty/app/mcp/httpapi/cli/ipc`), CI gates incl. dependency-direction check |
| 1 | Walking skeleton (ports/adapters + event bus) | Not started | Spawn one process end-to-end through every layer; proves the architecture before features |
| 2 | Config & projects (real `solo.yml`, trust, sync, detect) | Not started | |
| 3 | Process supervisor (3 subtypes, status FSM, orphans) | Not started | Highest-risk; budget extra test time |
| 4 | PTY & terminal I/O (rendered+raw, input, resize, OSC) | Not started | |
| 5 | Dashboard UI (sidebar tree, status dots, terminal pane, trust dialog) | Not started | Playwright e2e starts here |
| 6 | Monitoring, restart (10/60s), file-watch, notifications | Not started | **Nightly soak test starts running from here** |
| 7 | Agents & idle detection (5-state FSM, optional summarization) | Not started | Summarization OFF by default |
| 8 | MCP server core (`soloist-mcp` stdio, scope+identity, tools) | Not started | High-risk |
| 9 | Coordination layer (scratchpads/todos/timers/leases/kv) | Not started | **v1 scope.** Sequence: durable store → leases/locks → timers/idle-watchers → scratchpads/todos → key-value. High-risk |
| 10 | HTTP API & CLI (`127.0.0.1:24678` + `soloist` CLI) | Not started | |
| 11 | UX polish & execution profiles (palettes, deep links, themes) | Not started | |
| 12 | Packaging (`.deb` + `.AppImage`, x86_64) | Not started | |
| 13 | Parity QA + longevity gate | Not started | The v1 definition-of-done; runs the soak/leak gate and parity walk |

Estimated v1 critical path: **~14–18 focused weeks** (one experienced Rust+TS dev); Phases 3, 8, 9 carry
the most risk. See `plan/phases/phase-13-parity-qa-testing.md` appendix for the per-phase breakdown.

---

## Decisions / changes this session

- 2026-06-14: Propagated **coordination layer = v1** across matrix (G1–G11, E7), Phase 9, decisions doc,
  estimate, README. **Summarization off by default** confirmed; git was initially recorded as no-repo
  (changed this session — see below).
- 2026-06-14: Added `CLAUDE.md` (operating manual) + this ledger so each phase-session behaves
  consistently.
- 2026-06-14: `CLAUDE.md` extended — new §4 (authoritative external sources: the two `llms.txt` indexes
  + `context7`, "never fabricate"), §5 (required skills), §6 (performance/size/responsiveness budget).
  Both `llms.txt` URLs verified reachable. (Superseded below: a full `tauri-*` skill suite IS installed
  project-local — that's the Tauri authority, backed by the official docs.)
- 2026-06-14: **Corrected the UI/UX skill** — the mandated design skill is the project-local
  **`/impeccable`** (`.claude/skills/impeccable/`), **not** `frontend-design`. `CLAUDE.md` §1/§5/§13
  updated to drive all UI through `/impeccable`; `PRODUCT.md` + `DESIGN.md` are now the design
  source-of-truth every impeccable command reads first.
- 2026-06-14: Ran **`/impeccable init`** (interview round) → wrote **`PRODUCT.md`**: register =
  **product**; personality **calm · precise · native**; visual direction **native-desktop calm**
  (soloterm feel, clean-room); anti-references = all four (no SaaS-dashboard, no cream/beige AI default,
  no web-app-in-a-window, no toy/skeuomorphic); a11y = AA contrast + **light/dark/system** (OS-follow) +
  reduced-motion; color-blind-safe status flagged as *recommended, confirm before Phase 5*. **`DESIGN.md`
  not yet created** (pre-implementation; seed it before UI work — see Open threads). **DESIGN.md
  deferred** by the user for now.
- 2026-06-14: Added `SESSION-START-PROMPT.md` (repo root) — the reusable prompt to paste at the start of
  every work session; it forces the CLAUDE.md §1 protocol and the non-negotiables.
- 2026-06-14: **Correction — Tauri skills DO exist.** Discovered a project-local suite of ~40 `tauri-*`
  skills under `.claude/skills/` (plus `impeccable`). `CLAUDE.md` §1/§5/§13 updated to **mandate
  invoking the matching `tauri-*` skill before any Tauri work** — e.g. `tauri-linux-packaging` for
  `.deb`/`.AppImage`, `tauri-binary-size` for the size budget, `tauri-ipc`/`tauri-capabilities` for
  commands/security — backed by the official docs.
- 2026-06-14: **Git initialized + private repo created.** `git init` (branch `main`); `.gitignore`
  added (Rust/Node/Tauri build output, `.impeccable/` runtime, `.claude/settings.local.json`). Initial
  commit `44e8c01` (157 files, incl. `.claude/skills` tooling). Private GitHub remote
  **`ArtMin96/soloist`** created and pushed → https://github.com/ArtMin96/soloist.

---

## Open threads / unresolved

- **Git:** ✅ resolved — initialized (`main`), private GitHub remote **`ArtMin96/soloist`**
  (`origin/main`, https://github.com/ArtMin96/soloist). Commit per phase from here.
- **Plan review:** user to skim `plan/05` (Solo behavior), `plan/04` (architecture), `plan/02` (parity)
  and confirm before implementation begins.
- `KNOWN-DIVERGENCES.md` not created yet (formally introduced in Phase 13, but start it the moment any
  intentional difference from Solo is implemented).
- **`DESIGN.md` pending** (the visual system: color/type/components/motion). It's the second file from
  `/impeccable init`; seed it via `/impeccable document` before the first UI work (Phase 5 at the
  latest). When seeding, `WebFetch` soloterm.com to observe its real palette/type/layout as a clean-room
  *reference* (feel, not assets), and run `.claude/skills/impeccable/scripts/palette.mjs` for a brand
  seed color. Color-blind-safe status encoding to be confirmed at the same time.

---

## Next session should start with

1. Confirm with the user: **plan approved?** (git is done — private repo `ArtMin96/soloist`).
2. If approved → either (a) produce the detailed implementation plan for **Phase 0 + Phase 1** (the
   foundations + walking-skeleton spine), or (b) begin Phase 0 directly if the user wants to build.
3. Whoever completes Phase 0 must fill in the real toolchain commands in `CLAUDE.md` §14 and flip
   Phase 0 to `Done — pending verify` → `Verified` here with evidence.
