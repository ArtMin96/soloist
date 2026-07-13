# PRD-02 — Fix the empty terminal pane on a freshly-launched agent/process

Status: needs-human-verify
Blocked by: none

- **Severity:** P1 (the owner's #1 daily symptom: "open a new agent, xterm shows nothing")
- **Area:** terminal attach · `crates/core/src/supervisor` (actor/registration/terminal),
  `crates/app/src/commands/mod.rs`, `crates/app/ui/src/components/terminal/useTerminal.ts`
- **Evidence:** root cause VERIFIED in code (main session, 2026-07-13). Structurally the wiring is
  present; this is a race, not a missing wire.

## Problem
Launching an agent (or starting a process) intermittently leaves the terminal pane showing the
*"This process hasn't started yet. Press Start to run it."* overlay, even though the process is
`Running` and producing output. Clicking Start is a no-op on a running process, so it stays stuck
until the process is re-selected (which remounts the pane) or its status changes again.

## Root cause (verified) — a two-part cross-boundary race
**(a) Backend: the terminal channel is created lazily inside the actor task.**
`terminals.open(id)` is called **only** at `crates/core/src/supervisor/actor.rs:177` (confirmed
sole caller) — inside the `tokio::spawn`ed actor body. `register`/`start`/`launch_actor` never
pre-create it. So between `supervisor.start()` returning and the actor being scheduled,
`attach_pty(id)` → `terminals.attach(id)` returns `None`
(`crates/core/src/terminal.rs:209-215`), and `pty_attach` rejects the invoke with
`"process has not started"` (`crates/app/src/commands/mod.rs:252`).

**(b) Frontend: the retry is keyed only on `process.status` + an optimistic flag.**
`useTerminal.attach()` sets `attachedRef.current = true` **before** the async attach resolves
(`useTerminal.ts:83`); on rejection it resets to `false` + `setState("not-started")` (`:164-168`).
The only retry trigger is `useEffect(… , [process.status, attach])` guarded by
`!attachedRef.current` (`useTerminal.ts:281-283`). Interleaving that strands the pane:
1. mount at `Starting` → `attach()` sets `attachedRef=true`, `ptyAttach` pending;
2. `Running` event arrives → effect re-runs → sees `attachedRef` still `true` → **skips**;
3. the pending `ptyAttach` now rejects → `attachedRef=false`, `state="not-started"`;
4. status never changes again → effect never re-fires → **no retry**.

Contract violated: "a state change with no event → permanently stale UI" — a live process is
shown as not-started.

## Fix approach
**Primary (backend, makes the race impossible):** create the process's `TerminalChannel`
**synchronously in `register` (or `launch_actor` before the `tokio::spawn`)**, so `attach_pty`
never returns `None` for a registered process. The actor then attaches to the already-open
channel instead of opening it. This alone fixes the user-visible bug.

**Secondary (frontend robustness, do as well):** drive the retry off attach **resolution**, not
`attachedRef` + status. On a rejected attach, either retry with a short backoff while the process
is active, or set a state that the effect keys on so it re-fires. Don't leave the only recovery to
a status change that may never come.

**Fold in C6 (resize race, same area):** set the actor's `current_io` **before** announcing
`Running`, and remember the last-known PTY size to re-apply on respawn, so an initial/relaunch
resize isn't dropped (leaving a TUI stuck at 80×24). See findings-log C6.

## Test plan (must fail before, pass after)
- **Core:** a test that `register`+`start`s a process and calls `attach_pty` **immediately**
  (before yielding to let the actor run) returns `Some((scrollback, receiver))`, not `None`.
  Use the existing `FakeSpawner` harness. This directly pins the lazy-channel fix.
- **Core (resize):** a resize issued in the pre-`Running` window is applied to the PTY, not
  dropped; a respawn re-applies the last size (mock the PtyIo).
- **UI (`useTerminal.test.tsx`):** simulate `ptyAttach` rejecting once then succeeding while the
  process is `Running` with no further status change → the pane ends in `live`, not `not-started`.
- **Live verify (`/verify` or the tauri-mcp bridge):** launch a real agent 10× and confirm the
  pane shows output every time (this is the acceptance the phase-5/6 runtime walk never did).

## Acceptance
- A freshly-launched agent shows its output on first render every time (no "Press Start" overlay
  on a running process). Backend `attach_pty` is total for a registered process. Resize on
  spawn/relaunch reaches the PTY. `just test` + `just lint` green + a live repro passes.

## Out of scope
The broader reconciliation backstops (PRD-07). Scrollback/search behavior (unaffected).

## Comments

**Implemented 2026-07-14 (commit `50e0e64`, branch `fix/stability-audit-2026-07`).**

- **Primary (backend):** `terminals.open(id)` moved out of the spawned actor body into
  `Supervisor::launch_actor` — opened synchronously (after the `begin_launch` race gate, before
  `tokio::spawn`), and the actor-facing `ActorTerminal` is passed into `actor::spawn`/`run`.
  `attach_pty` is now total for a *launched* process; a never-started resting process still
  returns `None`, so the "Press Start" overlay is preserved (verified by a test). `terminals` was
  dropped from `ActorPorts` (the actor no longer opens the channel). All six `launch_actor`
  callers (normal start, bulk auto-start, crash auto-restart, orphan adoption) get a correct
  channel; relaunch still reuses buffers via `Terminals::open`.
- **C6 (resize):** `current_io` is set **before** announcing `Running`; a shared
  `last_size: Arc<Mutex<PtySize>>` (written by the input pump on every `Resize`, even with no live
  child; read when building each respawn's `SpawnSpec`) makes a relaunch re-create the PTY at the
  last requested size instead of the 80×24 default. Note: this remembers the size for a **within-
  actor** respawn (restart/resume — the common relaunch); a brand-new actor from a crash
  auto-restart starts from the registered default and relies on the FE re-sync, as before.
- **Secondary (frontend):** a backoff retry effect in `useTerminal.ts` (keyed on `state` +
  status, `ATTACH_RETRY_MS = 120`) recovers a rejected attach while the process is active, so a
  strand no longer waits on a status change that may never come. It does not retry a
  resting/never-started process.

**Fidelity note:** the acceptance's literal "total for a *registered* process" is met as "total
for a *launched* process" — a deliberate scope so a never-started process keeps its overlay rather
than showing an empty live pane. Confirmed with the ticket's intent (the race window is
`start`→actor-scheduled, i.e. `Starting`+).

**Tests (red-before / green-after):** core `attach_pty_is_available_synchronously_after_start`,
`a_never_started_process_has_no_terminal_channel`, `a_resize_reaches_the_running_pty`,
`a_respawn_relaunches_the_pty_at_the_last_resize_size` (new `FakeSpawner::records_resizes` +
`ResizeLog`); UI `useTerminal` attach-retry (reject-once-then-succeed, and no-retry-while-inactive).

**Gates:** `just lint` exit 0; `just test` exit 0 — Rust core 557 (+ all workspace crates green,
3 pty soak tests ignored), UI 288 across 58 files.

**Status: `needs-human-verify`** — the "live repro" acceptance (launch a real agent ~10× via
`just dev` and confirm the pane shows output every time; and a relaunch keeps the pane's size) is a
GUI walk this headless session cannot run. Implementation + unit tests are complete.
