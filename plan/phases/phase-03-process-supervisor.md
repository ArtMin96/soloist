# Phase 3 — Process Supervisor (C2)

**Goal:** The heart. Spawn/stop/restart the three process **subtypes**, drive the status **FSM**, capture
output, signal **process groups** for clean shutdown, and **adopt orphans** on relaunch. Built on Phase
1's actor pattern; headless and exhaustively tested with the **mock clock**. (Raw pipes here; real PTYs
are Phase 4.)

**Delivers:** B1–B9. **Architecture:** context C2; `ProcessSpawner` + `Clock` ports; trust gate from C1.

## Scope
**In:** process registry; `ProcStatus` FSM; start/stop/restart (one + all); subtype semantics; graceful
stop (SIGTERM→grace→SIGKILL on the group); bounded log ring buffer; orphan adoption. **Out:** PTY/
interactive I/O (Phase 4); metrics & restart-policy execution & file-watch (Phase 6 builds on these
events); idle detection (Phase 7).

## Status FSM (ref §4)
```
Stopped ─start─► Starting ─ok─► Running ─exit0─► Stopped
                    │spawn-fail        │exit≠0
                    └────► Crashed ◄────┘
stop(): any ─► Stopping ─(group dead)─► Stopped
restart(): Running ─► Stopping ─► Starting ─► Running
(RestartExhausted set by Phase 6 policy after 10/60s)
```
Every transition emits `ProcessStatusChanged{id,from,to,at,exit_code}`. Transitions are explicit
functions returning `Result<_,IllegalTransition>` (`04` §4).

## Tasks
1. **Subtypes (B1, ref §2):** `Command` (trust-gated, eligible for auto-start/restart/watch), `Agent`
   (interactive, gets activity state in Phase 7), `Terminal` (plain shell). One registry; subtype gates
   which policies apply.
2. **Spawn into a process group (B5,B6):** `tokio::process::Command` with `process_group(0)` via
   `nix`/`pre_exec`; run through the login shell with the captured env (ref §5; full env capture is
   Phase 11, here use `$SHELL -lc`). Apply `working_dir`/`env`.
3. **Trust gate (ref §4):** `start*`/`restart*` refuse untrusted command variants (consults C1
   `TrustStore`) — enforced in core for **all** adapters. Terminals/agents per Solo's rules.
4. **Output capture (pre-PTY):** stdout+stderr → bounded `Ring<LogLine>` (default 5,000 lines) +
   `LogLine` events. (Phase 4 swaps to PTY, same buffer/event contract.)
5. **Exit watcher:** classify `Stopped` (0/killed-by-us) vs `Crashed` (≠0/unexpected signal); record
   `exit_code`; emit.
6. **start_all / stop_all / restart_running (B4):** `start_all` starts trusted `auto_start` **commands**;
   `stop_all` SIGTERMs every group, waits a **5 s grace** (mock-clock-testable), then SIGKILLs; emits a
   summary. Bulk ops scoped to trusted commands (ref §7 bulk).
7. **stop releases todo locks + clears crash tracking (B7, ref §4):** on stop, signal C6 to release
   that process's locks (wire the hook; C6 lands in Phase 9) and remove from restart tracking.
8. **Graceful shutdown:** on app quit, `stop_all()` so no children leak (ref §10).
9. **Orphan adoption (B8, ref §4):** persist `{name→pgid}` to the runtime-state file; on launch, prune
   stale records and **adopt** running orphans when project+name+command match, else emit an
   `OrphansFound` decision (Kill/KillAll/Leave — UI in Phase 5).
10. **Cancellation/cleanup (`04` §5/§8):** each process task holds a `CancellationToken`; stop/restart/
    shutdown cancel cleanly and **reap** the group in the cancel path.

## Interfaces
```rust
struct Supervisor { /* registry, event tx, ports */ }
impl Supervisor {
  async fn start(&self,id:ProcessId)->Result<()>;  async fn stop(&self,id:ProcessId)->Result<()>;
  async fn restart(&self,id:ProcessId)->Result<()>;
  async fn start_all(&self,p:ProjectId)->Result<StartSummary>;  async fn stop_all(&self,p:ProjectId)->Result<()>;
  fn snapshot(&self)->Vec<ProcessView>;  fn subscribe_logs(&self,id:ProcessId)->Receiver<LogLine>;
  async fn reconcile_orphans(&self)->OrphanReport;
}
```

## Acceptance criteria
- `start_all` on a fixture (three `bash -c`/`sleep` commands) → all `Running`; `stop_all` → all
  `Stopped` with **zero surviving child PIDs** (pgroup assertion).
- A non-zero exit → `Crashed` with correct `exit_code`; a user-stopped process → `Stopped` (not Crashed).
- Killing a process's whole group externally is detected and reflected.
- Starting an **untrusted** command is refused by every path.
- A child that spawns grandchildren is fully reaped on stop (no orphans).
- Orphan adoption: a leftover child from a previous run is adopted (match) or surfaced (no match).
- The full suite passes using fake spawner + **mock clock** (grace window tested without real waiting).

## Test plan
- **Unit/integration (headless):** fixtures — `sleep 100`, `bash -c 'exit 3'`, chatty loop, grandchild-
  spawner, `trap '' TERM` (must escalate to SIGKILL after grace). Assert transitions, exit codes, no
  orphans, buffer bound.
- **Stress:** start/stop 50 processes in a loop → zero leaked PIDs, stable memory (precursor to the
  Phase 13 soak).

## Risks & mitigations
- **Orphaned grandchildren** → always signal the **group**; verify with the spawner fixture.
- **exit-vs-stop race** → single owning actor per process (`04` §5), state guarded by the task.
- **SIGKILL data loss** → SIGTERM + grace first; escalate only on timeout.

## Effort
~5–7 days — most correctness-critical phase; budget extra test time.
