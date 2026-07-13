# PRD-08 — Move SQLite off the async runtime + bound coordination payloads

Status: ready-for-agent
Blocked by: none

- **Severity:** P2 (runtime-thread blocking under I/O stalls; unbounded DB growth from a
  misbehaving agent)
- **Area:** `crates/store/src/lib.rs` (+ all repo modules), `crates/app/src/open_project.rs`,
  `crates/core` coordination write surfaces, `crates/ipc/src/frame.rs`
- **Evidence:** D4 VERIFIED (single `Mutex<Connection>`, no `busy_timeout`, inline calls); D3
  AGENT-reported; `load_project` blocking is a known-deferred item.

## Problem
1. **D4 (VERIFIED):** every store call runs `rusqlite` **inline** on the tokio runtime — the store
   is a single `std::sync::Mutex<Connection>` (`crates/store/src/lib.rs:40`) with no
   `spawn_blocking`, and `configure()` sets WAL + foreign_keys but **no `busy_timeout`**
   (`lib.rs:91-95`). A WAL-commit `fsync` (slow disk, full disk) blocks a runtime worker thread.
   Contract: CLAUDE.md §8 "spawn_blocking (no blocking calls on the tokio runtime)."
2. **Known-deferred:** `facade.rs::load_project` does blocking fs/SQLite on the calling (main/
   async) thread — the same class, on the hottest UI path (open project).
3. **D3:** the only ceiling on any coordination write is `MAX_FRAME = 8 MiB`
   (`crates/ipc/src/frame.rs:14`). `kv_set` (arbitrary JSON), `scratchpad_write`, todo docs, and
   `timer_set` bodies have no length validation, and record counts are unbounded — a misbehaving
   in-project agent can grow `soloist.db` without bound. Contradicts `plan/05 §7` ("kv = small
   structured state, not logs") and CLAUDE.md §8 "bounded everything."

## Fix approach
- **Off-runtime execution:** run store operations on a blocking-safe executor. Options, cheapest
  first: (a) wrap the `SqliteStore` calls in `tokio::task::spawn_blocking` at the adapter boundary
  where they're awaited (ipc_server `handle_request`, Tauri handlers); or (b) move the connection
  to a dedicated DB thread with an mpsc request channel (a single-writer actor, matching the
  architecture). (b) is cleaner and removes the shared mutex; (a) is a smaller diff. Recommend (b)
  if time allows, else (a). Keep core pure — this is adapter/composition work.
- Add `PRAGMA busy_timeout` (a few seconds) in `configure()` as defense-in-depth even with one
  connection (future-proofs a second reader).
- **`load_project` off-thread:** since it spawns actors, it needs real-runtime verification — do it
  as part of this workstream with a live check, not a headless edit (per the known-deferred note).
- **D3 bounds:** add named per-value caps for `kv_set` (value bytes), `scratchpad_write` (content
  bytes), todo doc size, and `timer_set` body — reject oversized writes with a typed error
  (`ConfigError`/coordination error), single-sourced as named consts. Consider a soft per-project
  row/byte budget if cheap; at minimum cap each value.

## Test plan (must fail before, pass after)
- **Store/adapter:** assert store calls execute off the runtime worker (e.g. a test that a slow
  store op doesn't stall a concurrent runtime task) or, more simply, that the DB access path is
  `spawn_blocking`/DB-thread routed (structural test).
- **Coordination:** `kv_set`/`scratchpad_write`/`todo`/`timer_set` over the per-value cap returns a
  typed error and writes nothing; at the cap succeeds (boundary test). Closes an audit bound gap.
- **`busy_timeout`:** `configure` sets it (pragma read-back test).
- **`load_project`:** live verify (open a project) still works and doesn't block the UI.

## Acceptance
- No `rusqlite` call runs inline on a runtime worker. `busy_timeout` set. Every coordination write
  is size-bounded by a named const. `soloist.db` can't be grown unboundedly by one write. `just
  test` + `just lint` green; soak shows no regression.

## Out of scope
Connection pooling for read concurrency (single-writer is fine per the architecture). Retention/
GC of old coordination rows (separate feature if wanted).
