# Orchestrator Phase O0 — Charter, Gap Decision & Read-Model (C6/C4/C2 query side)

**Goal:** Lay the foundation the whole track stands on, with **no new business logic**: (1) record the
orchestrator as a clean-room **gap decision** and propagate the new `O`-rows into the matrix; (2) build
a single **orchestration read-model** — one `Facade` query projecting the live lead→worker tree, todos,
timers, leases, scratchpads, and key-value per project; (3) emit the **coordination `DomainEvent`s** a
live UI needs so panels update without polling. This is pure CQRS-lite read side over the already-built
C6/C4/C2 state ([`04` §5 CQRS-lite](../04-engineering-architecture-and-patterns.md), [README](README.md) §0/§2).

**Delivers:** O1, O2. **Architecture:** query projections in C8 `Facade` over C6 repos + C4 idle + C2
registry; `events::DomainEvent` additions ([`06` §5.6](../06-codebase-blueprint-and-cleanup.md)). No
adapter logic; no port changes.

## Scope
**In:** the gap/matrix/divergence record; an `OrchestrationSnapshot` read-model + `Facade` query
methods; the coordination `DomainEvent` variants (+ their emission at the existing C6 mutation sites);
the TS `domain.ts` mirror + `store/projection.ts` reducer entries. **Out:** any Tauri command or React
component (orch-01+); any new MCP tool (orch-04); any change to coordination *write* semantics (G1–G11
are frozen — we only add reads + events alongside them).

## Why a read-model first
The UI must render a *pushed* projection and hold **no business logic** ([`04` §2](../04-engineering-architecture-and-patterns.md));
coordination is durable SQLite state independent of live processes ([`04` §3](../04-engineering-architecture-and-patterns.md)).
A single query + an event stream is the seam that lets orch-01/02/03 be thin presentational layers and
keeps the dependency rule intact (UI → Facade, never UI → a context internal, [`06` §5.5/§5.7](../06-codebase-blueprint-and-cleanup.md)).

## Tasks
1. **Record the gap + expand the matrix (O-rows) + fold in the three demo-fidelity decisions:** add an
   "Orchestrator (clean-room composition)" gap row to [`05` §12](../05-solo-reference-and-sources.md) (the
   concept is undocumented for Solo — it is ours; [README](README.md) §1); add rows **O1–O14** to
   [`02-feature-parity-matrix.md`](../02-feature-parity-matrix.md) with a new `O — Orchestrator` group
   header; cross-link the demo as the `🟡` UX source. **Then record the three 2026-06-28 re-verification
   decisions (the demo's small load-bearing details, owner-approved as v1 — [README](README.md) §0/§1):**
   - **O12 (comment authorship):** update the `MCP todo_comment_*` row in [`05` §12](../05-solo-reference-and-sources.md)
     to **attribute a comment to its creating bound actor** (`author_actor_id`), citing the demo's
     `todo_get` showing `author`/`author_actor_id` — this **reverses** that row's earlier "no author
     attribution" decision (it is a correction toward the demo, so it *removes* a latent divergence rather
     than adding one).
   - **O13 (spawn orchestration-context preamble):** add a gap-decision recording that `spawn_agent`/
     `spawn_process` inject a first-turn `[SOLO ORCHESTRATION CONTEXT]` preamble (identity + the
     coordination tools), citing the demo's `include_agent_instructions`. Net-new behavior toward the
     demo; clean-room content (our preamble text, not Solo's).
   - **O14 (`solo://` handoff):** record that the orchestrator slice of the documented `solo://` deep
     links ([`05` §10](../05-solo-reference-and-sources.md)) is **promoted from `later` (I4) to v1** — a
     scratchpad/todo link + resolver for the agent handoff.
   Add a `KNOWN-DIVERGENCES.md` entry **only** where we observably differ from a *documented* Solo
   behavior (none is expected — net-new UI/tools and these three corrections move us toward the demo, not
   away; the disciplined-schema choices remain `D-7`/`D-8`). No source code in this task.
2. **`OrchestrationSnapshot` read-model (O1, [`04` §5](../04-engineering-architecture-and-patterns.md)):**
   a serde read type in `core` projecting, for an effective project: the **agent lineage tree** (each
   managed `Agent`/`Command`/`Terminal` with `ProcStatus`, and — once orch-01 lands lineage — its
   `parent`/children), each agent's `AgentActivity` (C4), open **todos** (with derived `blocked` flag,
   lock owner, blockers), armed **timers** (`FireCond`, `waiting_on`, deadline), held **leases**, and
   **scratchpad**/**kv** summaries. Assemble it from existing repo `list`/`get` reads + the C2 registry
   snapshot + the C4 idle tracker — **no new persistence, no new repo methods unless a read is missing**.
3. **`Facade` query methods (O1, [`06` §5.1](../06-codebase-blueprint-and-cleanup.md)):**
   `orchestration_snapshot(project) -> OrchestrationSnapshot` (and any focused sub-queries the panels
   need, e.g. `todos(project)`, `timers(project)`), mirroring the existing `snapshot()` query shape.
   Read-only; honors effective project scope in the core ([`04` §12](../04-engineering-architecture-and-patterns.md)).
4. **Coordination `DomainEvent`s (O2, [`06` §5.6](../06-codebase-blueprint-and-cleanup.md)):** add the
   variants a live UI needs that aren't already emitted — `TodoChanged{project,id}`,
   `TimerArmed/TimerFired/TimerCleared{owner,id}`, `LeaseChanged{project,key}`,
   `ScratchpadChanged{project,id}`, `KvChanged{project,key}` (reuse any that already exist; do **not**
   duplicate). Emit each at its **existing** C6 mutation site via the `EventBus` — a one-line emission
   next to the write, never new logic. `AgentActivityChanged` (C4) is reused as-is for the tree.
5. **TS mirror + projection (O2, [`06` §5.6](../06-codebase-blueprint-and-cleanup.md)):** mirror the new
   variants in the **one** `ui/src/domain.ts` `DomainEvent` union and handle them in the exhaustive
   `store/projection.ts` switch; mirror `OrchestrationSnapshot` once in `domain.ts`. The
   `"domain-event"` channel name stays the single per-side constant.

## Interfaces
```rust
// core read-model (query side; no writes)
struct OrchestrationSnapshot {
  project: ProjectId,
  agents: Vec<AgentNode>,            // lineage tree (parent filled once orch-01 lands)
  todos: Vec<TodoView>,             // incl. derived `blocked`, `locked_by`, `blockers`
  timers: Vec<TimerView>,           // FireCond, waiting_on, deadline
  leases: Vec<LeaseView>, scratchpads: Vec<ScratchpadSummary>, kv: Vec<KvEntry>,
}
struct AgentNode { id: ProcessId, parent: Option<ProcessId>, kind: ProcessKind, status: ProcStatus, activity: Option<AgentActivity> }
impl Facade { pub fn orchestration_snapshot(&self, p: ProjectId) -> OrchestrationSnapshot; }

enum DomainEvent { /* …existing… */ TodoChanged{project:ProjectId,id:TodoId}, TimerArmed{owner:ProcessId,id:TimerId}, TimerFired{owner:ProcessId,id:TimerId}, /* … */ }
```

## Acceptance criteria
- `02`/`05 §12` carry the **O1–O14** rows + the orchestrator gap decision; the matrix has an `O` group;
  the three re-verification decisions are recorded (O12 comment-author reversal, O13 spawn preamble, O14
  `solo://` promotion); no `KNOWN-DIVERGENCES` entry was forced where there is no documented-behavior
  divergence.
- `orchestration_snapshot(project)` returns the live tree + todos + timers + leases + scratchpads + kv,
  scoped to the project, assembled purely from existing reads.
- Creating/completing a todo, arming/firing a timer, acquiring/releasing a lease, and writing a
  scratchpad each emit exactly one corresponding `DomainEvent`; the TS projection switch stays exhaustive
  (`tsc` passes).
- `crates/core` still imports no adapter crate (dependency-direction guard green); coordination write
  paths (G1–G11) are behaviorally unchanged (their existing tests stay green).

## Test plan
- **Unit (core, `MockClock`):** snapshot assembly from seeded fakes (a lead + two workers, a blocked
  todo, an armed `fire_when_idle` timer, a held lease) returns the expected projection; each mutation
  emits its event exactly once (subscribe a test `EventBus` receiver).
- **Unit (UI, Vitest):** `projection.ts` reduces each new event into the read-model store correctly;
  exhaustiveness holds.
- **Regression:** the full existing Phase 9 suite + `crates/pty/tests/orchestration.rs` stay green
  (proves reads/events are additive).

## Risks & mitigations
- **Read-model drift from write truth** → the snapshot is *derived* on read from the repos/registry, not
  a separately stored copy; never cache domain state in the projection ([`04` §2](../04-engineering-architecture-and-patterns.md)).
- **Event storms from a chatty orchestration** → events are change-notifications (ids only), not
  payloads; the UI re-queries coalesced per animation frame (orch-01+), honoring the responsiveness
  budget (CLAUDE.md §6) and bounded-everything ([`04` §8](../04-engineering-architecture-and-patterns.md)).
- **Touching frozen write semantics** → tasks add reads + emissions *beside* the existing writes only;
  G1–G11 tests are the guard.

## Effort
~2–3 days (mostly read-model assembly + event wiring; the matrix/gap record is documentation).
