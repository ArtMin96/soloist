# PRD-07 — Finish the reconciliation layer + close the actor launch-window races

Status: done
Blocked by: 02

- **Severity:** P2 (stale UI on bus lag/drop; narrow but real lost-command / orphan-on-quit races)
- **Area:** `crates/app/ui/src/store` (SignalsProvider, useLineage, useOrchestration),
  `crates/core/src/supervisor.rs`
- **Evidence:** AGENT-reported; consistent with the sprint's own design (the sprint added
  reconciliation to `useProcesses`/`useProjects` but not these). Re-verify each before coding.

## Problem
The 2026-07-13 sprint added a reconciliation backstop (`DOMAIN_RESYNC` + `useReconcile`
on-signal/on-focus) to `useProcesses` and `useProjects`, but three other snapshot-then-delta
surfaces were left without it, and two actor launch-window races remain:

1. **E7 — agent activity / signals** (`SignalsProvider.tsx:14-27`): subscribes to `onDomainEvent`
   only — no `onResync`, no focus refresh, no snapshot seed. On a dropped `AgentActivityChanged`
   during bus lag, the idle badge is **permanently stale** (edge-triggered). Also a dropped
   `ProcessRemoved` leaks that id's entries. (Broader than the known "webview-reload snapshot"
   deferral — the lag/drop path is uncovered too.)
2. **E8 — lineage & orchestration** (`useLineage.ts`, `useOrchestration.ts`): re-read on specific
   events but never call `useReconcile`. A dropped delta leaves the sidebar worker-nesting / the
   orchestration board stale until the next relevant event.
3. **C3 — illegal FSM transitions are silently dropped, untraced** (`supervisor.rs:497-509`): the
   `Err(_)` arm publishes nothing and logs nothing, so a future regression is invisible and can
   desync the registry/actor mirror.
4. **C4 — `stop()` in the launch window is lost but returns `true`** (`supervisor.rs:235-245` +
   `438-456`): the actor handle is set after `begin_launch` moves the process to `Starting` with no
   await between; a `stop`/`stack_stop` landing in that window sends nothing yet reports success.
5. **C5 — `shutdown()` can miss a mid-launch process → orphan on quit** (`supervisor.rs:402-416`):
   a `begin_launch`'d-but-not-yet-`set_handle` process is invisible to `with_live_actor` and can
   spawn its child after `shutdown` returns. Same root as C4.

## Fix approach
- **E7/E8:** give `SignalsProvider`, `useLineage`, and `useOrchestration` the same `useReconcile`
  treatment as `useProcesses` — refresh on `DOMAIN_RESYNC` and on window focus. For signals, add a
  snapshot seed (the known-deferred item: a core query/field for current agent activity) so a
  webview reload and a lag both recover. This also subsumes the deferred "agent-activity snapshot
  seed."
- **C3:** log (`tracing::warn!`) and/or emit a diagnostic event on the illegal-transition `Err`
  arm. Cheap, high-value observability.
- **C4/C5:** set the actor handle **inside the same `begin_launch` critical section** (or record a
  pending-stop flag the launching actor checks before spawning), so a stop/shutdown in the window
  is neither lost nor reports false success, and `shutdown` can't miss a mid-launch process. This
  is the single structural fix for both.

## Test plan (must fail before, pass after)
- **UI:** firing `DOMAIN_RESYNC` re-runs each store's refresh (signals/lineage/orchestration) —
  mirror the existing `useReconcile.test.tsx`. A dropped `AgentActivityChanged` followed by a
  resync restores the correct badge from the snapshot seed.
- **Core:** an illegal transition is observable (a captured tracing event / diagnostic event).
- **Core:** a `stop()` issued between `begin_launch` and `set_handle` either reaches the actor or
  is honored via the pending-stop flag — assert the process actually stops and `stop()` doesn't
  return `true` while dropping the command. A `shutdown()` racing a launch leaves no child running
  (extend the existing shutdown/reap tests, which use `FakeSpawner`).

## Acceptance
- All snapshot-then-delta stores self-heal on lag and focus. Illegal transitions are traced. No
  stop/shutdown command is silently lost in the launch window; no orphan survives quit via that
  window. `just test` + `just lint` green + the nightly soak (§8) shows no task/FD drift.

## Out of scope
The empty-pane attach race (PRD-02) and orphan PID-reuse (PRD-03), handled separately.

## Comments
Done 2026-07-15 (branch `fix/stability-audit-2026-07`, impl commit `ccfd29c`; docs/ledger commit follows). All five parts landed test-first:

- **C4/C5** — `Registry::begin_launch` now installs the actor **mailbox** under the same claim lock that moves the process to `Starting`, so a stop/shutdown in the launch window reaches the actor rather than being dropped while `stop()` still returns `true`. The join is attached after the task spawns (`attach_join`, which `abort()`s a superseded task so a close-in-window leaks no child). The actor drains a pending stop **before spawning** (`stop_is_pending`), so the process goes `Starting → Stopping → Stopped` without ever spawning a child; `shutdown` sees mid-launch entries (`with_live_actor` keys on the mailbox), stops them in place, and reaps via a **bounded** retry (`MAX_SHUTDOWN_IDLE_PASSES`). `stop`/`restart`/`shutdown` all route through one `registry.signal` primitive (the old `mailbox()` accessor was removed). Tests: `a_stop_in_the_launch_window_is_delivered_and_stops_without_spawning`, `shutdown_reaps_a_process_still_in_its_launch_window`, `begin_launch_installs_the_mailbox_before_the_join_is_attached`, `a_superseded_launch_aborts_its_orphaned_actor`.
- **C3** — `apply_transition`'s illegal-`Err` arm now `tracing::warn!`s (added `tracing` to core) instead of dropping silently. Test `an_illegal_transition_is_refused_and_traced` captures the warning with a minimal in-test subscriber and asserts no delta is published.
- **E7** — `IdleTracker::activity_snapshot` → `Facade::agent_activity` → the `agent_activity` Tauri command (+ `AgentSignal` DTO, single-sourced Rust→`domain.ts`). `SignalsProvider` seeds the idle badges on mount and re-seeds on `DOMAIN_RESYNC`/focus (`useReconcile`), reconciling the activity map to the snapshot (drops a stale/departed badge). Proven end-to-end by the real-PTY `orchestration.rs` integration test (worker classified Idle → `agent_activity` reports it) plus `SignalsProvider`/`signalStore`/`signals` unit tests. **Scope note:** the seed reconciles the agent-activity map (the ticket's named "stale idle badge"); metrics/attempts entries for a dropped `ProcessRemoved` still fold from their own deltas (metrics self-heal via the periodic tick, attempts clear on status change) and are never rendered for a removed row, so they were left as-is.
- **E8** — `useLineage` and `useOrchestration` now call `useReconcile(refresh)`; resync tests added (`useLineage.test.ts`, new `useOrchestration.test.tsx`).

**`/code-review` (Standards + Spec, parallel sub-agents):** Spec axis clean (all 5 parts implemented + tested, no scope creep, nothing wrong). Standards axis raised 3, all fixed before commit: unbounded shutdown retry → bounded (`MAX_SHUTDOWN_IDLE_PASSES`, §8); DRY dup of the try-send → routed `stop`/`restart`/`shutdown` through `registry.signal` and removed `mailbox()`; the resulting `signal(id, msg)` is now called with both `Stop` and `Restart`, so its generality is justified.

**Gates:** `just lint` exit 0 (fmt, clippy `-D warnings`, tsc, eslint, prettier, dep-direction; file-size advisory only). `just test` — **Rust 974 passed / 0 failed / 3 ignored, UI 315 passed / 63 files** (net Rust +6, UI +10). **`just soak`** (the leak gate) green: start/stop ×40 → **tasks 0→0, fds 4→4, threads 5→5**; crash-storm ×5 → tasks 1→1, fds 4→4 — no task/FD drift from the launch-window mailbox/join lifecycle. Fully headless-verified → **done**.
