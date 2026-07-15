# PRD-08 — Move SQLite off the async runtime + bound coordination payloads

Status: done
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

## Comments

**Resolved 2026-07-14 — impl commit `f15dcad` (branch `fix/stability-audit-2026-07`); docs/ledger commit follows.**

**Design decision (owner-confirmed this session, `AskUserQuestion` → "Comprehensive"):** the ticket
recommended option (b), a dedicated DB-thread actor. But the durable ports are **synchronous**, so a
DB-thread handle would still block the calling tokio worker on its reply for the whole `fsync`
duration — it relocates *where* rusqlite runs without freeing the worker, i.e. it does not fix the
stated Problem. The only design that frees the worker is `spawn_blocking` at the adapter boundary
(option a), applied **comprehensively** across all three in-process adapters.

**What changed**
- **Off-runtime (D4):**
  - MCP `handle_request` peels the three requests that themselves await the core
    (`send_input`/`close_process`/port-wait) and routes every other request through one
    `spawn_blocking(dispatch_blocking)`; `dispatch_blocking` stays an exhaustive `match` (a new
    request variant fails to compile).
  - HTTP: `ApiState::blocking` offloads every synchronous handler (the store-touching reads and
    all sync mutations); the one async handler (`remove_project`) stays inline.
  - Tauri: an `offload()` helper wraps every synchronous store-touching command — project
    load/list, trust, agent list/launch, proc/stack start/restart/resume, coordination writes,
    settings, per-project settings, timers, orchestration snapshot. Pure in-memory commands
    (`proc_list`, `proc_stop`, `stack_stop`, `pty_*`, `*_link`, `lineage_edges`) correctly stay
    inline; the async ones (`project_remove`, `agent_detect`, `pty_write/resize`) can't be
    spawn_blocking-wrapped and are the known residual.
  - `open_project::open` loads a handed-in project (`solo.yml` association / CLI arg / second
    launch) on the blocking pool so it never blocks the main/event thread.
- **`busy_timeout`:** `configure()` sets `PRAGMA busy_timeout=5s` (named const `BUSY_TIMEOUT_MS`),
  with a pragma read-back test.
- **Bounded writes (D3):** named byte caps per aggregate (consts at module top) — kv value 64 KiB,
  scratchpad content 256 KiB, todo doc 64 KiB, timer body 16 KiB — enforced **before** persistence.
  New `CoordinationError::PayloadTooLarge` (→ `IpcError::PayloadTooLarge`, a request error) for
  kv/timer; scratchpad/todo fold the cap into their existing `validate()` "not well-formed" message
  so both the MCP and local Tauri write paths are covered. Recorded in `plan/05` §12.

**Tests (red-before/green-after):** the four cap boundaries (at cap accepted, over cap rejected +
persists nothing) — `facade::kv`, `facade::coordination`, `coordination::scratchpad`,
`coordination::todo`; `store` busy_timeout read-back; `ipc` `PayloadTooLarge` mapping +
request-error classification; a **non-timing barrier test** (`httpapi::lib_tests`) proving the
blocking helper runs façade ops off the runtime thread (inline execution would deadlock on the
barrier within the 5 s timeout).

**Gates:** `just lint` exit 0 (fmt, clippy `-D warnings`, tsc, eslint, prettier, dep-direction;
file-size advisory only). `just test` — **Rust 941 passed / 0 failed / 3 ignored** (pty soak),
**UI 306 passed**. `/code-review` (independent subagent) confirmed the four correctness-critical
areas clean; one acted-on finding (a `plan/05 §7` citation in a source comment, removed per §8).

**Why `needs-human-verify`, not `done`:** every mechanism is unit/behavior-tested headless, but
the ticket calls out that `load_project` **spawns actors** and needs a real-runtime check. Please run
`just dev` and confirm:
1. **Open a project** (folder picker) — it loads, its processes register/auto-start, and the UI stays
   responsive during the open (no freeze). Open a folder with **no `solo.yml`** too (it auto-creates
   one).
2. **Open via file association / CLI arg** — `soloist /path/to/project` (or double-click a `solo.yml`)
   opens that project and the window comes to front.
3. **Coordination panels still work** — edit a scratchpad and a todo in the panel (save succeeds), and
   a settings toggle persists.
4. Nothing regressed under a **chatty process** (terminal output stays smooth while a project loads).

**Next frontier ticket: 09** (`ready-for-agent`, `Blocked by: none`) — working_dir/peer-uid/trust-hash/
PATH/CLI hardening. (07 is still `Blocked by: 02`, which remains `needs-human-verify`; 10 is unblocked.)

**Owner-confirmed working at runtime 2026-07-15** (`just dev`, fixture `~/soloist-verify`). All walk steps passed → `Status: done`.
