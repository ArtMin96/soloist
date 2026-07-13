# PRD-04 — Notification toggles must actually gate notifications (+ add the terminal-bell path)

Status: ready-for-agent
Blocked by: none

- **Severity:** P1 (user-visible controls that do nothing; a promised feature — bell alerts — has
  no backend path at all)
- **Area:** `crates/core/src/notify/reactor.rs`, `crates/core/src/facade.rs` +
  `facade/project_settings.rs`, `crates/app/ui/.../NotificationsSection.tsx`
- **Evidence:** AGENT-reported; mechanism corroborated by the wiring trace. Re-verify the two exact
  gates before coding.

## Problem
Two per-project notification switches are decorative:
1. **"Crash & exit alerts"** (`crash_exit_alerts`) — stored and shown but never consulted by the
   notification reactor. Turning it off does nothing; crash toasts still fire.
2. **"Terminal alerts"** (`terminal_alerts`) — there is **no `TerminalBell` → notification path at
   all**; the switch gates nothing.

Additionally, the **global** notifications-enabled flag is never called by any adapter, so
notifications cannot be disabled anywhere.

Contract: phase-11a NOTIFICATIONS — "Get notified when commands crash or exit unexpectedly" /
"…ring the bell or request attention."

## Root cause (from the wiring audit — verify each line first)
- The reactor gates every toast on one global flag: `if !self.enabled.load(...) { continue }`
  (`notify/reactor.rs:60`); `compose` fires crash toasts with no project-settings reference
  (`:77-84`) and handles only crash/exhausted/permission/error — **`TerminalBell` is ignored**
  (`:75-105`). `TerminalBell` IS emitted by core and folded by the UI projection, but nothing
  turns it into a notification.
- `crash_exit_alerts` has no consumer beyond its setter (`facade/project_settings.rs:65`).
- The global `enabled` flag (`facade.rs:141` default true; setter `:177`) is never called by any
  adapter (app/httpapi/mcp/cli/ui) — confirm with grep before deciding.

## Fix approach
- In the reactor's `compose`/dispatch, look up the **originating process's project** and consult
  `crash_exit_alerts` (crash/exit) and `terminal_alerts` (bell) from project settings before
  emitting. Keep the per-command `terminal_alerts_for()` override honored.
- Add a `TerminalBell` → notification arm to `compose` (respecting `terminal_alerts` +
  per-command override), so the bell switch does something.
- Wire the **global** enabled flag to a real control: either the Integrations/Notifications master
  setting calls `facade.set_notifications_enabled(...)`, or remove the unused flag. Decide with the
  owner; simplest is to make the existing UI master toggle call it.
- Single-source: read project settings through the same façade the settings page uses; no domain
  `if` in the adapter.

## Test plan (must fail before, pass after)
- **Core (`notify` tests):** with `crash_exit_alerts=false` for project P, a crash of a P process
  produces **no** notification; with it `true`, it does. Same matrix for a second project so the
  scoping is proven. Use the existing notify test harness + a fake notifier that records calls.
- **Core:** a `TerminalBell` with `terminal_alerts=true` produces a notification; `false` → none;
  per-command override wins.
- **Core:** the global disable suppresses all.

## Acceptance
- Every notification switch changes observable behavior. The bell path exists and is gated. Global
  disable works. `just test` + `just lint` green.

## Out of scope
Notification styling/content; OS-level notification backend choice (unchanged).
