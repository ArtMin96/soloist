# PRD-03 — Orphan reconciliation must not SIGKILL a recycled PID/PGID

Status: done
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

## Comments

**Done 2026-07-14 — commit `f2494e5` (branch `fix/stability-audit-2026-07`).**

What changed:
- New `ProcessIdentity { boot_id, started_at }` + `identity: Option<ProcessIdentity>` on
  `OrphanRecord` (`#[serde(default)]` → legacy files migrate to `None`). Identity is captured at
  record time in the actor via the port.
- `OrphanControl` now exposes `identify(pgid)` (the `/proc` probe, adapter-only) and a fail-closed
  default `is_recorded_alive(record)` (boot_id + leader start-time must match; legacy/recycled/dead
  all → not-the-orphan). Core stays pure; `PgidOrphanControl::identify` reads
  `/proc/<pgid>/stat` field 22 + `/proc/sys/kernel/random/boot_id`.
- Every recorded-orphan signal path is identity-guarded: classify/reconcile (recycled → prune),
  `kill_orphan` (re-checks identity, returns the failed SIGKILL and keeps the record instead of
  forgetting it), the adopted-group liveness poll, and `GroupSignal::terminate`/`kill`.
- E9: a failed kill surfaces via the error banner and keeps the row; `killAll` fans out over
  `killOne` so a partial failure drops only the groups actually reaped.
- Fidelity recorded in `plan/05` (Orphaned processes) and `KNOWN-DIVERGENCES.md` D-16.

Tests (red-before/green-after): core `reconcile_prunes_a_recycled_group_with_a_different_boot_id`,
`…_different_start_time`, `reconcile_fails_closed_on_a_legacy_record_without_identity`,
`kill_orphan_does_not_signal_a_recycled_group`, `kill_orphan_reports_a_failed_signal_and_keeps_the_record`;
adopt-guard `does_not_signal_a_recycled_group` / `signals_the_group_while_its_identity_matches`;
pty adapter `identify_reads_the_current_process_from_proc` +
`orphan_control_tracks_a_group_by_identity_until_it_dies` (real `/proc`); store
`a_legacy_record_without_identity_loads_as_unverifiable`; UI `useOrphans.test.ts` (6, incl. partial-fail).

Gates: `just lint` exit 0; `just test` exit 0 — Rust workspace all green (3 pty soak ignored),
UI 294 passed across 59 files. Independent adversarial code-review of the diff: no P0/P1; the three
P3s resolved (killAll partial-fail fixed; the actor panic-path bare SIGKILL is a same-session
in-memory pgid — not a persisted recorded orphan — and guarding it would risk reintroducing the
double-spawn regression the panic-reap fixes, so it is a deliberate non-change; the leader-gone
prune tradeoff is disclosed in D-16). No live GUI walk needed — orphan reconciliation is fully
headless-testable (port fakes + a real `/proc` adapter test).
