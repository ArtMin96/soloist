# 04 — Engineering Architecture, Domains & Patterns

> The brief: *"a clean, well-architected app which is not going to break in continuous work."* Soloist
> is a long-running supervisor that may stay open for weeks, spawning/killing hundreds of processes,
> streaming gigabytes of terminal output, and watching filesystems. The enemy is **slow rot** — leaked
> PIDs, unbounded buffers, wedged tasks, drifting state. This document defines the architecture and the
> non-negotiable patterns that prevent that. Every phase must conform to it; Phase 1 builds it as a
> walking skeleton before any feature lands.

This file is the **design contract**. `05-solo-reference-and-sources.md` is the **behavior contract**.
Phases cite both.

---

## 1. Architectural style: Hexagonal (Ports & Adapters)

The **domain core is pure and framework-free**. Tauri, MCP, the HTTP API, the CLI, SQLite, the OS, and
the PTY library are all **adapters** plugged into **ports** (Rust traits). The core never imports
`tauri`, never speaks JSON-RPC, never knows a UI exists.

```
            ┌──────────────── adapters (drive the core) ────────────────┐
            │  Tauri commands   MCP server   HTTP API   `soloist` CLI    │
            └───────┬───────────────┬───────────┬───────────┬───────────┘
                    ▼               ▼           ▼           ▼
            ╔════════════════════════════════════════════════════════════╗
            ║                     DOMAIN CORE (pure)                       ║
            ║   bounded contexts §3 · ports (traits) §2 · event bus §5     ║
            ╚═══════┬───────────────┬───────────┬───────────┬─────────────╝
                    ▼               ▼           ▼           ▼
            ┌──────────────── adapters (driven by the core) ─────────────┐
            │  ProcessSpawner(PTY)  Clock  FileWatcher  Notifier  Store   │
            │  (portable-pty)      (tokio) (notify)   (libnotify) (SQLite)│
            └────────────────────────────────────────────────────────────┘
```

**Dependency rule (enforced):** adapters depend on the core; the core depends on **nothing
app-specific**. A Cargo lint/CI check forbids `tauri`, `rmcp`, `axum`, `rusqlite` from appearing in
`crates/core`. This is what lets us unit-test the entire behavior headless and swap WebKit/MCP/storage
without touching logic.

**Ports the core defines (traits), adapters implement:**

| Port (trait) | Real adapter | Test adapter |
|--------------|-------------|--------------|
| `ProcessSpawner` (spawn into PTY+pgroup, write, resize, kill) | `portable-pty` + `nix` | in-memory fake child |
| `Clock` (now, sleep, intervals) | `tokio::time` | deterministic mock clock |
| `FileWatcher` | `notify` | scripted event source |
| `Notifier` (desktop toast) | `notify-rust` | recording spy |
| `Store` (durable repos §7) | SQLite (`rusqlite`/`sqlx`) | in-memory SQLite / fakes |
| `Summarizer` (agent idle summary) | user's agent CLI headless | canned responses |
| `EventSink` (emit to UI/MCP) | Tauri emit / MCP push | channel collector |

Mockable ports + a deterministic `Clock` make timers, backoff, debounce, and rate-limits testable
**without real waiting** — essential for the longevity guarantees in §8.

---

## 2. The core is the only source of truth

- The **core owns all authoritative state**. Adapters hold **no** business state — they translate
  requests in and project state out.
- The frontend (React) holds a **read-model projection** pushed via events; it never computes
  authoritative status. (No business logic in React — see §10.)
- Three different adapters (Tauri UI, MCP, HTTP/CLI) must produce identical behavior because they all
  funnel through the **same core commands**. We never reimplement "restart" three times.

---

## 3. Domain decomposition (bounded contexts)

Each context is a module in `crates/core` with one responsibility, a published interface, and its own
tests. Contexts communicate through **typed commands and events**, not by reaching into each other's
internals. Ask of each: *what does it do, how do you use it, what does it depend on?*

| # | Bounded context | Owns | Depends on (ports/contexts) | Maps to Solo (ref §) |
|---|-----------------|------|-----------------------------|----------------------|
| C1 | **Projects & Config** | project registry, `solo.yml` load/validate/sync/hash, auto-detection, trust store | Store, FileWatcher | §3,§4,§9 |
| C2 | **Process Supervision** | process registry, status FSM, start/stop/restart, orphan adoption, restart policy | ProcessSpawner, Clock, C1(trust) | §2,§4 |
| C3 | **Terminal I/O** | PTY streams, rendered+raw buffers, input, resize, OSC parsing | ProcessSpawner | §7,§10 |
| C4 | **Agents & Idle** | agent tool defs, launch, 5-state idle FSM, auto-summarization | C2, C3, Summarizer, Clock | §6 |
| C5 | **Monitoring** | CPU/mem sampling, port discovery, readiness | Clock, /proc | §7,§10 |
| C6 | **Coordination** | scratchpads (rev-guarded), todos (blockers/locks/comments), timers, leases, key-value | Store, Clock, C4(idle) | §7 |
| C7 | **Notifications** | crash/attention/idle toasts, unread/attention-bell state | Notifier, EventSink | §10 |
| C8 | **Integration façade** | the public command/query API the adapters call; identity & effective-project scope | all above | §7,§8 |

**Why these boundaries:** they match Solo's real seams (ref §) and they isolate failure. A bug in
auto-summarization (C4) cannot corrupt the process registry (C2). Coordination state (C6) persists
independently of live processes (C2), so todos/scratchpads survive restarts while PTY buffers don't.

**Context map (who calls whom):** adapters → C8 only. C8 orchestrates C1–C7. C2↔C3 are tightly paired
(a process has a PTY) and share a `ProcessId`. C4 observes C3 output + C2 status. C6 references
`ProcessId`/`ProjectId` but never controls processes. No cycles.

---

## 4. Core domain types (the ubiquitous language)

Stable IDs everywhere (`ProjectId`, `ProcessId`, `TodoId`, `ScratchpadId`, `LockId`, `ActorId`).
Newtypes, not bare strings/ints. Key enums are **closed** and drive exhaustive `match`:

```rust
enum ProcessKind { Command, Agent, Terminal }                 // ref §2
enum ProcStatus { Stopped, Starting, Running, Crashed,        // ref §4
                  Restarting, Stopping, RestartExhausted }
enum AgentActivity { Idle, Permission, Thinking, Working, Error } // ref §6
enum Trust { Untrusted, Trusted { variant_hash: Hash } }      // ref §4
enum Visibility { Shared, Local }                             // YAML vs app-state, ref §3
```

State transitions are **explicit functions** returning `Result<NewState, IllegalTransition>`, never
ad-hoc field mutation. Illegal transitions are unrepresentable or rejected — the FSM is the contract.

---

## 5. Concurrency & control flow: actor-style supervision, event-driven

**One async runtime (`tokio`), message-passing, no shared mutable domain state.**

- **Process actor (the core pattern):** each managed process is one supervised `tokio` task that
  **solely owns** its child handle, PTY master, stdin sender, and exit watcher. Other contexts interact
  with it only by sending messages over a bounded `mpsc` and reading its emitted events — never by
  locking shared fields. This eliminates the classic supervisor races (exit-vs-stop, double-kill).
- **Cancellation:** every task holds a `CancellationToken`; stop/restart/shutdown cancel deterministically. A task **must** clean up its OS resources in its cancellation/`Drop` path (close PTY, kill pgroup).
- **Event bus:** contexts publish typed `DomainEvent`s on `tokio::sync::broadcast`; adapters subscribe.
  Events are **deltas over a snapshot** the adapter fetched, so a late/!dropped subscriber re-syncs by
  re-reading the snapshot (UI does this on focus/reconnect).
- **CQRS-lite:** **commands** (start, restart, write-scratchpad) mutate via the owning context and
  return a typed result; **queries** read cheap projections (`ProcessView`, `ProjectView`). Reads never
  block writes.
- **Single-writer per aggregate:** a given process/todo/scratchpad has exactly one task/owner that
  writes it. Coordination locks (C6) mediate *cross-agent* intent, not in-process data races.

```
adapter --command--> C8 --routes--> owning context/actor --emits--> DomainEvent --> all adapters
```

---

## 6. Error handling & fault isolation

- **Typed errors at boundaries** (`thiserror`), ergonomic `anyhow` *inside* a context; adapters map
  core errors to their wire format (Tauri error, MCP error object, HTTP status). Every fallible command
  returns a value the UI can render — **no panics as control flow**.
- **No `unwrap()`/`expect()`/`panic!` in any long-running task** (clippy-enforced via
  `#![deny(clippy::unwrap_used, clippy::expect_used)]` in `core`, allowed only in tests). A child
  process dying, a malformed `solo.yml`, a full disk, a missing agent binary — all are **expected
  errors**, handled, surfaced, and recoverable.
- **Panic isolation:** each process actor and each background sampler runs under a supervisor that
  catches task panics (`JoinHandle` + `catch_unwind` boundary), logs them, marks that unit `Error`, and
  **keeps the rest of the app alive**. One wedged process never takes down the supervisor.
- **Watchdog/self-supervision:** long-lived internal tasks (metrics sampler, file-watch dispatcher,
  event pump) are themselves supervised and **auto-restarted with backoff** if they exit unexpectedly —
  the app supervises itself the way it supervises user processes.

---

## 7. State & persistence

Split by lifetime; this is central to "doesn't break in continuous work."

| State | Lifetime | Home |
|-------|----------|------|
| Process registry, `ProcStatus`, PIDs/pgids, metrics | **ephemeral** (rebuilt each run) | in-memory + a small runtime-state file for **orphan adoption** (ref §4) |
| PTY rendered + raw buffers, scrollback | **ephemeral**, **bounded** | in-memory ring/byte buffers (caps in §8) |
| Trust decisions, project registry, settings, agent tool defs | **durable** | **SQLite** |
| Todos, scratchpads, key-value, locks/leases | **durable** | **SQLite** |
| `solo.yml` | **durable, user-owned** | the repo file (we never rewrite it silently) |

- **Repository pattern** over SQLite (`Store` port): each aggregate (TodoRepo, ScratchpadRepo,
  KvRepo, LockRepo, TrustRepo, ProjectRepo, SettingsRepo) has a focused trait. SQLite in **WAL mode**,
  versioned **migrations**, all writes in transactions.
- **Optimistic concurrency** for scratchpads/todos: persisted `revision`; `*_write` takes an
  `expected_revision` and fails on mismatch (matches Solo's revision-guarded writes, ref §7). This is
  how concurrent agents don't clobber shared notes.
- **Leases** carry an explicit **TTL + owner `ProcessId`**; expired or owner-closed → auto-released
  (closes Solo's documented gap, ref §7/§12).
- **Crash recovery:** on launch, reconcile the runtime-state file with live PIDs → adopt/kill/leave
  orphans (ref §4); durable state in SQLite is always consistent because writes are transactional.
- Data dir honors `SOLOIST_APP_DATA_DIR` (our analog of `SOLOTERM_APP_DATA_DIR`); default XDG
  `~/.local/share/soloist/`.

---

## 8. Longevity: resource bounds, backpressure, anti-rot (the core requirement)

Every unbounded thing is a future crash. Hard rules:

- **Bounded buffers, always.** Per-process log ring buffer (default 5,000 lines) and raw scrollback
  byte cap (default 256 KB); oldest dropped. A global cap across all processes too. No buffer grows
  without a ceiling.
- **Backpressure, never drop silently.** Bounded channels between the PTY reader and consumers; if the
  UI can't keep up with a chatty process, we **coalesce** output per animation frame and the ring
  buffer remains source of truth — we slow producers via bounded queues, we don't OOM.
- **Rate-limit & debounce by design** (matches Solo, ref §4): crash auto-restart capped at **10/60s →
  RestartExhausted**; file-watch events **debounced** into a quiet window; metrics sampled on an
  interval (~1s), not per event; idle summaries cadence-limited.
- **Guaranteed OS-resource reclamation.** Every spawn is into a fresh **process group**; every
  stop/restart/shutdown/cancel signals the **group** (SIGTERM→grace→SIGKILL) and **waits** (reaps) so
  no zombies/orphans accumulate. PTY masters and file handles are closed in `Drop`/cancel paths. A
  start/stop loop of N processes must end at the **same** PID/FD count it started with — asserted in a
  soak test.
- **No leaks over time.** Subscriptions are dropped when panels close; timers/watchers are cancelled
  when their owner closes; SQLite connections are pooled, not per-call. A multi-hour soak (Phase 13)
  asserts flat RSS, flat FD count, flat task count.
- **Graceful degradation.** If summarization (LLM) is unavailable → idle detection falls back to the
  heuristic-only signal and the app keeps working. If port discovery (`/proc`) fails → ports show
  "unknown", supervisor unaffected. No optional subsystem can take down the core.
- **Deterministic shutdown ordering:** stop accepting commands → cancel watchers/timers/samplers →
  `stop_all()` processes (reap) → flush SQLite → exit. Matches Solo's "closing stops all processes"
  (ref §10) and guarantees no orphans on quit.

---

## 9. Cross-cutting patterns (catalog)

| Pattern | Where | Why |
|---------|-------|-----|
| Ports & Adapters (hexagonal) | whole app | testability, swap WebKit/MCP/SQLite |
| Actor / supervision tree | C2 process actors, internal tasks | race-free ownership, fault isolation |
| Finite state machines | `ProcStatus`, `AgentActivity`, `Trust` | legal transitions only |
| Event-driven + CQRS-lite | core↔adapters | one behavior, many frontends; cheap reads |
| Repository + Unit of Work | C6/C1 over SQLite | clean persistence, transactional writes |
| Optimistic concurrency (revisions) | scratchpads/todos | safe concurrent agents |
| Lease/lock | C6 coordination | cooperative multi-agent, auto-release |
| Newtype IDs + closed enums | domain types | compiler-enforced correctness |
| Strategy (per-provider) | C4 idle heuristics, agent tool types | Claude/Codex/Gemini differ cleanly |
| Anti-corruption layer | C8 façade | adapters never see core internals |

---

## 10. Code organization & standards

**Workspace (Rust):**
```
crates/
  core/    # C1–C8, ports (traits), domain types, event bus — NO tauri/mcp/axum/sqlite imports
  store/   # SQLite implementation of the Store port (repos, migrations)
  pty/     # ProcessSpawner impl over portable-pty + nix (could fold into an adapters crate)
  app/     # Tauri binary + command/event adapter + bundled UI
  mcp/     # `soloist-mcp` binary: MCP (stdio) adapter over the core via IPC
  httpapi/ # local HTTP API (127.0.0.1) adapter
  cli/     # `soloist` CLI (thin HTTP client, ref §8)
  ipc/     # app<->mcp UDS transport + request/response message types shared by app/mcp/httpapi/cli
```
**Frontend (`crates/app/ui`, React/TS):** `api.ts` (typed `invoke`/`listen` only) · `store/` (event
reducer → read-model) · `components/` · `views/`. **No business logic in React** — it renders
projections and sends commands.

**Standards (CI-enforced):**
- `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` in `core`; `clippy -D warnings`
  workspace-wide; `rustfmt`; `tsc --noEmit`; ESLint.
- **Module size discipline:** when a file does more than one thing, split it. Favor many small,
  single-purpose modules over god-files (also keeps each unit in-context for reliable edits).
- **Doc comments** on every public item: what it does, how to use it, what it depends on.
- **Dependency-direction test** in CI: fail the build if `core` imports a forbidden adapter crate.
- Errors are values; logging via `tracing` spans keyed by `ProcessId`/`ProjectId`.

---

## 11. Testing strategy (per `04` because architecture *is* testability)

- **Domain unit tests (headless, deterministic):** every FSM transition, restart backoff/rate-limit,
  debounce, trust gating, revision conflicts — using the **mock `Clock`** so no real time passes.
- **Adapter contract tests:** the MCP/HTTP/CLI adapters each tested against a fake core to lock their
  wire contracts; the SQLite `Store` tested against the repo traits.
- **Integration (real OS, fixture processes):** `sleep`, `bash -c 'exit 3'`, chatty loop, child-spawner
  (pgroup kill), `python -m http.server` (ports/readiness), `read x` (interactive input).
- **Property tests:** `solo.yml` round-trips; ring buffer never exceeds cap; no FSM reaches an illegal
  state.
- **Soak/leak tests (the longevity gate, Phase 13):** hours-long run with periodic crashes/restarts →
  assert flat RSS, FD count, task count, zero leaked PIDs.
- **e2e (Playwright via webapp-testing):** dashboard, terminal interactivity, trust dialog, palette.

---

## 12. Security

- **Trust gate** (ref §4) is enforced **in the core**, not the UI: `start*`/`restart*`/auto-* refuse
  untrusted command variants regardless of which adapter asks. Trust is per (project, command-variant
  hash), persisted in SQLite.
- **HTTP API** binds loopback only, requires `X-Soloist-Local-Auth` on mutations, CORS localhost-only
  (ref §8).
- **MCP** action tools honor the trust gate and the **effective project scope**; identity via bound
  `ProcessId`/`register_agent`/`whoami` (ref §7). A tool cannot touch another project's state.
- Child processes get a sanitized env (ref §5 precedence); Soloist-internal vars stripped except the
  injected `SOLOIST_PROCESS_ID`.

---

## 13. Anti-patterns we explicitly forbid

- Business logic in a Tauri command handler or React component (must live in `core`).
- Shared mutable domain state behind a big `Mutex` (use actors + messages).
- Unbounded buffers, channels, or retry loops (every one needs a cap/backoff).
- `unwrap()`/`panic!` in long-running paths.
- Killing a PID instead of its process group (orphan risk).
- Re-implementing an action per adapter (must route to one core command).
- Letting an optional subsystem (summarizer, ports, notifications) be able to crash the core.

---

## 14. Longevity checklist (Phase 13 verifies each)

- [ ] Start/stop 100 processes in a loop → identical PID & FD count at end.
- [ ] 6-hour soak with random crashes → flat RSS, flat task count, zero zombies.
- [ ] Chatty process (MB/s output) → bounded memory, UI stays responsive (backpressure works).
- [ ] Kill the metrics sampler task → it self-restarts; app unaffected.
- [ ] Pull the summarizer (LLM offline) → idle detection degrades gracefully, no crash.
- [ ] Force-quit the app mid-run → next launch adopts/cleans orphans; SQLite state intact.
- [ ] `solo.yml` edited 50× rapidly → debounced to few sync prompts, no runaway restarts.
- [ ] Dependency-direction CI check is green (`core` has no adapter imports).
