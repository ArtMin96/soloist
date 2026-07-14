# PRD-04 — Notification toggles must actually gate notifications (+ add the terminal-bell path)

Status: needs-human-verify
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

## Comments

**Implemented — commit `7e5807c` (branch `fix/stability-audit-2026-07`).** Gates green:
`just lint` exit 0; `just test` exit 0 (full Rust workspace; UI vitest 60 files / 296 tests).

What changed:
- **Reactor gating (`notify/reactor.rs`).** Each attention event now resolves its process's
  project + label (`supervisor.view`) and consults the durable settings before composing a toast.
  A closed `Attention` enum maps each event → the switch that gates it → the toast it shows:
  crash / exhausted → `crash_exit_alerts`; **bell + agent Permission/Error → `terminal_alerts`**
  (honouring the per-command `terminal_alerts_for` override).
- **Bell path added** — `TerminalBell` had no notification arm before; the "Terminal alerts" switch
  now gates a real bell toast.
- **Global master switch is real + persisted.** New `Notifications { enabled }` sub-document on
  global `Settings` (serde-default on); `notification_settings` / `set_notification_settings`
  façade + Tauri command; a **Notifications tab** in the global Settings overlay (removed from
  `UNDEFINED_TABS`), mirroring the Integrations pattern. The reactor reads the durable value **live**
  (the ephemeral, unreachable `AtomicBool` was removed).
- **Owner decision (this session):** global flag → "Build a real master toggle" (persisted +
  UI). Also, gating agent Permission/Error under `terminal_alerts` is slightly beyond the literal
  Fix-approach but matches the switch's own copy ("rings the terminal bell **or asks for
  attention**") and the `DomainEvent` contract — recorded as intended.

Tests (all green): reactor crash/bell/attention gating incl. **second-project scoping** and the
per-command override **both directions**; the global master silences all; façade round-trip;
SQLite round-trip of the new field; UI load/persist of the master toggle. `/code-review`
(Standards + Spec) ran clean — no hard violations, spec satisfied.

**Why `needs-human-verify` (not `done`):** a new user-facing Settings tab was added (CLAUDE.md §5
wants a live UI pass) and the end-to-end real-desktop-toast suppression is adapter/runtime — both
need a GUI walk this headless session can't do. The gating **logic** is fully unit-verified.

**Human check (`just dev`), with a trusted `auto_restart:false` command in a project:**
1. Settings → **Notifications** tab renders; the "Desktop notifications" master switch reflects the
   stored state and toggles cleanly (looks consistent with the other tabs / a quick `/impeccable`
   glance).
2. Project settings → Notifications → turn **Crash & exit alerts** off → `kill -9` the command →
   **no** toast. Turn it back on → crash again → a **"<name> crashed"** toast fires.
3. Turn **Terminal alerts** off → make the command `printf '\a'` (ring the bell) → **no** toast;
   on → **"<name> rang the bell"** toast. (Same switch also gates an agent's permission prompt.)
4. Global master **off** → neither of the above fires anywhere; **on** → they resume.
5. Restart the app → the global master switch keeps its last value (persisted).
