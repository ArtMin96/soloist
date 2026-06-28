# Orchestrator Phase O1 — Agent Lineage & Live Orchestration Tree (C2/C4 + Tauri + UI)

**Goal:** Make the lead→worker relationship **visible and live**. Record a worker's **parent
`ProcessId`** when a lead `spawn_agent`s it (a small C2/C4 read-model addition), and render the nested
**orchestration tree** — lead with its workers under it, each showing its live 5-state activity
(Working/Thinking/Idle/Permission/Error). This is the sidebar the demo shows: one lead spawning four
Codex workers, each with a live status glyph ([README](README.md) §1, video `WAKGhlzpYgs`).

**Delivers:** O3, O4 (promotes the previously-`later` row I14 into this track). **Architecture:** lineage
in C2/C4 surfaced through orch-00's `OrchestrationSnapshot`; a thin Tauri query/event bridge; a
presentational React tree. No business logic in the adapter or components ([`04` §2](../04-engineering-architecture-and-patterns.md)).

## Scope
**In:** record `parent: Option<ProcessId>` on a spawned agent (set when `spawn_agent` runs in a bound
lead's session); expose it in the read-model; a Tauri command + event subscription feeding a React
**`OrchestrationTree`** (lead nodes → worker children → activity glyph, reusing the existing
`ProcessIndicator`); lineage survives a worker's transient states and is cleared when the worker leaves
the registry. **Out:** todo/timer/scratchpad panels (orch-02/03); spawning *from* the UI beyond the
existing launch affordance (the tree observes; actions route to existing `Facade` methods); the deep
cross-project Activity Monitor (I12, `later`).

## Why lineage is the missing piece
`spawn_agent` is built and the idle FSM is `Verified`, but **parent/child lineage is not recorded** —
the tree can't nest a worker under its lead without it. Recording the parent at spawn time (the lead is
the bound session owner, known from identity, [`05` §7](../05-solo-reference-and-sources.md)) is the
single small backend addition; everything else is presentation over orch-00's snapshot.

## Tasks
1. **Record lineage (O3, [`06` §5.1](../06-codebase-blueprint-and-cleanup.md)):** when `spawn_agent`
   creates a worker for a bound lead session, persist/track the worker's `parent` = the lead's
   `ProcessId` (the session owner from `bind_session_process`, [`05` §7](../05-solo-reference-and-sources.md)).
   Lineage is ephemeral process metadata (per-run ids, like leases/timers — [`05` §12](../05-solo-reference-and-sources.md));
   no migration. A spawn with no bound lead (external/manual launch) has `parent: None` (a root).
2. **Surface lineage in the read-model (O3):** fill `AgentNode.parent` in orch-00's `OrchestrationSnapshot`;
   add a `ProcessLineageChanged` `DomainEvent` (or reuse the existing process lifecycle event) so the
   tree restructures live when a worker spawns or closes ([`06` §5.6](../06-codebase-blueprint-and-cleanup.md)).
3. **Tauri bridge ([`06` §5.5](../06-codebase-blueprint-and-cleanup.md)):** a thin `#[tauri::command]`
   `orchestration_snapshot(project)` → the one `Facade` query (no logic in the handler); register it in
   the `invoke_handler!` list; typed wrapper in `ui/src/api.ts` (the command-name string lives only
   there). The tree subscribes to the existing `"domain-event"` channel for live updates.
   **Invoke the matching `tauri-*` skills** (`tauri-calling-rust`, `tauri-calling-frontend`,
   `tauri-frontend-events`) and confirm the IPC/event APIs against the official Tauri v2 docs before
   writing (CLAUDE.md §4/§5).
4. **`OrchestrationTree` component (O4, [`06` §5.7](../06-codebase-blueprint-and-cleanup.md)):** a
   presentational tree under `components/orchestration/`: lead rows with their worker children nested,
   each row = name + `ProcessKind` + the existing `ProcessIndicator` 5-state glyph, fed by a `store/`
   hook over the read-model. Props-in/callbacks-out; **no `invoke`, no logic** in the component. Collapse
   state per lead; smooth, non-flickering activity updates (coalesce per frame, CLAUDE.md §6). **Drive
   the whole surface through `/impeccable`** against `../../PRODUCT.md`/`../../DESIGN.md` (CLAUDE.md §5);
   match the demo's *feel*, never its assets (CLAUDE.md §9).
5. **Empty/degenerate states:** a project with no agents, a root agent with no workers, and a worker
   whose lead has closed (re-parent to root, don't orphan-hide) all render cleanly.

## Interfaces
```rust
struct AgentNode { id: ProcessId, parent: Option<ProcessId>, label: String, kind: ProcessKind, status: ProcStatus, activity: Option<AgentActivity> }
#[tauri::command] async fn orchestration_snapshot(project: ProjectId) -> OrchestrationSnapshot; // → Facade
```
```ts
// ui/src/store/useOrchestration.ts — pure read-model hook over the snapshot + domain events
type AgentNode = { id: number; parent: number | null; label: string; kind: ProcessKind; status: ProcStatus; activity?: AgentActivity }
```
`label` (the row's name) is filled from the existing `ProcessView` during snapshot assembly, so the
tree renders from one self-contained projection rather than joining a second read-model.

## Acceptance criteria
- A lead that `spawn_agent`s a worker shows that worker **nested under it** in the tree; a manually
  launched agent appears as a **root**.
- Each agent row reflects its **live** 5-state activity (transitions Working↔Idle↔Permission visible
  without a refresh or flicker), reusing `ProcessIndicator`.
- Closing a worker removes its node and (if it had children) re-parents them to root — no stranded nodes.
- The component issues **no `invoke`** and holds **no business logic** (review-checked); the Tauri
  handler is a one-line `Facade` call; `tsc`/ESLint/clippy green.

## Test plan
- **Unit (core):** lineage recorded on a bound-lead `spawn_agent`; `parent: None` on an unbound spawn;
  re-parent-to-root on lead close.
- **Unit (UI, Vitest):** the read-model hook builds the correct parent→children shape from a snapshot +
  a sequence of lineage/activity events.
- **Headless IPC + real-window e2e:** the Phase-5 finding (recorded in `PROGRESS.md`) is that WebKitGTK
  exposes no CDP, so the real-window walk is **WebdriverIO + tauri-driver** (sudo deps, user-only), and
  the **headless layer is mockIPC behavior tests** — *not* Playwright. orch-01's headless coverage: the
  pure `buildOrchestrationTree` (parent→children shape over a sequence of snapshots), the
  `OrchestrationTree` component (nested treeitems, kind, empty state), and a mockIPC `orchestration_snapshot`
  wrapper round-trip. The live glyph-flip on an activity event is part of the user-only real-window walk.

## Risks & mitigations
- **"Quiet ≠ done" idle ambiguity (D-5, [`05` §12](../05-solo-reference-and-sources.md))** → the tree
  *shows* activity, it never auto-acts on it; idle is a signal, decisions stay with the lead agent/user
  (mirrors the Phase 7 idle risk).
- **Tree churn under chatty workers** → coalesce activity updates per animation frame; virtualize if a
  project ever exceeds a sane node count (CLAUDE.md §6 / [`04` §8](../04-engineering-architecture-and-patterns.md)).
- **Lineage outliving its run** → lineage is per-run ephemeral process metadata, never persisted across
  restart ([`05` §12](../05-solo-reference-and-sources.md) timer/lease precedent).

## Effort
~3–4 days (lineage is small; the tree UI + `/impeccable` pass + Playwright is the bulk).
