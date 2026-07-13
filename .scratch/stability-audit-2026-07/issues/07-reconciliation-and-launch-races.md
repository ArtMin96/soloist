# PRD-07 — Finish the reconciliation layer + close the actor launch-window races

Status: ready-for-agent
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
