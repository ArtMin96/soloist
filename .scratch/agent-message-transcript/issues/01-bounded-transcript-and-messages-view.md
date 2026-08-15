# PRD-01 — Make agent-to-agent traffic visible: a bounded retained transcript + the Messages view (PR #167)

Status: ready-for-agent
Blocked by: none (but see the **blocking decision** below — the ticket is actionable either way, and
branch (b) materially grows it)

- **Severity:** P1 (the feature's whole point is a human watching agents coordinate; today none of
  that traffic is visible anywhere in the app)
- **Parity rows:** extends `plan/02-feature-parity-matrix.md` **O15** (authenticated live-run agent
  messaging). **There is no UI parity row for message visibility** — O5 (scratchpad panel), O6 (to-do
  board), O7 (timers panel) and O8 (wake-cycle visibility) cover the other orchestration surfaces and
  none covers messages. This ticket must **add one** (proposed **O17**); see "Doc changes" below.
- **Area:** `crates/core/src/coordination/mailbox/state.rs`,
  `crates/core/src/coordination/mailbox/vocabulary.rs`, `crates/core/src/facade/mailbox.rs`,
  `crates/core/src/facade/orchestration.rs`, `crates/core/src/orchestration.rs`,
  `crates/core/src/events.rs`, `crates/core/src/facade.rs`,
  `crates/app/ui/src/domain.ts`, `crates/app/ui/src/store/projection.ts`,
  `crates/app/ui/src/store/useOrchestration.ts`,
  `crates/app/ui/src/components/orchestration/OrchestrationPane.tsx`,
  `crates/app/ui/src/components/orchestration/MessagesPanel.tsx` (new)
- **Evidence:** VERIFIED in code (research session, 2026-08-14) against
  `feat/agent-mailbox-coordination` HEAD `f0e4993`. `git diff --stat main...HEAD -- crates/app/ui/`
  is **empty** — the mailbox backend shipped with zero frontend code.
- **Depends on:** the mailbox backend (PR #166). Ships after it, before #168/#169.

> **Line numbers are hints; symbol names are authoritative.** This ticket was written while PR #166
> was still being revised, so `crates/core/src/coordination/mailbox/*` and
> `crates/core/src/facade/mailbox.rs` were under active edit. Every citation was re-verified against
> the working tree at the time of writing, but they will drift again as #166 lands. **If a line number
> does not match, find the named function or type — the symbol is the real reference.** Everything
> outside the mailbox module (`events.rs`, `facade.rs`, `orchestration.rs`, `vocabulary.rs`, the
> frontend) was stable.

---

## BLOCKING DECISION — resolve before writing code

**How long must the transcript live?** The two branches differ in size by roughly 4×.

### (a) Survives a webview reload — in-memory. *This is the design as written below.*

The transcript lives in the `AgentMailbox` aggregate as process-local state. A webview reload
(devtools reload, `location.reload()`, a Vite HMR full refresh) does **not** restart the Rust process,
so the transcript is re-read from the snapshot and the history is intact. A **Soloist restart** clears
it, exactly as the delivery queue is cleared today.

This is consistent with the existing classification: CLAUDE.md §8 puts ephemeral state (registry,
PIDs, metrics, PTY buffers) in memory and durable state in SQLite, and
`crates/core/src/coordination/mailbox/state.rs:48` already documents the mailbox as
*"The per-run mailbox."* Messages are per-run by construction — a `ProcessId` is meaningless across
restarts, and every roster/lineage relationship the transcript renders is live-process-scoped.

**Cost:** no migration, no new port, 550–700 lines. Everything below is written for this branch.

### (b) Survives an app restart — durable.

Requires, in addition to everything below:

1. A `TranscriptRepo` port (`plan/06-codebase-blueprint-and-cleanup.md` §5.2 recipe) with a `Noop`
   default, threaded through `CorePorts` and `crates/app/src/lib.rs::build_facade`.
2. A SQLite migration in `crates/store/src/migrate.rs` plus a row module beside
   `crates/store/src/todo_rows.rs`.
3. **Reclassifying agent messages from ephemeral to durable** under CLAUDE.md §8 — a contract change
   that needs recording in `plan/05-solo-reference-and-sources.md` §12 and `KNOWN-DIVERGENCES.md`.
4. An answer to what a persisted `ProcessId` means after a restart: the senders and recipients no
   longer exist, `agent_roster` cannot resolve them, and the transcript would render dangling ids
   unless labels are denormalised into each row.

**Cost:** roughly 4× the ticket, and item 4 is a genuine design question, not just plumbing.

**Recommendation:** (a). Item 4 above is the substantive argument — a durable transcript full of dead
`ProcessId`s is not obviously more useful than no transcript, and making it useful means storing
denormalised labels, which is a second source of truth for something the registry owns.

**If the owner picks (b), stop and re-scope.** Sections 1, 2 and the test plan change materially.

---

## Problem

On `feat/agent-mailbox-coordination` a human **cannot see any agent-to-agent traffic in Soloist.**
Two independent reasons, both verified:

### 1. No UI, and no way to build one

- `git diff --stat main...HEAD -- crates/app/ui/` is empty.
- Grepping `crates/app/ui/src/` for `mailbox`, `broadcast`, `AgentMessage`, `CompletionReport`
  returns **zero** hits (the `roster` hits are the unrelated diagram/scratchpad rosters at
  `crates/app/ui/src/components/orchestration/DiagramRoster.tsx:34` and `ScratchpadRoster.tsx:34`).
- The mailbox is on the **Unix-socket IPC surface only** — seven verbs in
  `crates/app/src/ipc_server/dispatch.rs` (`AgentRoster`, `AgentMessageSend`,
  `AgentMessageBroadcast`, `AgentMessageList`, `AgentMessageGet`, `AgentMessageAcknowledge`,
  `AgentReportCompletion`). The Tauri `invoke_handler!` list at `crates/app/src/lib.rs:407-552`
  contains **no** mailbox command.
- The cause is architectural and deliberate: every mailbox method is `impl ScopedFacade<'_>`
  (`crates/core/src/facade/mailbox.rs:98`) and needs a `SessionId`. `ScopedFacade` is the
  session-bound MCP authority with no accessor onto a context
  (`crates/core/src/facade/scoped.rs:62-79`). Tauri commands hold `State<'_, Arc<Facade>>` and call
  project-scoped `Facade` methods (`crates/app/src/commands/coordination.rs:21-30`). **The UI
  structurally cannot reach the mailbox.**

### 2. Even with a UI, there would be nothing to render

`AgentMailbox::acknowledge` **removes** the pending record
(`crates/core/src/coordination/mailbox/state.rs:322`). `task_receipts`
(`state.rs:33`, `:38-44`) retains only `Task`-kind rows and **drops the body**.
`remove_process` wipes an agent's whole inbox on close (`state.rs:357`).

A panel over `AgentMailbox::list` (`state.rs:293`) would show only **undelivered** messages, each
vanishing the instant the worker acknowledges it. **No conversation history exists anywhere in the
process.**

### What a human sees today instead

- Workers appear and nest in the agent tree (`ProcessSpawned` + lineage).
- Activity badges flip as wakes land (`AgentActivityChanged`).
- Todo/completion movement on the to-do board (`TodoChanged`, published at
  `crates/core/src/facade/mailbox.rs:265-271`).
- In the recipient's terminal pane: the wake envelope — which carries **message ids, not bodies**:
  `"Addressed message(s) {ids} are waiting. Call agent_message_get, then agent_message_acknowledge
  after accepting each."` (`crates/core/src/coordination/mailbox/onboarding.rs:54-56`). The body
  travels back over the MCP socket via `agent_message_get`; the terminal never sees it.

That is the shadow of the traffic, never the traffic.

> **The e2e suite is not counter-evidence.** `e2e/specs/orchestration/agent-messaging.spec.ts:58-59`
> does assert message bodies — but via `terminalPane.waitForText` (`:42`, `:51`, `:62`), reading the
> **test fixture's own stdout**. The lead-agent fixture prints every body it receives
> (`e2e/fixtures/lead-agent/src/mailbox.rs:67`, `:112`, `:171`, `:240`, `:294`). A real agent CLI does
> not do that, and the app does not do it for them.

---

## Decisions already made (do not re-litigate)

### 1. Bounded **retained transcript**, not a payload-carrying event

Two shapes were considered.

- **Payload-carrying event, no retained state.** Precedent exists: `NotificationRaised`
  (`crates/core/src/events.rs:192-200`) and `TerminalNotification` (`:139-143`) both carry composed
  text, with the documented rationale at `:188-191` — *"A notification is transient — there is no
  record to re-query once it has been raised."* History would live only in the React hook and be lost
  on every webview reload.
- **Bounded retained log + id-only event.** ← **chosen** (owner decision, 2026-08-14). The
  conversation survives a webview reload.

**Given the retained log, the event MUST be id-only.** A payload-carrying event would put the body in
two places — the log and the delta — which is exactly the second copy
`plan/04-engineering-architecture-and-patterns.md` §2 forbids, and the UI could fold a body the log
later evicted, so the two would visibly disagree. The `NotificationRaised` rationale (*no record to
re-query*) stops applying the moment a record exists.

### 2. No new Tauri command

The transcript rides the existing `orchestration_snapshot`, already the local-authority
project-scoped read (`crates/app/src/commands/orchestration.rs:1-7`). This keeps the read on
`Facade`, never `ScopedFacade`, per CLAUDE.md §16.

### 3. Bodies never reach the event bus

A consequence of decision 1, and independently desirable. The bus fans out to every subscriber and
`Facade::subscribe()` is **public** (`crates/core/src/facade.rs:232`), so any future adapter gets the
full stream with no scope check.

For the record — the subscriber audit was done, and today's set is safe. The **complete** production
subscriber list: the Tauri forwarder (`crates/app/src/lib.rs:157-172`, forwards **everything**
unfiltered to the webview), the app-icon badge (`crates/app/src/badge.rs:34`), the notification
reactor (`crates/core/src/notify/reactor.rs:70`), the file-watch reactor
(`crates/core/src/filewatch/reactor.rs:66`), the git status watcher (`crates/core/src/git/watch.rs:127`),
the config watcher (`crates/core/src/projects/config_watch.rs:74`), the timer scheduler
(`crates/core/src/coordination/scheduler.rs:76`), the template evictor
(`crates/core/src/coordination/template_evictor.rs:45`), the restart policy
(`crates/core/src/supervisor/restart.rs:221`) and the mailbox reactor
(`crates/core/src/coordination/mailbox/reactor.rs:90`). Every one either matches specific variants
with `Ok(_) => {}` or falls through `_ => None` (`notify/reactor.rs:266`).
`crates/httpapi/src`, `crates/mcp/src` and `crates/cli/src` contain **zero** `DomainEvent`/`subscribe`
references — the HTTP API is seven pull-only `get` routes with no stream
(`crates/httpapi/src/routes.rs:36-42`). So bodies on the bus would leak nothing today. **Id-only is
chosen for the single-source-of-truth reason above, plus the public-`subscribe()` future risk — not
because of a present confidentiality hole.**

---

## Four load-bearing warnings

These would otherwise **ship green**. Read them before writing code.

### (a) `agent_message_broadcast` bypasses `send_message` — there are FOUR emit sites

The obvious implementation emits from `send_message` and stops. That silently ships **invisible
broadcasts**, and a test that only exercises a direct send stays green.

`AgentMailbox` holds only `Mutex<MailboxState>` and has **no bus handle**
(`crates/core/src/coordination/mailbox/state.rs:50-52`), so the emit cannot live in `state.rs`. It
must be at the facade layer, where `self.inner.bus` is reachable (as already used at
`crates/core/src/facade/mailbox.rs:268`). The four sites:

| # | Site | Covers |
|---|---|---|
| 1 | `crates/core/src/facade/mailbox.rs:342` — `send_message` | `Direct` + `Completion` |
| 2 | `crates/core/src/facade/mailbox.rs:166` — `agent_message_broadcast`, which calls `enqueue_many` **directly, bypassing `send_message`** | broadcast fan-out |
| 3 | `crates/core/src/facade/mailbox.rs:321` — `queue_spawned_task` (`enqueue_reserved`) | the spawn-time `Task` |
| 4 | `crates/core/src/facade/mailbox.rs:213` — `agent_message_acknowledge` | the outcome transition to `Acknowledged` |

Test 4 below is the discriminating one: it must stay green when only `send_message` is exercised.

### (b) The transcript **evicts**; the delivery queue **refuses**. Two ceilings, opposite policies, same aggregate

An implementer will be tempted to reuse `MailboxCapacityError` and collapse this into one cap. Do not.

- `MailboxCapacityError` (`crates/core/src/coordination/mailbox/vocabulary.rs:90-98`) governs
  `inboxes`. Its own doc: *"A queue ceiling that refused an enqueue without dropping an existing
  message."* Dropping a queued message would silently lose work an agent is waiting on. The **O15
  parity row demands this explicitly**: *"the … ceilings refuse overflow without dropping queued
  messages"* (`plan/02-feature-parity-matrix.md`, row O15 Verify).
- The transcript is a **display** structure. Refusing would make **a send fail because the log is
  full**, which is absurd — the traffic is the product; the record of it is the convenience.

**Invariant: a full transcript must NEVER cause a send to fail.** Test 1 asserts the send still
returned `Ok`, which is the only assertion that catches a collapse of the two policies. An
eviction-only assertion passes against the bug.

### (c) `remove_process` must EXEMPT the transcript

`AgentMailbox::remove_process` (`crates/core/src/coordination/mailbox/state.rs:357`) today wipes
an agent's inbox, onboarding, wake state and task receipts on `ProcessRemoved`
(`crates/core/src/coordination/mailbox/reactor.rs:103`). Extending it to the transcript would delete
**exactly what a human wants to read** — a closed worker's messages.

The keys differ, which makes the exemption clean: `inboxes` is keyed per-recipient `ProcessId`; the
transcript is keyed per-`ProjectId`. The process-close path and the eviction path never share a key.

The transcript's own lifecycle hook is **`ProjectRemoved`**, matching the template evictor
(`crates/core/src/coordination/template_evictor.rs:86`). Wire it in the same reactor loop
(`crates/core/src/coordination/mailbox/reactor.rs:89-111`), which currently ignores `ProjectRemoved`
via its `Ok(_) => {}` arm at `:105`.

Test 5 locks this in.

### (d) `Clock::now_unix_millis()`, never `SystemTime` — and `AgentMailbox` loses its `Default`

There is **no timestamp anywhere in the mailbox vocabulary** today. It is the one genuinely new
field, and it must come from the `Clock` port:

- Use `Clock::now_unix_millis()` (`crates/core/src/ports.rs:175`) — the documented *"persistable
  absolute time"*, where *"a mock advances it in lockstep with `now`, so those paths stay
  deterministic with no real time elapsed."*
- **Not** `Clock::now()` (`ports.rs:169`), which is monotonic and process-local — explicitly
  *"never for a persisted deadline."*
- **Not** `std::time::SystemTime`, which makes the tests non-deterministic and violates the
  pure-core rule.

**Threading required.** `AgentMailbox::new()` takes no arguments today
(`crates/core/src/coordination/mailbox/state.rs:54-57`) and is constructed at
`crates/core/src/facade.rs:224` as `Arc::new(AgentMailbox::new())`. It becomes
`AgentMailbox::new(clock.clone())`, **exactly matching every sibling coordination aggregate**:

```rust
// crates/core/src/facade.rs
leases:      Leases::new(lock_repo, clock.clone()),          // :188
timers:      Timers::new(timer_repo, clock.clone(), ...),    // :191
scratchpads: Scratchpads::new(scratchpad_repo, clock.clone()), // :199
diagrams:    Diagrams::new(diagram_repo, clock.clone()),     // :200
feedback:    Feedback::new(feedback_repo, clock.clone()),    // :205
```

`Facade` already holds `clock: Arc<dyn Clock>` at `crates/core/src/facade.rs:115`.

**Two mechanical consequences:**

1. `#[derive(Default)]` on `AgentMailbox` (`state.rs:49`) must be **removed** — it cannot hold a
   `dyn` collaborator. `MailboxState` keeps its own `Default` (`state.rs:21`).
2. All **ten** `AgentMailbox::new()` call sites in
   `crates/core/src/coordination/mailbox/state_tests.rs` (`:22`, `:42`, `:59`, `:83`, `:108`,
   `:135`, `:161`, `:180`, `:208`, `:228`) need a clock. Use `MockClock`
   (`crates/core/src/testing/clock.rs:35`). Mechanical churn, but it touches every existing mailbox
   test — budget for it.

---

## Fix approach — ordered, file by file

Each step names the `plan/06-codebase-blueprint-and-cleanup.md` §5 recipe it satisfies.

### Step 1 — vocabulary (`plan/06` §5.1: logic in the owning context, C6)

**`crates/core/src/coordination/mailbox/vocabulary.rs`** (98 lines → ~130) — add three consts beside
`MAX_PENDING_*` (`:7-15`):

```rust
/// Maximum retained transcript entries held for one project. Overflow evicts the oldest.
pub const MAX_TRANSCRIPT_ENTRIES_PER_PROJECT: usize = 512;
/// Maximum retained transcript entries held across the entire running application.
pub const MAX_TRANSCRIPT_ENTRIES: usize = 4096;
/// Maximum UTF-8 body bytes retained in one transcript entry; a longer body is truncated.
pub const MAX_TRANSCRIPT_BODY_BYTES: usize = 4 * 1024;
```

**Values are a proposal, not a measured number.** Rationale: `MAX_TRANSCRIPT_ENTRIES` deliberately
equals `MAX_PENDING_AGENT_MESSAGES` (`vocabulary.rs:13`) so the transcript can hold one full
mailbox-worth. `MAX_TRANSCRIPT_BODY_BYTES` is a quarter of `MAX_AGENT_MESSAGE_BYTES`
(16 KiB, `vocabulary.rs:7`) because retaining 4096 full-size bodies would be **64 MiB** — over the
CLAUDE.md §6 idle-RSS budget (~150 MB) on its own. At 4 KiB the worst case is 16 MiB. If the owner
wants different numbers, change them here; nothing else hard-codes a limit.

And the record type, reusing existing vocabulary rather than re-rolling it:

```rust
/// One recorded exchange, retained for display after delivery. `delivery.outcome` is updated in
/// place as the message moves, so the transcript shows live delivery state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMessageRecord {
    pub delivery: AgentMessageDelivery,
    /// Wall-clock time the exchange was recorded, from `Clock::now_unix_millis`.
    pub at_unix_millis: u64,
    /// True when the retained body was cut at `MAX_TRANSCRIPT_BODY_BYTES`. The **delivered**
    /// message is never truncated.
    pub truncated: bool,
}
```

`AgentMessageDelivery` (`vocabulary.rs:49-52`) already pairs `AgentMessage` — id, project, sender,
recipient, kind, body, todo_id (`:37-45`) — with `AgentMessageOutcome`
(`:29-33`: `Queued | WakeSubmitted | Acknowledged`). That covers sender → recipient, kind, body and
delivery outcome with **zero new vocabulary**. Only the timestamp and truncation flag are new.

Export from `crates/core/src/coordination/mailbox.rs` (the `pub use vocabulary::{...}` block) and
re-export from `crates/core/src/lib.rs:71`.

**Truncation must be UTF-8-safe.** `body.len()` is bytes; cutting mid-codepoint panics on
`String::truncate`. Use `char_indices` to find the last boundary at or below the cap.

### Step 2 — the aggregate (`plan/06` §5.1)

**`crates/core/src/coordination/mailbox/state.rs`** (439 lines — already over the ~400 smell).
**Put the transcript in its own module**, `crates/core/src/coordination/mailbox/transcript.rs`, with
`impl AgentMailbox` blocks, exactly as `onboarding.rs` and `reactor.rs` already extend the same type
from sibling files. Do not grow `state.rs`.

- Add one field to `MailboxState` (`state.rs:22-34`):
  `transcript: HashMap<ProjectId, VecDeque<AgentMessageRecord>>` plus a `transcript_count: usize`
  for the process-wide ceiling.
- Add `AgentMailbox { clock: Arc<dyn Clock> }` and change `new()` per warning (d). Remove
  `#[derive(Default)]` at `:49`.
- `pub(crate) fn record(&self, delivery: &AgentMessageDelivery)` — truncates, timestamps, pushes
  back, evicts oldest on either ceiling. **Returns nothing and cannot fail** — that is the enforcement
  of warning (b).
- `pub(crate) fn record_outcome(&self, project, id, outcome)` — updates an existing entry in place
  (used by the acknowledge site).
- `pub(crate) fn transcript(&self, project: ProjectId) -> Vec<AgentMessageRecord>` — oldest first.
- `pub(crate) fn forget_project(&self, project: ProjectId)` — the `ProjectRemoved` hook.
- **Do not touch `remove_process` (`state.rs:357`).** See warning (c).

**`crates/core/src/coordination/mailbox/reactor.rs`** — add a `ProjectRemoved` arm to the loop at
`:93-107`, calling `forget_project`. It currently swallows the event at the `Ok(_) => {}` arm (`:105`).

### Step 3 — the event (`plan/06` §5.6)

**`crates/core/src/events.rs`** — one variant, id-only, matching `TodoChanged` (`:156`) and
`ScratchpadChanged` (`:177`):

```rust
/// A recorded agent-to-agent exchange in `project` changed — queued, woken, or acknowledged. A
/// change-notification carrying ids only: the orchestration UI re-reads the snapshot rather than
/// trusting a payload, so a chatty run coalesces to one re-query per frame.
AgentMessageChanged { project: ProjectId, id: AgentMessageId },
```

`AgentMessageId` already exists (`crates/core/src/ids.rs:122`); import it into `events.rs`.

**One variant covers all four transitions**, so the UI's `SNAPSHOT_EVENTS` set grows by exactly one.

### Step 4 — the emit sites (`plan/06` §5.1)

**`crates/core/src/facade/mailbox.rs`** (403 lines — at the smell; if the additions push it past ~430,
split the transcript-recording helper into `crates/core/src/facade/mailbox_record.rs`).

Add a private helper and call it from **all four** sites in warning (a):

```rust
fn record_and_announce(&self, delivery: &AgentMessageDelivery) {
    self.inner.mailbox.record(delivery);
    self.inner.bus.publish(DomainEvent::AgentMessageChanged {
        project: delivery.message.project,
        id: delivery.message.id,
    });
}
```

- `send_message` (`:327`) — after the `enqueue`/wake block, once the final outcome is known.
- `agent_message_broadcast` (`:166`) — inside the `for delivery in &mut deliveries` loop at
  `:174-181`, after the wake attempt sets the outcome. **One call per recipient.**
- `queue_spawned_task` (`:316`) — on the `Ok` path of `enqueue_reserved`.
- `agent_message_acknowledge` (`:213`) — via `record_outcome`, so the entry flips to `Acknowledged`
  rather than being appended twice.

### Step 5 — the snapshot (`plan/06` §5.1)

**`crates/core/src/orchestration.rs`** (82 lines) — one field on `OrchestrationSnapshot` (`:66-82`),
after `kv` (`:81`):

```rust
/// Recorded agent-to-agent exchanges in the project, oldest first.
pub messages: Vec<AgentMessageRecord>,
```

**`crates/core/src/facade/orchestration.rs`** (127 lines) — one read in `orchestration_snapshot`
(`:61`): `messages: self.mailbox.transcript(project),`.

**Why this does not violate the "never a cached second copy" rule
(`plan/04-engineering-architecture-and-patterns.md` §2).** The rule forbids duplicating state another
aggregate owns, not retained state as such. `orchestration_snapshot` is already **derived on read from
owning aggregates** — it composes the registry, lineage and idle tracker into `agents`
(`crates/core/src/facade/orchestration.rs:65-87`) and reads `self.timers.list_project(project)?` live
at `:91-93`. Timers, todos, scratchpads and leases are all genuinely retained state in their own
aggregates; the snapshot reads them, it does not cache them. The transcript is exactly that shape:
**`AgentMailbox` is its single owner** and the snapshot gains one more read. Nothing holds a second
copy — which is precisely why the event must stay id-only (decision 1).

### Step 6 — the read path, frontend (`plan/06` §5.6 step 2, §5.7)

No `api.ts` change: `orchestrationSnapshot` already exists (`crates/app/ui/src/api.ts:81`) and no new
Tauri command is added.

1. **`crates/app/ui/src/domain.ts`** (1308 lines) — mirror `AgentMessage`, `AgentMessageKind`,
   `AgentMessageOutcome`, `AgentMessageRecord`; add `messages` to the `OrchestrationSnapshot` type;
   add `| { type: "AgentMessageChanged"; project: number; id: number }` to the `DomainEvent` union
   (`:216`). **This is the one TS definition** — nowhere else.
2. **`crates/app/ui/src/store/projection.ts`** (92 lines) — add the `AgentMessageChanged` case to the
   exhaustive switch.
3. **`crates/app/ui/src/store/useOrchestration.ts`** (146 lines) — five edits:
   `"AgentMessageChanged"` into `SNAPSHOT_EVENTS` (`:19-33`); `messages: AgentMessageRecord[]` on
   `OrchestrationStore` (`:35-47`); `messages: []` in `EMPTY` (`:51-59`); `messages: snap.messages`
   in the `refresh` mapping (`:76-86`); `messages: view.messages` in the return (`:136-145`).
   The existing per-frame coalescing (`:98-104`) absorbs a chatty run for free — no new throttling.

### Step 7 — the UI (`plan/06` §5.7; **`/impeccable` is MANDATORY per CLAUDE.md §5**)

**`crates/app/ui/src/components/orchestration/OrchestrationPane.tsx`** (72 lines) — a sixth `View`:
add `"messages"` to the union (`:14`), an option to `VIEW_OPTIONS` (`:16-22`), and a body case
(`:56-68`).

**New `crates/app/ui/src/components/orchestration/MessagesPanel.tsx`.**

**Closest structural model to copy: `TimersPanel`** —
`<TimersPanel timers={timers} agents={agents} project={project.id} />`
(`OrchestrationPane.tsx:68`, component at
`crates/app/ui/src/components/orchestration/TimersPanel.tsx`, 284 lines). Props-in, no `invoke`,
takes the collection **plus `agents`** for id→label lookup — exactly what a message list needs to
render "Lead → Worker 2" instead of raw `ProcessId`s.

**Not** `ScratchpadPanel`/`DiagramPanel`, whose roster+editor split is for editable documents. A
transcript is read-only.

Proposed props:

```tsx
interface MessagesPanelProps {
  messages: AgentMessageRecord[];
  agents: AgentNode[];   // id → label, exactly as TimersPanel uses it
  project: number;
}
```

No business logic in the component; any grouping/sorting goes in a pure, unit-tested module under
`crates/app/ui/src/store/` (the `crates/app/ui/src/store/timerPanel.ts` precedent).

---

## UI design decisions for `/impeccable`

CLAUDE.md §5 makes the skill mandatory for UI work; `PRODUCT.md` and `DESIGN.md` are the design source
of truth. **These are open questions, not decisions — do not settle them by writing code:**

1. **Single chronological stream vs per-agent panes.** The inspiration sketch
   (`gemini-code-1786538380867.md:506-549`) shows one pane per agent, each filtered to what that agent
   sent and received. `OrchestrationPane`'s established idiom is **one body per view**. These pull
   opposite ways and it is a genuine UX call.
2. **The evicted-history boundary.** A transcript that silently begins mid-conversation is
   misleading. It needs an explicit "earlier messages dropped" affordance at the head of the list.
3. **Truncated-body indication** — the `truncated` flag needs a visual treatment.
4. **Kind differentiation** (`Direct` / `Task` / `Completion`) and **outcome state**
   (`Queued` / `WakeSubmitted` / `Acknowledged`). `crates/app/ui/src/lib/status.ts` is the
   single-map precedent for that kind of mapping — do not scatter conditionals through the component.
5. **Virtualization.** Up to 512 entries per project; CLAUDE.md §6 requires long lists be virtualized
   and the surface hold ~60 fps under chatty processes.
6. **Empty state** — what the view says before any traffic exists.

---

## Test plan (must fail before, pass after)

Placement per CLAUDE.md §16. `crates/core/src/facade/mailbox_tests.rs` is already **754 lines** — do
**not** grow it. Attach a new file from the new transcript module:

```rust
#[cfg(test)]
#[path = "transcript_tests.rs"]
mod tests;
```

Facade-level cases (3, 4) go in a new `crates/core/src/facade/mailbox_transcript_tests.rs`, attached
the same way from `crates/core/src/facade/mailbox.rs`.

Fixtures: `MockClock` (`crates/core/src/testing/clock.rs:35`) for every timestamp path;
`crates/core/src/testing/fixtures.rs` for façade construction.

Every case asserts an **observable outcome**, never a call shape (CLAUDE.md §15). Since the code is
new, "it doesn't compile yet" is **not** proof — for each row, break the fix, watch it redden, restore.

| # | Test | Asserts (observable) | Reddens when |
|---|---|---|---|
| 1 | `an_overflowing_transcript_evicts_the_oldest_and_the_send_still_succeeds` | Enqueue past `MAX_TRANSCRIPT_ENTRIES_PER_PROJECT`; the oldest entry is absent, the newest present, length capped — **and every send returned `Ok`** | Change the transcript overflow from evict-oldest to returning `MailboxCapacityError`. **The send-succeeded assertion is the only one that reddens** — an eviction-only assertion passes against the bug. This is warning (b) |
| 2 | `a_direct_send_is_recorded_and_announced` | After `agent_message_send`, the project transcript holds one entry with the right sender/recipient/kind/body, and one `AgentMessageChanged` carrying that `AgentMessageId` was published | The emit or the record at `facade/mailbox.rs:342` is dropped |
| 3 | `a_broadcast_records_and_announces_one_entry_per_recipient` | Broadcast to **two** related recipients → **two** transcript entries and **two** `AgentMessageChanged` events | **Delete the record+emit in `agent_message_broadcast` (`facade/mailbox.rs:166`) only.** Test 2 stays green — which is exactly the invisible-broadcast bug of warning (a) |
| 4 | `a_transcript_is_scoped_to_its_project` | Two projects; a message in A → A's `orchestration_snapshot().messages` has one, B's is empty | Drop the `project` key in `AgentMailbox::transcript` |
| 5 | `a_record_survives_the_recipient_closing` | Send, then `remove_process(recipient)` → the inbox is gone, **the transcript entry remains** | Extend `remove_process` (`state.rs:357`) to wipe the transcript. This is warning (c) |
| 6 | `removing_a_project_forgets_its_transcript` | Record in A and B, publish `ProjectRemoved{A}` → A's transcript is empty, B's intact | The `ProjectRemoved` arm is missing from the reactor loop (`reactor.rs:93-107`) |
| 7 | `the_outcome_transitions_are_visible_in_the_transcript` | Enqueue → `Queued`; wake → `WakeSubmitted`; acknowledge → `Acknowledged`, **entry still present** | Stop calling `record_outcome` from `agent_message_acknowledge` (`facade/mailbox.rs:213`), or append a second entry instead of updating in place |
| 8 | `an_oversized_body_is_truncated_in_the_record_but_delivered_intact` | Body > `MAX_TRANSCRIPT_BODY_BYTES` → retained body capped, `truncated == true`, **and `agent_message_get` returns the full body** | Truncate the delivered `AgentMessage` instead of only the record. Also: use a multi-byte body so a naive `String::truncate` panics on a codepoint boundary |
| 9 | `a_record_timestamps_from_the_clock_port` | With `MockClock`, `at_unix_millis` equals the mocked value; `advance` moves the next record's stamp | Swap `clock.now_unix_millis()` for `SystemTime::now()` — the assertion on the exact mocked value reddens because no real time passes. This is warning (d) |

### Frontend

| File | Test |
|---|---|
| `crates/app/ui/src/store/useOrchestration.test.tsx` | An `AgentMessageChanged` event triggers exactly one snapshot re-read; a burst of N coalesces to one per frame. Reddens if the type is left out of `SNAPSHOT_EVENTS` |
| `crates/app/ui/src/components/orchestration/MessagesPanel.test.tsx` | Renders sender and recipient **labels** (not raw ids) by joining `agents`; shows kind and outcome; shows the truncation and evicted-history affordances once `/impeccable` settles their shape. Reddens if the panel renders `ProcessId` numbers |

### E2E

`e2e/specs/orchestration/agent-messaging.spec.ts` (68 lines) currently proves the exchange **only via
the fixture's own stdout** in the terminal pane (`:42`, `:51`, `:62`). Extend it — or add a sibling
spec — to open the Messages view and assert the bodies appear **there**, in the app's own UI. That is
the assertion which distinguishes this ticket from a fixture that prints its own mail.

---

## Acceptance — Done when

- [ ] A direct send, a broadcast to N recipients, a spawn-time task, and an acknowledgement each
      appear in the project's transcript and each publish `AgentMessageChanged`.
- [ ] A human can open the Messages view in the Orchestration pane and **read the body of a message
      one agent sent another**, live, without reading a terminal pane.
- [ ] The transcript survives a webview reload (branch (a)) — or an app restart, if the owner picked
      branch (b), in which case this ticket was re-scoped first.
- [ ] Overflow **evicts the oldest and never fails a send** (test 1).
- [ ] A closed worker's messages remain readable (test 5); a removed project's are forgotten (test 6).
- [ ] **No message body is ever published on the event bus** — the event carries `project` + `id`
      only. Grep `crates/core/src/events.rs` for `body` and confirm the only hits are the pre-existing
      `TerminalNotification` and `NotificationRaised`.
- [ ] No new Tauri command was added; the read rides `orchestration_snapshot`.
- [ ] Timestamps come from `Clock::now_unix_millis()`; no `SystemTime` anywhere in the mailbox
      module (test 9).
- [ ] The UI half went through `/impeccable` (CLAUDE.md §5) with the six decisions above resolved and
      recorded.
- [ ] Every test above has been **observed failing** against the unfixed behaviour (break the fix,
      watch it redden, restore) — a test never seen red is unproven.
- [ ] `just lint` and `just test` exit 0.
- [ ] `PROGRESS.md` updated per CLAUDE.md §10.

### Doc changes that are part of this ticket

- [ ] `plan/02-feature-parity-matrix.md` — **add the missing UI row** (proposed **O17**), since O5/O6/
      O7/O8 cover the other orchestration surfaces and none covers messages. Suggested Verify: *"Two
      spawned agents exchange a direct message and a broadcast; both bodies are readable in the
      Messages view without opening a terminal; a closed worker's messages remain readable; an
      overflowing transcript evicts oldest without failing a send."*
- [ ] `plan/02-feature-parity-matrix.md` row **O15** — note that the ceilings it names
      (*"refuse overflow without dropping queued messages"*) govern the **delivery queue**, and that
      the transcript is a separate structure with the opposite policy. Without this the two read as a
      contradiction.
- [ ] `plan/05-solo-reference-and-sources.md` §12 — a gap row: Solo documents no agent-message
      transcript UI, so the retained log, its ceilings, its eviction policy, and the Messages view are
      a **clean-room Soloist decision**. Record it; do not claim Solo behaviour.
- [ ] `KNOWN-DIVERGENCES.md` — the same, if the §12 row implies a divergence.
- [ ] `plan/orchestrator/README.md` — add the new row alongside its O-row siblings.

---

## Size estimate

**Branch (a): 550–700 inserted lines** — roughly 250 production (≈150 Rust, ≈100 TS/TSX), 300–400
tests (including the ten mechanical `AgentMailbox::new()` call-site updates in
`crates/core/src/coordination/mailbox/state_tests.rs`), 40 docs. **No schema change, no migration.**

**Branch (b) adds** a port + `Noop`, a `CorePorts` field and composition-root wiring, a SQLite
migration and row module, denormalised labels, and the ephemeral→durable reclassification: roughly
**4× the above**, plus an unresolved design question (dead `ProcessId`s after restart).

---

## Out of scope

- **Any way for the UI to *send* a message.** This ticket is read-only visibility. A human composing
  a message to an agent would need a new `Facade` write and a Tauri command, and raises an identity
  question (what `sender` does a human get?) that is not answered here.
- **Durable persistence** unless the owner picks branch (b) — in which case re-scope first.
- **The roster as a UI surface.** `agent_roster` stays MCP-only; the agent tree
  (`OrchestrationTree`) already renders live lineage for the human.
- **The trust-request affordance** — that is PR #169.
- **`spawn_process`** — that is PR #168.
