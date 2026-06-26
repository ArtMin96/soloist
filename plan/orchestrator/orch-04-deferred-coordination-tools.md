# Orchestrator Phase O4 — Deferred Coordination Tools (C6/C8 + MCP)

**Goal:** Land the two **tracked deferrals** an orchestration occasionally needs, each blocked only on a
**security design** that this phase does first: `spawn_process` (spawn an *arbitrary terminal command*
over MCP, vs the existing known-agent `spawn_agent`) with its **trust treatment**, and cross-project
`scratchpad_transfer` / `todo_transfer` with **cross-scope authorization**. Both tool *names* are
documented for Solo ([`05` §7](../05-solo-reference-and-sources.md)); their schemas + safety semantics
are ours and were explicitly deferred ([`05` §8/§12](../05-solo-reference-and-sources.md), `PROGRESS.md`).

**Delivers:** O9, O10. **Architecture:** new MCP tools as thin handlers over **new `Facade` behavior**,
following the add-an-MCP-tool recipe ([`06` §5.3](../06-codebase-blueprint-and-cleanup.md)); trust + scope
enforced **in the core** ([`04` §12](../04-engineering-architecture-and-patterns.md)). Invoke
`mcp-builder` + confirm against `modelcontextprotocol.io` / `code.claude.com/llms.txt` / `rmcp` docs
before writing (CLAUDE.md §5).

## Scope
**In:** the trust-treatment design + implementation for `spawn_process`; the cross-scope authorization
design + implementation for `scratchpad_transfer` / `todo_transfer`; their clean-room JSON Schemas; tests
+ the gap-decision records. **Out:** any UI (these are agent-facing MCP tools; the orch-01/02 panels
*reflect* their effects); the scratchpad free-form/file-io deferrals (`_save_to_file`/`_load_from_file`
need their own project-root FS-scoping pass — keep deferred, [`05` §12](../05-solo-reference-and-sources.md)).

## Why these were deferred (and the blocker to clear)
- **`spawn_process`** lets an agent start an **arbitrary command**, not a vetted agent tool — so it is
  trust-sensitive in a way `spawn_agent` is not. It was deferred *"design its trust treatment first…
  don't pull forward"* (`PROGRESS.md`; [`05` §8](../05-solo-reference-and-sources.md)).
- **`*_transfer`** moves a todo/scratchpad **across projects**, which raises the same cross-scope question
  the F13 binding model answers for *acting* but not yet for *moving content* ([`05` §12](../05-solo-reference-and-sources.md);
  `D-6`). The blocker gate (G4) never depended on transfer.

## Tasks
1. **Design the `spawn_process` trust treatment (O9, gap → [`05` §12](../05-solo-reference-and-sources.md)):**
   decide and record how an arbitrary spawned command is trust-gated. Anchor on the existing trust gate
   (commands are `Untrusted` until the user confirms, per (project, command-variant hash);
   [`05` §4](../05-solo-reference-and-sources.md)) and the F13 scope model (`SO_PEERCRED`→pgid,
   `D-6`): a `spawn_process` must run **in the caller's effective project scope** and the spawned command
   variant must be **trusted there**, else it is refused — the same guarantee a manual command start
   gets, enforced in the core for every adapter. Record the decision before coding.
2. **Implement `spawn_process` (O9, [`06` §5.3](../06-codebase-blueprint-and-cleanup.md)):** add the
   `Facade` behavior (create+start a `Terminal`/`Command` subtype in scope via C2, honoring the trust
   gate) first (§5.1), then a thin MCP handler that parses a clean-room schema and routes to it. It binds
   the spawned process like `spawn_agent` does (`SOLOIST_PROCESS_ID`) so lineage (orch-01) and
   coordination attach correctly. No domain logic in the handler.
3. **Design cross-scope transfer authorization (O10, gap → [`05` §12](../05-solo-reference-and-sources.md)):**
   decide how a transfer between projects is authorized — the caller must be **bound/scope-authenticated
   to both** the source and the target project (extend the F13 model), or the transfer is refused
   (`ForeignProject`). Preserve the documented transfer semantics (todo transfer keeps comments/completion,
   clears blockers/locks; [`05` §7](../05-solo-reference-and-sources.md)). Record the decision.
4. **Implement `scratchpad_transfer` / `todo_transfer` (O10, [`06` §5.3](../06-codebase-blueprint-and-cleanup.md)):**
   `Facade` behavior over the existing repos (move the durable aggregate to the target project, applying
   the documented field rules), then thin MCP handlers with clean-room schemas. Revision/identity rules
   stay the repos' (G2/G3).
5. **Safety + schemas (O9/O10, [`04` §12](../04-engineering-architecture-and-patterns.md)):** every action
   honors the trust gate + effective scope **in the core**; document each tool's clean-room JSON Schema
   ([`05` §12](../05-solo-reference-and-sources.md) "MCP param schemas"); update the MCP tool-count guard.

## Interfaces
```rust
impl Facade {
  // trust-gated, scoped — same guarantee as a manual command start (04 §12):
  async fn spawn_process(&self, scope: ProjectId, owner: ProcessId, command: SpawnSpec) -> Result<ProcessId, SpawnRefused>;
  // authorized only when the caller is scope-authenticated to BOTH projects (extends F13):
  fn todo_transfer(&self, from: ProjectId, to: ProjectId, id: TodoId, caller: ProcessId) -> Result<TodoId, TransferRefused>;
  fn scratchpad_transfer(&self, from: ProjectId, to: ProjectId, id: ScratchpadId, caller: ProcessId) -> Result<ScratchpadId, TransferRefused>;
}
```

## Acceptance criteria
- `spawn_process` of a **trusted** command in the caller's scope creates+starts it (bound, lineage-visible);
  an **untrusted** variant is **refused**, and a **cross-project** target is **refused** — by the core,
  for every adapter.
- `todo_transfer` to a project the caller is scope-authenticated for moves the todo preserving
  comments/completion and clearing blockers/locks (documented semantics); a transfer to an
  **unauthorized** project is refused (`ForeignProject`).
- Each new tool has a documented clean-room JSON Schema; the tool-count guard is updated; the trust/scope
  decisions are recorded in [`05` §12](../05-solo-reference-and-sources.md) (and `KNOWN-DIVERGENCES` if a
  documented behavior is diverged).

## Test plan
- **Unit (core, `MockClock`):** trust-gate refusal for an untrusted `spawn_process`; scope refusal for a
  cross-project spawn/transfer; transfer field-preservation (comments/completion kept, blockers/locks
  cleared).
- **Integration (MCP over stdio, headless — the Phase 8 harness):** a scripted client spawns a trusted
  command and observes it in the app event stream; an untrusted/cross-project call is refused; a transfer
  honors/refuses scope. Action tools mutate real state.
- **Regression:** existing `spawn_agent`, todo/scratchpad, and `crates/pty/tests/orchestration.rs` stay green.

## Risks & mitigations
- **Arbitrary spawn = the biggest new attack surface** → reuse the existing trust gate + scope auth
  unchanged; *no* new bypass; refuse-by-default; the decision is recorded before code (CLAUDE.md §9/§12).
- **Transfer leaking content across project boundaries** → require scope-auth to **both** ends; default
  refuse; never widen scope silently ([`04` §12](../04-engineering-architecture-and-patterns.md)).
- **Scope creep into the FS file-io deferrals** → explicitly out of scope; `_save_to_file`/`_load_from_file`
  stay deferred behind their own security pass ([`05` §12](../05-solo-reference-and-sources.md)).

## Effort
~4–6 days (design-first security work dominates; the implementations are small over existing C2/C6).
