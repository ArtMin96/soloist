# PRD-03 — Orphan reconciliation must not SIGKILL a recycled PID/PGID

Status: ready-for-agent
Blocked by: none

- **Severity:** P1 (can SIGKILL an unrelated same-user process group; the exact class Solo fixed
  in v0.9.3)
- **Area:** orphan adoption · `crates/core/src/ports/mod.rs`, `crates/core/src/supervisor/reconcile.rs`,
  `crates/core/src/orphans.rs`, `crates/store/src/runtime.rs`, `crates/pty/src/lib.rs`,
  `crates/app/ui/src/store/useOrphans.ts`
- **Evidence:** VERIFIED in code (main session, 2026-07-13).

## Problem
On relaunch after a crash/force-quit + PID churn (or a reboot), Soloist judges a persisted
orphan's liveness purely by process-group id. The OS may have reassigned that pgid to an unrelated
process group belonging to the same user. Soloist can then **adopt-then-kill** or **offer the user
a "Kill" that SIGKILLs** that unrelated group.

## Root cause (verified)
- `OrphanRecord` persists only `{ project_root, name, command, pgid }` — **no boot-id, no process
  start-time** (`crates/core/src/ports/mod.rs:297-302`).
- `FileRuntimeState` writes/reads it verbatim across restarts/reboots
  (`crates/store/src/runtime.rs`, `~/.local/share/soloist/runtime-state.json`).
- `is_alive(pgid)` is bare `killpg(pgid, None)` liveness (`crates/pty/src/lib.rs:248-251`) — it
  cannot tell the original group from a recycled one.
- Two bad paths: **(i) surface/kill** — `kill_orphan` SIGKILLs the bare recorded pgid with no
  identity check (`reconcile.rs:70-73`); **(ii) adopt** — a recycled group matching a resting
  registered command by `{project_root,name,command}` is adopted and later SIGKILLed on stop.
- Nuance: the `{project_root,name,command}` match narrows the *adopt* risk, but the kill-by-bare-
  pgid **surface path is unguarded**, and two runs of the same command in one project share
  identity.

## Fix approach
Stamp each `OrphanRecord` with a **stable process identity** captured at record time and require it
to match before adopting or killing:
- Add `started_at` = the group leader's start-time from `/proc/<pid>/stat` field 22 (jiffies since
  boot) **and** `boot_id` = `/proc/sys/kernel/random/boot_id`. Both are cheap, Linux-native
  (project is Linux-only, D2), and together make PID/PGID reuse detectable (a different boot_id ⇒
  the recorded pgid is meaningless; a same-boot pgid whose leader start-time differs ⇒ recycled).
- In `is_alive`/the classify step, treat a pgid as the recorded orphan **only if** boot_id matches
  and the leader's current start-time equals the recorded one; otherwise treat the record as dead
  (drop it, never kill).
- This lives behind the existing `OrphanControl` port; the `sys`/`pty` adapter reads `/proc`. Core
  stays pure — add the identity fields to the record and the match logic to the port contract.
- **Migration:** old runtime-state files lack the new fields → deserialize them as "unverifiable"
  and **do not kill/adopt** (fail-closed: surface as "leftover, identity unknown" or drop),
  rather than trusting a bare pgid.

**Fold in E9:** `useOrphans.ts:41,49` swallows the resolve rejection (`catch(() => {})`) and drops
the row optimistically — surface a failed SIGKILL to the user (toast/error) and keep the row.

## Test plan (must fail before, pass after)
- **Core/adapter:** an `OrphanRecord` whose `boot_id` differs from the current boot is classified
  **dead** (never adopted, never offered for kill). A record whose leader start-time no longer
  matches is likewise not killed. Use a fake `/proc` reader or inject the identity probe via the
  port so it's unit-testable on a mock.
- **Core:** a genuine same-boot, same-start-time record is still correctly adopted/surfaced (no
  regression to legitimate reconciliation — the existing `reconcile.rs` tests must still pass).
- **Migration:** a legacy record (no identity fields) is treated fail-closed.
- **UI:** a rejected `orphansResolve` keeps the row and shows an error.

## Acceptance
- Soloist never SIGKILLs a process group whose identity (boot_id + leader start-time) doesn't match
  the recorded orphan. Legitimate leftovers are still reaped/adopted. Legacy records fail closed.
  Failed kills are visible in the UI. `just test` + `just lint` green.
- Record the Solo v0.9.3 fidelity item in `plan/05` / `KNOWN-DIVERGENCES.md`.

## Out of scope
Adding a SIGTERM handler to the app process (tracked separately in PRD-07/E10).
