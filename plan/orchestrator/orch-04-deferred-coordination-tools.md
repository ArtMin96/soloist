# Orchestrator Phase O4 — Deferred Coordination Tools & Live Agent Messaging (C2/C4/C6/C8 + MCP)

**Goal:** Land the two **tracked deferrals** an orchestration occasionally needs, each blocked only on a
**security design** that this phase does first: `spawn_process` (spawn an *arbitrary terminal command*
over MCP, vs the existing known-agent `spawn_agent`) with its **trust treatment**, and cross-project
`scratchpad_transfer` / `todo_transfer` with **cross-scope authorization**. Both tool *names* are
documented for Solo ([`05` §7](../05-solo-reference-and-sources.md)); their schemas + safety semantics
are ours and were explicitly deferred ([`05` §8/§12](../05-solo-reference-and-sources.md), `PROGRESS.md`).
This phase also adds Soloist's clean-room live-run mailbox: authenticated lineage-root messaging,
idle-gated wake submission, and atomic durable worker completion reports.

**Delivers:** O9, O10, **O13**, O15, O16. **Architecture:** new MCP tools
as thin handlers over **new `Facade` behavior**, following the add-an-MCP-tool recipe ([`06` §5.3](../06-codebase-blueprint-and-cleanup.md));
trust + scope enforced **in the core** ([`04` §12](../04-engineering-architecture-and-patterns.md)). Invoke
`mcp-builder` + confirm against `modelcontextprotocol.io` / `code.claude.com/llms.txt` / `rmcp` docs
before writing (CLAUDE.md §5).

**Current split:** O13 is implemented for `spawn_agent`. `spawn_process` remains future O9 work, so
O13 is intentionally partial only for that second spawn surface.

**Note on O13's independence:** onboarding (Task 6) is **not** gated on the arbitrary-spawn trust design
(Tasks 1–2). It applies now to the already-built `spawn_agent` and reuses O15's bounded wake path. The
`spawn_process` leg remains partial until future O9 lands; that does not hold back `spawn_agent` or
widen command authority.

## Scope
**In:** the trust-treatment design + implementation for `spawn_process`; the cross-scope authorization
design + implementation for `scratchpad_transfer` / `todo_transfer`; their clean-room JSON Schemas; the
spawn onboarding/task path; live lineage roster; addressed direct/group messages; bounded retrieve/ack;
atomic completion reporting; tests + the gap-decision records. **Out:** any UI (these are agent-facing MCP tools; the orch-01/02 panels
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
6. **Spawn onboarding + optional first task (O13, [`06` §5.1](../06-codebase-blueprint-and-cleanup.md)):**
   after `spawn_agent` records lineage, queue one reusable clean-room orchestration briefing by default
   (`include_agent_instructions: true`). The caller may opt out. An optional `prompt` queues a bounded
   addressed `Task`; its `todo_id` correlation is optional, and it never enters provider arguments or
   startup input. Wait for the worker's first C4 `Idle` transition, then use the O15 semantic
   wake path to submit one compact envelope naming the pending task ids and the coordination primitives.
   The worker retrieves and acknowledges the task through MCP. Keep one briefing source in `core`; do
   not duplicate O13 as another parity row. Apply the same contract to `spawn_process` only when O9 is
   implemented.
7. **Authenticated live-run roster + mailbox (O15):** expose `agent_roster`, `agent_message_send`,
   `agent_message_broadcast`, `agent_message_list`, `agent_message_get`, and
   `agent_message_acknowledge` through `ScopedFacade`. Derive project, sender, and lineage root from the
   authenticated bound session. The roster contains only live agents sharing that root. Messages are
   ephemeral, ordered, and bounded by the core constants: **16 KiB per message, 64 pending per
   recipient, 1,024 pending per project, 4,096 pending process-wide, and 16 MiB of pending payload
   process-wide**. Retained lineage edges keep surviving siblings in one authorization root when an
   ancestor closes, though only live agents appear in the roster. Direct and group sends refuse unrelated/cross-project
   recipients. A group send checks all capacity before enqueuing any copy. On an `Idle` event, submit a
   compact wake envelope through `try_submit_turn`; `wake_submitted` records only PTY-channel acceptance.
   The recipient must retrieve and acknowledge the message before it leaves the inbox. Spawn Tasks,
   debate, direct/group sends, and acknowledgement work without a todo; optional `todo_id` only correlates
   live exchange to durable board work.
8. **Atomic completion report (O16):** add `agent_report_completion(todo_id, summary)` on
   `ScopedFacade`. One store transaction applies the existing blocker-gated completion and appends one
   result comment whose author comes from the bound worker. Retrying returns that same record. Queue an
   ephemeral `Completion` notice to the live parent only after the durable commit; missing parent,
   mailbox capacity, or deferred PTY wake cannot roll back or duplicate the durable result.

## Interfaces
```rust
impl Facade {
  // trust-gated, scoped — same guarantee as a manual command start (04 §12):
  async fn spawn_process(&self, scope: ProjectId, owner: ProcessId, command: SpawnSpec) -> Result<ProcessId, SpawnRefused>;
  // authorized only when the caller is scope-authenticated to BOTH projects (extends F13):
  fn todo_transfer(&self, from: ProjectId, to: ProjectId, id: TodoId, caller: ProcessId) -> Result<TodoId, TransferRefused>;
  fn scratchpad_transfer(&self, from: ProjectId, to: ProjectId, id: ScratchpadId, caller: ProcessId) -> Result<ScratchpadId, TransferRefused>;
}

impl Supervisor {
  // Normalizes trailing CR and submits once; write_stdin remains raw.
  fn try_submit_turn(&self, worker: ProcessId, body: Vec<u8>) -> Result<bool>;
}

impl ScopedFacade<'_> {
  fn agent_roster(&self) -> Result<Vec<AgentRosterEntry>>;
  fn agent_message_send(&self, recipient: ProcessId, body: String, todo: Option<TodoId>) -> Result<AgentMessageDelivery>;
  fn agent_message_acknowledge(&self, message: AgentMessageId) -> Result<AgentMessageDelivery>;
  fn agent_report_completion(&self, todo: TodoId, summary: String) -> Result<CompletionReport>;
}
```

## Acceptance criteria
- `spawn_process` of a **trusted** command in the caller's scope creates+starts it (bound, lineage-visible);
  an **untrusted** variant is **refused**, and a **cross-project** target is **refused** — by the core,
  for every adapter.
- `todo_transfer` to a project the caller is scope-authenticated for moves the todo preserving
  comments/completion and clearing blockers/locks (documented semantics); a transfer to an
  **unauthorized** project is refused (`ForeignProject`).
- **(O13)** A `spawn_agent` worker receives default-on reusable instructions after its first idle
  transition; opting out suppresses them. An optional prompt is a retrievable/acknowledgeable `Task`
  without requiring a todo, never a CLI argument or startup paste. The `spawn_process` leg is accepted
  with O9 when that future tool lands.
- **(O15)** Only authenticated live lineage-root members can exchange messages, with or without an
  optional todo correlation. Every mailbox limit is enforced without dropping existing records;
  acknowledgement removes the addressed record. A
  `wake_submitted` outcome makes no claim that the agent retrieved or acted on the payload.
- **(O16)** Completion and its authored result are one all-or-nothing durable change, repeated reporting
  is idempotent, and parent-notification failure cannot change that durable outcome.
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
- **(O13/O15)** a `spawn_agent` with default instructions and an optional prompt queues before readiness,
  writes nothing to the PTY until the child becomes idle, then submits one compact wake; the worker
  retrieves and acknowledges its no-todo `Task`. Opt-out omits the briefing. Repeat for `spawn_process`
  when O9 lands.
- **(O15)** roster/scope/authentication, direct and all-other-members broadcast, ordered list/get/ack,
  all three capacity refusals, atomic broadcast refusal, idle-deferred wake, and process-removal cleanup.
- **(O16)** store-failure atomicity, blocker refusal, one authored result on retry, parent-gone/full-mailbox
  success, and later idle wake where a notification was queued.
- **Regression:** existing `spawn_agent`, todo/scratchpad, and `crates/pty/tests/orchestration.rs` stay green.

## Risks & mitigations
- **Arbitrary spawn = the biggest new attack surface** → reuse the existing trust gate + scope auth
  unchanged; *no* new bypass; refuse-by-default; the decision is recorded before code (CLAUDE.md §9/§12).
- **Transfer leaking content across project boundaries** → require scope-auth to **both** ends; default
  refuse; never widen scope silently ([`04` §12](../04-engineering-architecture-and-patterns.md)).
- **Treating PTY acceptance as delivery** → expose `queued` / `wake_submitted` / `acknowledged` as
  separate states; only acknowledgement removes the message.
- **Busy-agent input corruption** → queue payloads in the mailbox and submit only a compact wake after
  C4 reports `Idle`; never type into an active composer or permission prompt.
- **Notification failure undoing completed work** → commit the todo + authored result first and keep
  notification outside the transaction as best-effort ephemeral state.
- **Scope creep into the FS file-io deferrals** → explicitly out of scope; `_save_to_file`/`_load_from_file`
  stay deferred behind their own security pass ([`05` §12](../05-solo-reference-and-sources.md)).

## Effort
~5–7 days (design-first security work dominates; the implementations are small over existing C2/C6; the
O13 onboarding reuses O15's mailbox/wake path; O16 composes existing todo writes into one transaction).
