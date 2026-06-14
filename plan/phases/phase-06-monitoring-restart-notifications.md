# Phase 6 — Monitoring, Auto-Restart & Notifications (C5 + C7)

**Goal:** Make the stack self-healing and observable: per-process CPU/mem, port discovery + readiness,
crash auto-restart **rate-limited to 10/60s** (ref §4), file-watch restarts (debounced, trusted-only),
and native notifications + the attention bell. This is Solo's "green/red dashboard that fixes itself."

**Delivers:** D1–D11. **Architecture:** contexts C5 (metrics/ports) + C7 (notify); `Clock`, `Notifier`,
`FileWatcher` ports; consumes Phase 3 status events.

## Scope
**In:** metrics sampler; port discovery; readiness; the restart-policy executor (the FSM/rate-limit on
top of Phase 3's events); file-watch restarts; desktop + in-app notifications; attention bell/unread.
**Out:** MCP exposure of these (Phase 8 reads them); idle detection (Phase 7).

## Tasks
1. **Metrics sampler (D1):** `sysinfo`, sample each process **group** every ~1 s for CPU% and RSS;
   emit `MetricsTick{id,cpu_pct,rss}`; UI throttled to ~2 Hz. Runs as a **self-supervised** task
   (`04` §6 — auto-restarts if it dies).
2. **Port discovery (D2, ref §7 `get_process_ports`):** match a group's PIDs to `/proc/net/tcp{,6}`
   inodes via `/proc/<pid>/fd`; cache; expose on `ProcessView.ports`. Best-effort (never crashes core).
3. **Readiness (D3, ref §7 `wait_for_bound_port`):** a `wait_for_port(id, port, timeout)` that resolves
   when the process binds the port; surface a "Running but not Ready" sub-state.
4. **Restart-policy executor (D4, ref §4):** subscribe to `ProcessStatusChanged`. On `Crashed` for a
   trusted `auto_restart` command, restart and record the timestamp in a sliding window; **after 10
   restarts in 60 s → `RestartExhausted`** + notify; reset the window after the process stays Running a
   stability period. **Disabled during app shutdown (D11).** All timing tested via the **mock clock**.
5. **Restart banner (D5):** on auto-restart, keep last crash output and insert a restart banner before
   new output (UI affordance).
6. **File-watch restarts (D6, ref §4):** for commands with `restart_when_changed`, a `notify` watcher on
   the project root, recursive, create+modify, matched via `globset` (`*` crosses separators),
   **debounced** into a quiet window, then `supervisor.restart(id)`. **Command-only, trusted-only**;
   empty/invalid globs → no watcher. **Default ignores** `.git`,`node_modules`,`target`,`dist`,`.venv`
   (D7, our addition). Independent rate-limit from crash restarts.
7. **Notifications (D8/D9, ref §10):** `notify-rust` toasts on crash, "restart exhausted", and (opt)
   "back up"; in-app toast surface; click focuses that process. Respect a global on/off.
8. **Attention bell + unread (D10, ref §10):** maintain per-process attention state (from crashes,
   bells, agent PERMISSION in Phase 7); a title-bar bell + unified unread across sidebar/title/dock.
9. **UI surfacing:** rows show CPU%/RSS, a "restarting (k/N)" badge, RestartExhausted, and "not ready".

## Interfaces
```rust
enum DomainEvent { MetricsTick{id,cpu_pct:f32,rss:u64}, RestartScheduled{id,attempt:u32},
                   RestartExhausted{id}, ReadyStateChanged{id,ready:bool}, FileRestart{id}, Attention{id,kind} }
impl Supervisor { async fn wait_for_port(&self,id:ProcessId,port:u16,to:Duration)->Result<()>; }
```

## Acceptance criteria
- `kill -9` on a process → Crashed → Restarting → Running automatically + a desktop notification.
- A process that crashes immediately and repeatedly stops at **exactly 10 restarts within 60 s** →
  `RestartExhausted` + notification; no hot-loop (verified with mock clock).
- Touching a watched file restarts exactly that command, **debounced** (a save burst = one restart);
  editing an ignored path does nothing.
- CPU%/RSS move for a busy process (`yes>/dev/null`), ~0 for idle; ports list a dev server's port; a
  `wait_for_port` resolves when it binds.
- Killing the metrics sampler task → it self-restarts; app unaffected (K4 precursor).

## Test plan
- **Integration (headless):** crasher fixture → 10/60s exhaustion + backoff via mock clock; file-touch →
  debounced restart; `python -m http.server` → ports + readiness; `yes`/`sleep` → metric deltas;
  sampler-kill → self-restart.
- **Manual:** real dev server: edit→restart; kill→auto-restart+toast.

## Risks & mitigations
- **Restart storms** → the documented 10/60s gate + trusted-only + shutdown-disable; never infinite.
- **Watcher false positives** → default ignores + debounce + glob-scoped matching.
- **`/proc` portability** → guard missing fields; ports are best-effort, never fatal.
- **CPU% semantics across cores** → normalize `sysinfo` deltas; document per-core vs total.

## Effort
~5–6 days.
