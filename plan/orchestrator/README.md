# Orchestrator Track — Charter & Index

> A **standalone build track** (`orch-00 … orch-05`) layered on top of the verified Phase 7/8/9
> coordination core. It does **not** re-build coordination primitives — those are done. It makes the
> multi-agent **orchestrator** experience *legible, first-class, and complete*: the human-facing UI,
> the deferred coordination sub-tools, and the documented capability + recipe.
>
> Read this before any `orch-NN` phase. It is subordinate to the canonical contract: when this file
> disagrees with `../04-engineering-architecture-and-patterns.md` / `../05-solo-reference-and-sources.md` /
> `../06-codebase-blueprint-and-cleanup.md`, the higher doc wins (CLAUDE.md §2).

---

## 0. Why this track exists (the honest finding)

The orchestration *mechanism* a lead agent uses — spawn workers, hand out blockered todos, set a
fire-when-idle timer, sleep token-free, wake to read/verify worker output — is **already built and
`Verified`** in Soloist (Phases 7–9). The exact loop is the passing E2E test
`crates/pty/tests/orchestration.rs` (*"lead → spawn worker → assign a locked todo → fire-when-idle-all
→ integrate on wake"*, landed 2026-06-24, mutation-verified against real PTYs).

What is **missing** is everything *around* the mechanism:

| Gap | State today | This track |
|-----|-------------|------------|
| Human-facing orchestration UI (agent tree, todo/timer/scratchpad panels, wake-cycle visibility) | **none** — coordination is headless/MCP-only | orch-01 / orch-02 / orch-03 |
| Deferred coordination sub-tools (`spawn_process`, cross-project `*_transfer`) | deferred pending trust/scope design (`../05` §8, §12) | orch-04 |
| The pattern as a documented, first-class capability + recipe | exists only as a headless test | orch-05 |
| A live read-model + events to drive any of the above | coordination emits no UI-facing read-model | orch-00 |
| Worker self-onboarding — a spawned agent told its identity + the coordination tools at spawn (the demo's `include_agent_instructions` preamble) | only `SOLOIST_PROCESS_ID` env is injected; discovery left to static docs | orch-04 (O13) |
| Comment authorship — a todo comment attributed to the agent that wrote it (the demo shows `author`/`author_actor_id`) | comments are anonymous (`Comment{id,body}`); `../05` had decided "no author attribution" | orch-02 (O12) |
| `solo://` copy-link handoff — paste a scratchpad/todo link to an agent (the demo's core human handoff) | no addressable link; deep links (I4) were `later` | orch-02 (O14) |

**Therefore this track is UX + formalization + deferred tools — not new primitives.** Every phase
*consumes* the existing C6/C4/C2 behavior through the one `Facade`; none reimplements it (`../04` §2).
The three demo-fidelity rows above (O12–O14) are the only ones that touch a coordination *write* (a
comment's author, a spawned worker's first turn, a link resolver) — each is a small, bounded addition to
an existing C6/C8 behavior, not a new subsystem.

## 1. Source & clean-room note

The UX north star is the public Solo demo **"Agent orchestration, simplified" (Aaron Francis)** —
`https://www.youtube.com/watch?v=WAKGhlzpYgs` (researched 2026-06-25; deep-dive in the local video
library, id `WAKGhlzpYgs`). It is **independent evidence of how the orchestration *feels*** (agent
tree, living-document scratchpad, blockered todo chain, fire-when-idle timer with a countdown, the
injected-turn wake). Per CLAUDE.md §9 we match the *behavior/feel*, **never** Solo's assets,
screenshots, layout, strings, or branding; the visual design is produced fresh through `/impeccable`
(CLAUDE.md §5) against `../../PRODUCT.md` / `../../DESIGN.md`.

**Frame-level re-verification (2026-06-28).** The demo was re-analysed frame-by-frame (the on-screen MCP
tool calls, not just the narration) and cross-checked against the code. It **confirmed** the core
mechanism is built (`timer_fire_when_idle_all` → the scheduler injects `body + "\r"` to the lead's PTY,
proven by `crates/pty/tests/orchestration.rs`), and surfaced three faithful-to-demo details the first
pass missed: comment authorship (O12), the per-spawn orchestration-context preamble (O13), and the
`solo://` copy-link handoff (O14). Per the owner's decision they are **v1** here; a fourth, minor detail
(the wake turn naming *why* it woke) is folded into O8. None attributes Solo's assets/strings — feel only.

"Orchestrator" is **not** a documented Solo concept — the word appears nowhere in `../05`, `../02`,
`../04`, or `../06`. It is a Soloist-original composition of documented primitives, so it is recorded
as a **gap decision** (`../05` §12 style), not attributed to Solo (CLAUDE.md §9). See orch-00 Task 1.

## 2. Dependencies (what this track stands on — all `Verified`/built)

- **Phase 7 (C4):** 5-state idle FSM (`AgentActivity`), `AgentActivityChanged`, agent launch — `Verified`.
- **Phase 8 (C8):** `spawn_agent`, `get_process_output`/`_raw`, `search_output`, `send_input`+`wait_ms`,
  identity (`bind_session_process`/`whoami`), scope auth (`SO_PEERCRED`→pgid, F13) — built/tested.
- **Phase 9 (C6):** scratchpads (G1/G2), todos+blockers+locks (G3–G5), leases (G6),
  timers + `timer_fire_when_idle(IdleMode::Any/All)` + `TimerScheduler` (G7–G9), key-value (G10),
  restart-persistence (G11) — `Verified`. See `../phases/phase-09-coordination-layer.md`.

Hard constraint inherited from `../04` §3: **C6 references `ProcessId`/`ProjectId` but never controls
processes.** Orchestration UI/read-model *observes* coordination + idle and *routes actions* (spawn,
complete, cancel) to C2/C4 through the **one `Facade`** — it never starts/stops a process from C6.

## 3. Matrix expansion — new `O`-rows (explicit, per the scope decision)

`O` is an unused group letter. These are the **new parity rows** this track adds; orch-00 Task 1
propagates them into `../02-feature-parity-matrix.md`. `Src`: `✅` documented · `🟡` stated elsewhere
(here: the demo) · `❓` our design.

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|-------|--------|--------|
| O1 | Orchestration read-model: one `Facade` query projecting the lead→worker tree, todos, timers, leases, scratchpads, kv per project | ❓ | orch-00 | v1 | Query returns the snapshot; reflects a mutation |
| O2 | Coordination `DomainEvent`s (todo / timer / lease / scratchpad / kv changed) for a live UI | ❓ | orch-00 | v1 | A mutation emits its event; UI updates without polling |
| O3 | Agent lineage: parent `ProcessId` recorded on `spawn_agent`; nested lead→worker tree (promotes `later` row I14) | 🟡 | orch-01 | v1 | A spawned worker nests under its lead |
| O4 | Live orchestration tree UI with per-agent activity (Working/Thinking/Idle/Permission/Error) | 🟡 | orch-01 | v1 | Tree renders lead + workers with live glyphs |
| O5 | Scratchpad panel — disciplined `ScratchpadDoc`, revision-guarded edit, living-doc view | ❓ | orch-02 | v1 | Read/edit a scratchpad; stale edit → conflict |
| O6 | To-do board UI — blockers / locks / comments / status, blocker-gate visible | ❓ | orch-02 | v1 | Blocker gating + lock owner shown; complete refused when blocked |
| O7 | Timers & fire-when-idle panel — armed timers, `waiting_on`, max-wait countdown, injected-turn `body` preview | 🟡 | orch-03 | v1 | A `fire_when_idle` arm shows `waiting_on` + countdown |
| O8 | Wake-cycle visibility — timer fires → `body` delivered as a fresh turn, surfaced on the lead | 🟡 | orch-03 | v1 | Fired timer's body appears on the lead; timer leaves the panel |
| O9 | `spawn_process` (arbitrary terminal over MCP) with its trust treatment | ✅ name / ❓ trust | orch-04 | v1 | Trusted spawn works; untrusted / cross-project refused |
| O10 | Cross-project `scratchpad_transfer` / `todo_transfer` with cross-scope authorization | ✅ | orch-04 | v1 | In-scope transfer works; cross-scope refused (delivered 2026-07-01) |
| O11 | Orchestrator capability — documented recipe + setup guidance + first-class status | ❓ | orch-05 | v1 | Recipe doc + `setup_agent_integration` guidance; E2E walk passes |
| O12 | Todo **comment authorship** — a comment records its creating bound actor (`author_actor_id` + display author), populated by the core on create; surfaced on the to-do board | 🟡 | orch-02 | v1 | A comment created by a bound process records its actor; the board shows who wrote each comment; reverses the `../05` "no author attribution" decision |
| O13 | **Spawn orchestration-context preamble** — `spawn_agent`/`spawn_process` deliver a first-turn `[SOLO ORCHESTRATION CONTEXT]` preamble (the worker's identity + the coordination tools: `whoami`, scratchpads, todos, locks/leases, kv, timers), mirroring the demo's `include_agent_instructions` | 🟡 | orch-04 | v1 | A spawned worker receives the preamble as its first turn and can use the primitives with **no skills loaded**; applies to the already-built `spawn_agent` (not gated on the O9 arbitrary-spawn trust work) |
| O14 | **`solo://` copy-link handoff** — a stable `solo://proj/<id>/scratchpad|todo/<id>` link + a UI "Copy link" affordance + a core resolver so a receiving agent reads the target; promotes the orchestrator slice of I4 to v1 | 🟡 name (`../05` §10) / ❓ shape | orch-02 | v1 | Copy a scratchpad's link; a bound agent given the link reads it; a malformed / foreign-scope link is refused |

`later` (tracked, non-gating — do **not** gold-plate): a deep cross-project "Activity Monitor" (I12),
prompt-template UI (I13), and LLM auto-summarization of worker output (E6, OFF by default — the core
must never hard-depend on an LLM, CLAUDE.md §3).

## 4. Phase index

| Phase | Title | Delivers | Touches |
|-------|-------|----------|---------|
| [orch-00](orch-00-charter-gap-and-read-model.md) | Charter, gap decision & read-model | O1, O2 | C6/C4/C2 read side, events, docs |
| [orch-01](orch-01-agent-lineage-and-tree-ui.md) | Agent lineage & live orchestration tree | O3, O4 | C2/C4, Tauri, UI |
| [orch-02](orch-02-coordination-panels-ui.md) | Scratchpad & to-do coordination panels | O5, O6, O12, O14 | C6, Tauri, UI |
| [orch-03](orch-03-timers-and-wake-cycle-ui.md) | Timers, fire-when-idle & wake-cycle | O7, O8 | C6/C4, Tauri, UI |
| [orch-04](orch-04-deferred-coordination-tools.md) | Deferred coordination tools + spawn preamble | O9, O10, O13 | C6/C8, MCP |
| [orch-05](orch-05-formalization-recipe-and-verify.md) | Formalization, recipe, docs & parity verify | O11 | docs, MCP, E2E |

**Build order is deliberate:** orch-00 (read-model + events) unblocks every UI phase; the three UI
phases are independent slices once it lands; orch-04 (backend tools) is independent of the UI; orch-05
closes the track once the surfaces exist. Each phase ends `just lint && just test` green and updates
`../../PROGRESS.md` (CLAUDE.md §10/§11).

## 5. Per-phase definition of done (inherits CLAUDE.md §7)

1. Every `v1` `O`-row in the phase passes its Verify with evidence.
2. Phase acceptance criteria met; test plan green (UI phases: Playwright e2e + Vitest reducers; backend:
   unit on `MockClock` + adapter/integration).
3. CI gates pass: `clippy -D warnings`, `rustfmt`, `tsc --noEmit`, ESLint, dependency-direction guard.
4. UI phases were driven through `/impeccable`; Tauri surfaces confirmed against the matching `tauri-*`
   skill + official docs (CLAUDE.md §4/§5).
5. `../../PROGRESS.md` updated; any new divergence in `../../KNOWN-DIVERGENCES.md`, any new gap in `../05` §12.
