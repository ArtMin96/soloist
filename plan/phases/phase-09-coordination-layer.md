# Phase 9 — Coordination Layer (C6)

**Goal:** The multi-agent coordination surface that distinguishes Solo: **scratchpads** (revision-
guarded), **todos** (tags/blockers/locks/comments/transfer), **timers** (incl. fire-when-idle),
**lease locks**, and **key-value** — all persisted in SQLite and exposed as MCP tools (ref §7). This is
what lets a lead agent spawn workers, hand out todos, take locks, and **wait token-free** until children
go idle.

**Delivers:** G1–G11; completes E7 (agents spawning agents, end-to-end). **Architecture:** context C6;
`Store` (SQLite repos) + `Clock`; idle signal from C4; identity/scope from C8.

## Scope
**In:** the five durable aggregates + their repos + migrations; revision/lock/lease semantics; the MCP
tools for each; the idle-watcher timer wiring. **Out:** UI panels for scratchpads/todos (Phase 11 adds
editors); the core MCP transport (Phase 8 built it — we register more tools here).

## Why durable & here
Coordination state must **survive app restarts and outlive any one process/chat** (ref §1 "longer than
one chat window"). It's modeled as repositories over SQLite (`04` §7) with optimistic concurrency, kept
separate from ephemeral process state.

## Tasks
1. **Scratchpads (G1/G2, ref §7):** `ScratchpadRepo`; tools `scratchpad_list/_read/_write/_append/
   _rename/_add_tags/_remove_tags/_clear/_delete/_archive/_transfer/_save_to_file/_load_from_file`
   (+ `_edit/_append_section/_tail/_find`). **Revision-guarded writes:** `_write` takes
   `expected_revision`; mismatch → `RevisionConflict` (`04` §7). Leading H1 = title; read modes
   full/headings/section.
   > **Resolved 2026-07-01 (D-7 / `05` §12):** the free-form verbs
   > (`_append`/`_edit`/`_append_section`/`_tail`/`_find`/`_clear`) and file-io
   > (`_save_to_file`/`_load_from_file`) were **not** built — intentional divergence / formally
   > declined; the disciplined-document subset shipped (revision-guarded `_write`), and `_transfer`
   > lands via O10.
2. **Todos (G3/G4/G5, ref §7):** `TodoRepo`; `todo_create/list/get/update/complete/delete`, tags,
   **blockers** (`_set_blockers/_add_blocker/_remove_blocker` — a todo is gated until blockers complete),
   comments (`_comment_create/update/delete/list`), `todo_transfer` (preserves comments/completion,
   clears blockers/locks), and **process-owned locks** (`todo_lock/_unlock`) that **auto-release when the
   bound process closes** (wired to Phase 3 stop hook).
3. **Lease locks (G6, ref §7+§12):** `LockRepo`; `lock_acquire` (project-scoped, **explicit TTL +
   owner ProcessId**), `lock_status`, `lock_release`; auto-release on TTL expiry **or** owner-process
   close. "Signals, not ownership" — non-blocking; contention returns current holder.
4. **Timers (G7/G8/G9, ref §7):** `TimerRepo` + a scheduler on the `Clock`; `timer_set` stores a `body`
   and, on fire, **delivers `body` to the owning agent as a fresh user turn** (via `send_input` to the
   bound agent process); `timer_fire_when_idle_any/all` subscribe to C4 `AgentActivityChanged` and fire
   when watched processes are idle (or max-wait elapses); `timer_cancel/pause/resume/list`. Responses
   include `already_idle`, `waiting_on`. Requires a bound owning actor.
5. **Key-value (G10, ref §7):** `KvRepo`; `kv_set/get/delete/list` — project-scoped JSON; **default
   off** (tool toggle).
6. **Tool gating (ref §7):** scratchpads/todos/timers inherit MCP enablement; key-value default off;
   per-group settings (Phase 11 surfaces toggles).
7. **Persistence/durability (G11):** all aggregates in SQLite (WAL, transactions, migrations); survive
   app restart; lock/lease cleanup reconciled on launch (expire stale).
8. **Agents spawning agents (E7):** with C8 `spawn_agent` + these primitives, a scripted lead agent can
   spawn a worker, assign a todo, take a lock, set `fire_when_idle_all`, and integrate on wake.

## Interfaces
```rust
// repos over Store
trait ScratchpadRepo { fn write(&self, id, body, expected_rev)->Result<Revision,RevisionConflict>; … }
trait TodoRepo { fn set_blockers(&self, id, Vec<TodoId>)->Result<()>; fn lock(&self, id, owner:ProcessId)->Result<()>; … }
trait LockRepo { fn acquire(&self, key, owner:ProcessId, ttl:Duration)->Result<Lease,Held>; … }
trait TimerRepo { fn set(&self, owner:ProcessId, fire:FireCond, body:String)->Result<TimerId>; … }
enum FireCond { At(Instant), WhenIdleAny(Vec<ProcessId>), WhenIdleAll(Vec<ProcessId>) }
```

## Acceptance criteria
- Scratchpad: write at the current revision succeeds; a stale `expected_revision` → `RevisionConflict`.
- Todo with a blocker stays gated until the blocker completes; a process-owned todo-lock releases when
  that process closes (ties to Phase 3 stop).
- Lease: `lock_acquire` with TTL auto-releases on expiry and on owner-process close; a second acquire
  while held reports the holder.
- Timer: `timer_set` delivers `body` to the owning agent as a fresh turn; `fire_when_idle_all` fires
  only when **all** watched processes are idle (verified with C4 fixtures + mock clock).
- All coordination state survives an app restart (SQLite).
- An end-to-end scripted "lead → spawn worker → assign todo → lock → wait-idle → integrate" run passes.

## Test plan
- **Unit (mock clock):** revision conflicts; blocker gating; lock TTL/owner-close release; timer fire
  conditions; KV round-trip.
- **Integration:** restart-persistence; the end-to-end orchestration script against real stub agents.

## Risks & mitigations
- **Lease semantics undocumented (ref §7/§12)** → we own explicit TTL + owner-close release; documented.
- **Timer/idle coupling** → reuse C4's tested idle FSM; timers subscribe to its events; deterministic
  tests via mock clock.
- **Concurrent agents clobbering state** → revision guards + locks are the mitigation; test contention.
- **Scope (now v1)** → per your decision the coordination layer is a **v1 must-have** (matrix
  G1–G11 + E7), not post-parity. It's large (~50 tools); sequence *within* the phase as **durable store
  → leases/locks → timers/idle-watchers → scratchpads/todos → key-value** so the highest-value piece
  (token-free fire-when-idle orchestration) lands first and the rest is additive.

## Effort
~7–9 days (the full surface). Durable store + timers/locks are the high-value core.
