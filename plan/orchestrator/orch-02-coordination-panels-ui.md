# Orchestrator Phase O2 — Scratchpad & To-do Coordination Panels (C6 + Tauri + UI)

**Goal:** Make the two shared "documents" of an orchestration **visible and editable**: the
**scratchpad** as the living research/plan document (the demo's "updated just now by orchestrator"), and
the **to-do board** as the blockered work chain (who blocks who, who holds the lock). Both render the
**disciplined typed schemas** the core enforces and write back **revision-guarded** — so a stale human
edit loses to a concurrent agent edit exactly as an agent's would ([README](README.md) §1; video
`WAKGhlzpYgs`).

**Delivers:** O5, O6. Supersedes Phase 11 Task 10 ("scratchpad/todo panels", previously deferred). 
**Architecture:** presentational React panels over orch-00's read-model; writes route through the
**existing** G1–G5 `Facade` methods via thin Tauri commands — no reimplementation ([`04` §2](../04-engineering-architecture-and-patterns.md),
[`06` §5.5](../06-codebase-blueprint-and-cleanup.md)).

## Scope
**In:** a **scratchpad panel** (read/list, render the canonical `ScratchpadDoc`, edit revision-guarded,
surface `RevisionConflict`); a **to-do board** (list/get, show status + **blockers** + the derived
`blocked` gate + **lock owner** + comments, create/update/complete revision-guarded). Both go through the
existing Phase 9 repos. **Out:** timers/wake-cycle (orch-03); free-form scratchpad tools and
cross-project transfer (deferred — orch-04 / [`05` §12](../05-solo-reference-and-sources.md)); mermaid
rendering polish (lazy-load later if a doc needs it, CLAUDE.md §6).

## Why typed, not free-form
Per `D-7`/`D-8` ([`../../KNOWN-DIVERGENCES.md`](../../KNOWN-DIVERGENCES.md)) a scratchpad is a typed
`ScratchpadDoc { objective, context, plan[], acceptance_criteria[], risks[], status, notes? }` and a todo
is a typed `TodoDoc { title, description, acceptance_criteria[], risks[], status }`, validated on write
and rendered to one canonical layout — *"I don't want to let AI write different ways every time."* The
panels are **structured editors over those schemas**, not a free Markdown textarea; the blocker **gate**
lives in the blocker set, not the status label ([`05` §12](../05-solo-reference-and-sources.md), G4).

## Tasks
1. **Expose the C6 reads/writes to the UI via Tauri ([`06` §5.5](../06-codebase-blueprint-and-cleanup.md)):**
   thin `#[tauri::command]`s for the scratchpad and todo `Facade` methods the panels call
   (`scratchpad_list/_read/_write`, `todo_list/_get/_create/_update/_complete/_comment_*`,
   `todo_set_blockers/_add_blocker/_remove_blocker`, `todo_lock/_unlock`) — each a one-line route to the
   **existing** `Facade` method (the MCP tools already call these; the UI is just another frontend,
   [`04` §2](../04-engineering-architecture-and-patterns.md)). Register in `invoke_handler!`; typed
   wrappers in `api.ts`. Confirm the IPC contract via the `tauri-calling-rust`/`-frontend` skills + docs.
2. **Scratchpad panel (O5, [`06` §5.7](../06-codebase-blueprint-and-cleanup.md)):** list scratchpads
   (name/tags/revision/objective gist); open one rendered as the canonical doc; a **structured editor**
   for its fields that writes with `expected_revision`; on mismatch show the `RevisionConflict {expected,
   actual}` clearly and offer reload. Live-refresh on `ScratchpadChanged` (the "living document" effect).
3. **To-do board (O6, [`06` §5.7](../06-codebase-blueprint-and-cleanup.md)):** a column/board of todos
   showing `status`, the **blockers** with their state, the derived **`blocked`** flag, the **lock owner**
   (`locked_by`), and comments; create/update/complete via the Tauri commands. **Completing a blocked
   todo is refused** in the core with `TodoBlocked { by }` — surface that, don't pre-empt it in the UI
   (the gate is one source of truth, [`05` §12](../05-solo-reference-and-sources.md) G4). Live-refresh on
   `TodoChanged`.
4. **Single-source rendering ([`04` §10](../04-engineering-architecture-and-patterns.md)):** the
   `ScratchpadDoc`/`TodoDoc` shapes and the status→glyph/label map are mirrored **once** in `domain.ts`/
   `lib/`; no per-component re-definition of a status string. Components are presentational; data via
   `store/` hooks; **no `invoke`/logic** in components ([`06` §5.7](../06-codebase-blueprint-and-cleanup.md)).
5. **`/impeccable` pass (CLAUDE.md §5):** design both panels through `/impeccable` against
   `../../PRODUCT.md`/`../../DESIGN.md`; calm density, keyboard-first, light/dark, contrast ≥4.5:1; match
   the demo's *feel*, not its assets (CLAUDE.md §9). Pair with `webapp-testing` for the e2e.

## Interfaces
```rust
// existing Facade methods (Phase 9) — exposed to UI via thin Tauri commands, NOT reimplemented:
impl Facade {
  fn scratchpad_write(&self, p: ProjectId, name: &str, doc: ScratchpadDoc, expected_rev: Option<Revision>) -> Result<Written, ScratchpadError>;
  fn todo_complete(&self, p: ProjectId, id: TodoId) -> Result<(), TodoBlocked>; // refused while a blocker is unmet
  fn todo_set_blockers(&self, p: ProjectId, id: TodoId, blockers: Vec<TodoId>) -> Result<(), TodoError>;
}
```

## Acceptance criteria
- A scratchpad opens rendered as its disciplined doc; editing and saving at the current revision
  succeeds; saving a **stale** revision shows the `RevisionConflict` and does not clobber.
- A todo with an unmet blocker shows as **blocked** and **`complete` is refused** (`TodoBlocked { by }`
  surfaced); once the blocker completes, the gate clears and complete succeeds.
- A todo locked by a process shows its **lock owner**; the panel never lets the UI "steal" a lock (locks
  are signals, not ownership — [`05` §12](../05-solo-reference-and-sources.md)).
- Both panels refresh live on their `DomainEvent` (no manual reload); components carry no `invoke`/logic;
  all gates green.

## Test plan
- **Unit (UI, Vitest):** the scratchpad editor maps fields ↔ `ScratchpadDoc` and threads
  `expected_revision`; the board derives `blocked`/lock-owner from a snapshot; the conflict path renders.
- **Integration (Tauri command → Facade):** each new command routes to the existing method and returns
  its typed error unchanged (a forced stale write returns `RevisionConflict`; a blocked complete returns
  `TodoBlocked`).
- **Playwright e2e:** edit + save a scratchpad; force a conflict (second writer) and assert the conflict
  UI; create a blocker chain and assert complete is refused then allowed after the blocker completes.

## Risks & mitigations
- **Re-implementing validation/gating in the UI** → forbidden; the UI renders the core's typed result
  and errors, never re-decides blocked-ness or revision validity ([`04` §13](../04-engineering-architecture-and-patterns.md)).
- **Free-form drift vs the disciplined schema** → the editor is field-structured per `D-7`/`D-8`; the
  deferred free-form tools (`_append`/`_edit`/…) are **not** pulled in here ([`05` §12](../05-solo-reference-and-sources.md)).
- **mermaid/markdown bundle weight** → lazy-load any heavy renderer only when a doc needs it (CLAUDE.md §6).

## Effort
~4–5 days (two structured editors + conflict/gate UX + `/impeccable` + Playwright).
