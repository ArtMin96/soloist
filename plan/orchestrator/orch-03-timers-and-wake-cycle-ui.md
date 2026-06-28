# Orchestrator Phase O3 — Timers, Fire-when-idle & Wake-Cycle (C6/C4 + Tauri + UI)

**Goal:** Surface the **headline orchestration behavior** — token-free waiting. Show a lead's armed
**timers**: a plain `timer_set`, and especially `timer_fire_when_idle(IdleMode::Any/All)` with its
`waiting_on` workers and a **max-wait countdown** (the demo's "28 minutes left"), plus a preview of the
**injected-turn `body`**. Then make the **wake** legible: when the watched workers go idle (or the
backstop elapses) the scheduler delivers the `body` to the lead **as a fresh user turn**, and the UI
shows that arrival and the timer leaving the panel ([README](README.md) §1; video `WAKGhlzpYgs`).

**Delivers:** O7, O8. **Architecture:** presentational panel + a lead "timeline" over orch-00's
read-model + events; the wake is the **existing** `TimerScheduler` delivering `body` via
`Supervisor::write_stdin(body + "\r")` ([`06` §4 Scheduler row](../06-codebase-blueprint-and-cleanup.md),
G7–G9) — observed, not re-implemented.

## Scope
**In:** a **timers panel** per lead (armed timers, `FireCond`, `waiting_on`, live countdown to the
max-wait deadline, `body` preview, `already_idle` at-arm state); **wake-cycle visibility** (a `TimerFired`
event → the delivered `body` appears on the lead's terminal/timeline; the timer disappears); manage a
timer the UI is permitted to (`cancel`/`pause`/`resume`) routed to existing `Facade` methods. **Out:**
arming timers *from* the UI as a primary flow (timers are agent-driven; the UI observes + offers
cancel/pause/resume); persistence changes (timers are process-owned/ephemeral by design); summarizing
worker output (E6, `later`/OFF).

## Why this is the centerpiece
Fire-when-idle is the mechanism that lets a lead **end its turn and sleep** instead of busy-polling, then
be woken as a fresh turn — the single idea the demo is built around. The behavior is `Verified`
(`crates/pty/tests/orchestration.rs`); what's missing is any way for a human to *see* a lead waiting,
on whom, for how long, and to *see* it wake. The countdown matters because the backstop (**default 1 h,
24 h ceiling**, [`05` §12](../05-solo-reference-and-sources.md)) guarantees a stuck worker can never
block forever — surfacing it turns an invisible safety net into an observable one.

## Tasks
1. **Timer reads + events in the model (O7):** ensure orch-00's snapshot carries each armed timer's
   `FireCond` (`At`/`WhenIdleAny`/`WhenIdleAll`), `waiting_on` (watched-but-not-yet-idle processes),
   absolute `deadline` (the max-wait backstop), and `body`; and that `TimerArmed`/`TimerFired`/
   `TimerCleared` events fire (orch-00 Task 4). The countdown is computed UI-side from `deadline` and the
   `Clock` — **no per-second backend event** ([`04` §8 rate-limit](../04-engineering-architecture-and-patterns.md)).
2. **Tauri bridge ([`06` §5.5](../06-codebase-blueprint-and-cleanup.md)):** thin commands for the
   timer reads and for `timer_cancel`/`_pause`/`_resume` (the management subset, **scoped to the bound
   owner** — a caller manages only its own timers, [`05` §12](../05-solo-reference-and-sources.md)),
   each a one-line route to the existing `Facade` method. Confirm event/IPC APIs via the `tauri-*`
   skills + official docs (CLAUDE.md §4/§5).
3. **Timers panel (O7, [`06` §5.7](../06-codebase-blueprint-and-cleanup.md)):** per lead, list armed
   timers; for a `fire_when_idle`, show **`waiting_on`** (which workers we're blocked on) and a **live
   countdown** to the deadline; show the **`body` preview** (what the lead will be told on wake) and the
   `already_idle` flag if the condition was met at arm time. Pause shows frozen remaining time; resume
   re-arms it (pause/resume semantics are the core's, [`05` §12](../05-solo-reference-and-sources.md)).
4. **Wake-cycle visibility (O8):** on `TimerFired`, surface the delivered `body` on the **lead** — it
   arrives as a real fresh turn in the lead's PTY (the scheduler already writes it via
   `Supervisor::write_stdin`); the UI marks the wake on a small **orchestration timeline** (armed →
   waiting on N → fired → delivered) and removes the timer from the panel. The UI **does not** inject the
   turn — it observes the existing delivery (one behavior, one path, [`04` §2](../04-engineering-architecture-and-patterns.md)).
   **Wake-reason prefix (the demo's `[Solo timer #id] [wait for all: all watched idle: …]` header):** the
   scheduler prepends a **compact, clean-room** reason line to the delivered body — the timer id and
   whether it fired because **all `waiting_on` went idle** vs the **max-wait backstop elapsed** (and which
   processes) — so the **woken agent itself** (not only the UI) can tell "all peers finished" from "I was
   timed out." One bounded format string in `core` next to the existing `deliver` (G7/G8); the headless
   `crates/pty/tests/orchestration.rs` assertion updates from the bare body to `body`-with-prefix. Record
   this small delivery refinement in [`05` §12](../05-solo-reference-and-sources.md).
5. **`/impeccable` pass (CLAUDE.md §5):** design the panel + timeline through `/impeccable`; the countdown
   and wake must animate with intent and a reduced-motion fallback; calm density; match the demo's *feel*,
   not its assets (CLAUDE.md §9). Pair with `webapp-testing`.

## Interfaces
```rust
enum FireCond { At(Instant), WhenIdleAny(Vec<ProcessId>), WhenIdleAll(Vec<ProcessId>) }
struct TimerView { id: TimerId, owner: ProcessId, fire: FireCond, waiting_on: Vec<ProcessId>, deadline: UnixMillis, body: String, already_idle: bool }
// wake is the EXISTING scheduler path — UI only observes TimerFired + the lead's PTY:
// TimerScheduler --(on fire)--> Supervisor::write_stdin(owner, body + "\r")  // [06 §4], G7–G9
```

## Acceptance criteria
- An armed `timer_fire_when_idle(All)` shows its **`waiting_on`** workers and a **countdown** to the
  max-wait deadline; pausing freezes the remaining time and resuming re-arms it.
- When all watched workers go idle, the timer **fires**: the `body` appears on the **lead's** terminal as
  a fresh turn (existing delivery), **prefixed with a compact wake-reason header** (timer id + all-idle vs
  backstop) so the agent knows why it woke; the orchestration timeline marks the wake, and the timer leaves
  the panel — with **no UI-side injection**.
- A timer that hits its **backstop** (max-wait) fires the same way; the countdown reaching zero is
  observable, not a silent hang.
- Management actions are scoped to the owner; the UI never fires/clears another process's timer; gates green.

## Test plan
- **Unit (UI, Vitest):** the panel derives `waiting_on` + countdown from a `TimerView` + `Clock`; the
  timeline reduces armed→waiting→fired→delivered from the event sequence; pause/resume render states.
- **Integration:** the management Tauri commands route to the existing `Facade` timer methods and return
  their scoped results (a non-owner cancel is refused by the core, surfaced by the UI).
- **Playwright e2e:** arm a `fire_when_idle(All)` over two stub workers (via a scripted lead), assert the
  panel shows `waiting_on` + countdown, drive the workers to idle, and assert the `body` lands on the
  lead pane and the timer clears. (Reuses the `crates/pty/tests/orchestration.rs` pattern as the driver.)

## Risks & mitigations
- **Per-second backend events would flood the bus** → the countdown is computed UI-side from the
  `deadline`; the backend emits only arm/fire/clear ([`04` §8](../04-engineering-architecture-and-patterns.md)).
- **"Idle but not done" false wake (D-5)** → the UI shows *why* it woke (all `waiting_on` idle vs backstop
  elapsed) so the lead/user can judge; idle never auto-acts, it only triggers the agent's own next turn.
- **Timers vanishing after a restart looks like a bug** → timers are process-owned and **cleared on
  launch reconcile** by design ([`05` §12](../05-solo-reference-and-sources.md)); the panel states "live,
  this run" so the ephemerality is intentional, not a loss (only scratchpads/todos/kv survive, G11).
- **Fired timer is one-shot/best-effort** → if the owner closed before fire, delivery is dropped by
  design; the timeline shows "owner gone" rather than implying success ([`05` §12](../05-solo-reference-and-sources.md)).

## Effort
~4–5 days (the countdown/timeline + wake observation + motion design + Playwright driver).
