# 06 — Codebase Blueprint & Cleanup Roadmap

> **What this file is.** `04-engineering-architecture-and-patterns.md` is the *design contract* — the
> principles (hexagonal, actors, FSMs, bounds, longevity). **This file is the concrete blueprint derived
> from those principles**: exactly *where* every kind of code lives, *which* design pattern to reach for
> *where*, the *step-by-step recipes* for adding the things this app will grow (contexts, adapters, MCP
> tools, endpoints, UI, events), and a *phased cleanup roadmap* to bring the current tree fully onto this
> bar. It exists so every future session architects changes the same way — no drift.
>
> **It is not a second source of truth.** If this file ever disagrees with `04`/`05`/`02`, the higher doc
> wins (`CLAUDE.md` §2) and this file is corrected. `04` says *what rule*; this file says *where it lands*.

**Read order for a structural change:** `CLAUDE.md` §1 protocol → `04` (the rule) → this file (the where) →
the phase file (the task).

---

## 1. The shape in one screen

```
 DRIVING adapters (call the core)            crates/
   Tauri UI ── crates/app                      core/    pure domain — NO tauri/rmcp/axum/rusqlite/notify-rust
   MCP      ── crates/mcp                       store/   SQLite adapter (Store/ProjectRepo/TrustRepo/…)
   HTTP/CLI ── crates/httpapi, crates/cli       pty/     PTY adapter (ProcessSpawner/PtyIo/OrphanControl)
        │  (each its own crate, core-only dep)  app/     Tauri binary + UI + composition root
        ▼                                       mcp/     soloist-mcp binary (stdio)              [stub→P8]
   ┌─────────────────────────────┐              httpapi/ loopback HTTP adapter                  [stub→P10]
   │  crates/core::Facade  (C8)  │              cli/     soloist CLI (HTTP client)               [stub→P10]
   │  bounded contexts C1–C8     │              ipc/     app↔mcp transport + shared msg types    [stub→P8]
   │  ports (traits) · event bus │
   └─────────────────────────────┘
        │  (driven ports — traits the core defines)
        ▼
 DRIVEN adapters (the core calls)
   ProcessSpawner→pty   Store→store   Clock→core   FileWatcher/Notifier/Summarizer→later phases
```

Two directions of dependency, **one rule**: everything points *at* `core`; `core` points at *nothing
app-specific*. Enforced by `scripts/check-core-deps.sh` (parity **K7**). This single rule is what makes
the whole app headless-testable and is the mechanical guarantee behind "remove an adapter, the app still
builds and runs" (§8).

---

## 2. Crate topology (authoritative placement of crates)

| Crate | Kind | Owns | May depend on | Status |
|-------|------|------|---------------|--------|
| `core` | domain | C1–C8, ports (traits), domain types, event bus | `tokio`/`serde`/`thiserror`/`vte`/etc. — **never** an adapter crate | live (C1–C3 + C8) |
| `store` | driven adapter | SQLite impl of `Store`/`ProjectRepo`/`TrustRepo`/`RuntimeState` + migrations | `core`, `rusqlite` | live |
| `pty` | driven adapter | `ProcessSpawner`/`PtyIo`/`ProcessControl`/`OrphanControl` over `portable-pty`+`nix` | `core`, `portable-pty`, `nix` | live |
| `app` | driving adapter + host | Tauri shell, command/event wiring, **the composition root**, bundled UI | `core`, `store`, `pty`, `httpapi`, `tauri` | live |
| `mcp` | driving adapter | `soloist-mcp` stdio binary → core over `ipc` | `core`, `ipc`, `rmcp` | stub → P8 |
| `httpapi` | driving adapter | loopback `127.0.0.1:24678` over `axum` | `core`, `ipc`, `axum` | stub → P10 |
| `cli` | driving adapter | `soloist` CLI = thin HTTP client | `ipc`, `clap` (not `core` directly) | stub → P10 |
| `ipc` | shared contract | app↔mcp UDS transport + request/reply message types | `serde` only | stub → P8 |

**Rules that keep this from rotting:**
- A new *external integration* (a new way to drive or be driven) is a **new crate or an existing adapter
  crate**, never logic added to `core` and never logic added to another adapter.
- An adapter crate holds **no business state and no business decisions** — it translates a wire format to a
  `Facade` call and a read-model back. If you're writing an `if` about *domain* meaning in an adapter,
  it belongs in a context.
- `cli` talks to `httpapi` over the wire (it is a *client*), so it depends on `ipc` types, **not** `core`.
  This is deliberate: the CLI is process-isolated from the engine (`05` §8).

---

## 3. Where everything lives (the placement map)

### 3.1 `crates/core` modules → bounded contexts

| Module(s) | Context | Holds | Phase |
|-----------|---------|-------|------|
| `config/` · `projects` · `trust` · `hash` · `debounce` | **C1 Projects & Config** | `solo.yml` parse/validate/sync, project registry, trust gate, content hashing, quiet-window debounce | live (P2) |
| `supervisor/` · `process` · `orphans` | **C2 Process Supervision** | registry, `ProcStatus` FSM, start/stop/restart, bulk ops, orphan reconcile | live (P3) |
| `terminal/` | **C3 Terminal I/O** | PTY read loop, rendered+raw buffers, OSC parse, attach replay | live (P4) |
| `agents` · `idle` | **C4 Agents & Idle** | agent-tool defs, launch, 5-state idle FSM, optional summary | **placeholder → P7** |
| `metrics` · `portscan` | **C5 Monitoring** | CPU/mem sampling, `/proc` port discovery, readiness | **placeholder → P6** |
| `coordination` | **C6 Coordination** | scratchpads, todos, timers, leases, key-value | **placeholder → P9** |
| `notify` | **C7 Notifications** | crash/attention/idle toasts, unread/bell state | **placeholder → P6** |
| `facade` · `identity` | **C8 Integration façade** | the public command/query API; MCP identity & effective scope | live (`facade`) / placeholder `identity` → P8 |
| `events` | cross-cutting | typed `DomainEvent` + `EventBus` (broadcast) | live |
| `ports` | cross-cutting | every port trait + its `Noop*` default | live |
| `ids` | cross-cutting | newtype IDs (`ProcessId`, `ProjectId`, …) | live |
| `sync` | internal util | poison-safe `lock()` helper | live |
| `testing` | test support | fakes + `MockClock` (see §6) | live, test-gated |

**The placeholder-module rule (the one allowed empty module).** An empty `pub mod foo;` is acceptable
**only** when `foo` maps to a documented future bounded context in this table and `01-architecture.md`'s
module table. It must carry a doc comment naming its context + the phase that fills it, and export nothing
until then. This is *a roadmap marker, not dead code* — distinct from a leftover empty file, which is
forbidden (`CLAUDE.md` §15). When a phase fills a placeholder, it stops being a placeholder; the bar in
§3.2 applies.

### 3.2 Within a context: the standard internal layout

A context that outgrows one file uses a module folder (as `config/`, `supervisor/`, `terminal/` already
do). Inside it:
- **`mod.rs`/`<context>.rs` (the root)** — the context's *published surface*: its public types and the thin
  orchestrating methods. Keep it small; it re-exports submodules. Target ≤ ~250 lines of code.
- **one submodule per cohesive concern** — e.g. `supervisor/{registry, actor, adopt}`. A submodule does
  one thing and owns its own inline tests.
- **types vs behavior** — closed enums + newtypes + FSM transition functions live with the smallest unit
  that owns them; they are `pub` only as far as needed.
- **the ~400-line split smell** (`CLAUDE.md` §15, counting *non-test* lines): when a `.rs` crosses it,
  split by concern. (`supervisor.rs` was the last outlier, split into `supervisor/` submodules in R2 and
  `core::testing` into `testing/` submodules in R5; the file-size guard now reports zero outliers — §7.)

### 3.3 Frontend (`crates/app/ui/src`) — the placement map

| Path | Role | Rule |
|------|------|------|
| `domain.ts` | the **single** TS mirror of core enums/`DomainEvent` | one definition per type; mirrors serde output; nowhere else |
| `api.ts` | typed `invoke`/`listen` + Channel only | **every** Tauri command/event name string lives here once |
| `store/` | read-model: pure reducers (`projection`, `grouping`) + hooks (`useProcesses`, …) | reducers are pure + unit-tested; hooks own subscriptions |
| `lib/` | pure presentational helpers (`status.ts` = the single `ProcStatus`→glyph/color/label map) | no IPC, no state |
| `components/` | small presentational components, grouped by surface (`sidebar/`, `terminal/`) | props-in/callbacks-out; **no** business logic, **no** `invoke` |
| `*.test.ts(x)` | vitest, beside the unit they test | exercises real logic; deletable-on-sight if tautological |

The frontend is already in good shape (largest file 121 lines, clean split). The rule is to **keep** it
this way: business decisions never migrate into a component, and a component never calls `api.ts` directly
— it receives data from a hook and emits callbacks.

---

## 4. Design patterns — which, where, and when

`04` §9 lists the catalog. This is the *actionable* version: the pattern, its concrete home, and the
trigger that tells you to reach for it. Use a pattern when its trigger fires — not preemptively (YAGNI).

| Pattern | Where it lives now / will live | Reach for it when… |
|---------|-------------------------------|--------------------|
| **Ports & Adapters** | the whole app (`ports/` + adapter crates) | any OS/UI/transport/storage concern appears → define a trait in `core`, implement in an adapter |
| **Facade / Anti-corruption layer** | `core::facade::Facade` (C8) | an adapter needs to *do* something → it calls one `Facade` method, never a context internal |
| **Actor + supervision** | `supervisor/actor.rs` (one task per process) | a resource needs a single owner racing events (child vs stop) → give it a task + bounded `mpsc` |
| **Finite state machine** | `ProcStatus::transition`, future `AgentActivity`, `Trust` | state has *legal transitions* → encode as `Result<New, IllegalTransition>`, never field mutation |
| **Observer (event bus)** | `events::EventBus` (broadcast) | a state change must reach N adapters → emit a `DomainEvent`; never call adapters back directly |
| **CQRS-lite** | `Facade::snapshot` (query) vs `supervisor.start` (command) | reads must not block writes → cheap projection for reads, owning context for writes |
| **Repository** | `store` (`ProjectRepo`/`TrustRepo`/…); future Todo/Scratchpad/Kv/Lock repos | durable aggregate → one focused trait per aggregate, SQLite behind it |
| **Newtype + closed enum** | `ids.rs`, `process.rs` | a domain id/state → newtype/enum, never a bare `String`/`int` |
| **Null Object** | `Noop{LockReleaser,RuntimeState,OrphanControl}` in `ports/mod.rs` | a **driven** subsystem is optional → ship a `Noop` default so the core runs without the real adapter (§8) |
| **Parameter Object / Builder** | `core::ports::CorePorts` + `CorePortsBuilder` — the port set for `Facade::new`/`Supervisor::new` | a constructor passes >4 collaborators (`too_many_arguments`) → group them in a struct/builder |
| **Registry** | **to add** — MCP tool registry (P8), agent-tool defs (P7) | a growing set of "one of many" handlers → register entries, don't extend a giant `match` |
| **Strategy** | **to add** — per-provider idle heuristics (P7), per-agent-tool launch (P7) | behavior varies by a closed set of providers → one trait, one impl per provider |
| **Optimistic concurrency** | **to add** — scratchpad/todo `expected_revision` (P9) | concurrent writers to one durable record → revision guard, reject stale writes |
| **Lease/lock** | **to add** — coordination locks (P9) | cooperative cross-agent intent → TTL + owner `ProcessId`, auto-release on close |

Anti-patterns to refuse are fixed in `04` §13. The one most relevant to this app's growth: **a giant
`match` over tool/provider/endpoint names**. When that set is open-ended, use a Registry or Strategy.

---

## 5. Recipes (how to add the things this app will grow)

Each recipe is a closed checklist. Follow it and the change lands in the right layer with single-source,
DRY, and the dependency rule intact. These *are* the "how future sessions architect changes" contract.

### 5.1 Add behavior to an existing context (e.g. a restart-policy rule)
1. Put the logic in the **owning context** module (`04` §3 table). Never in an adapter or the frontend.
2. If it has states, express transitions as FSM functions returning `Result<_, IllegalTransition>`.
3. If it needs OS/time/IO, take it through an **existing port**; add a new port only if none fits (§5.2).
4. Emit a `DomainEvent` if adapters must observe the change; extend the projection (§5.6) on the TS side.
5. Add inline unit tests using `core::testing` fakes + `MockClock` (no real time, no real OS).
6. Expose it to adapters by adding (or reusing) **one** `Facade` method — never per-adapter logic.

### 5.2 Add a new port + driven adapter (e.g. metrics sampler, file watcher)
1. Define the **trait** in `core::ports` with a doc comment stating its contract; add a `Noop` default if
   the subsystem is optional (so the app runs without it — §8).
2. Implement the trait in the **right adapter crate** (`store` for durable, `pty` for OS/process, or a new
   adapter crate if it's a genuinely new technology — justify a new crate against the size budget, `04` §10).
3. Wire it in the **composition root** (`app::build_facade`, §8) — pass the real adapter; tests pass a fake.
4. Add a fake to `core::testing` so every consumer tests against one shared fake (§6).
5. Keep the port **minimal**: add methods when a phase needs them (the `FileWatcher`/`Notifier`/`Summarizer`
   stubs are methods-less on purpose until their phase).

### 5.3 Add an MCP tool (Phase 8+)
1. The tool is a **thin handler in `crates/mcp`**: parse params (clean-room JSON Schema, `04`/`09`), call **one
   `Facade` method**, map the result to the MCP wire type. No domain logic in the handler.
2. Register it in the mcp crate's **tool registry** (Registry pattern) — do not grow a hand-written match.
3. If the behavior doesn't exist on the `Facade` yet, add it via §5.1 first, then call it. The tool is a
   *caller*, never an *owner*, of behavior.
4. Honor the **trust gate + effective scope in the core** (not the handler): the `Facade` method enforces
   them, so HTTP/CLI/UI get the identical guarantee for free (`04` §12).
5. Removing the whole MCP surface = remove `crates/mcp` from the workspace members + the app's launch of the
   sidecar. The core and every other adapter are untouched (§8).

### 5.4 Add an HTTP endpoint / CLI command (Phase 10)
1. Endpoint handler in `crates/httpapi` → one `Facade` method (mutations require `X-Soloist-Local-Auth`,
   loopback + localhost CORS, `05` §8). CLI subcommand in `crates/cli` → one HTTP call to that endpoint.
2. Same behavior as the UI button and the MCP tool because all three route to the **same** `Facade` method.
   If you find yourself reimplementing the behavior, stop — route to the core.

### 5.5 Add a Tauri command (UI ↔ core)
1. Add a thin `#[tauri::command]` in `crates/app/src/commands.rs` → one `Facade` method.
2. Register it in the `invoke_handler!` list in `app/src/lib.rs` (the command name = the fn name; single
   source).
3. Add the typed wrapper in `ui/src/api.ts` (the command-name string lives **only** here on the TS side).
4. Never put logic in the command handler; it marshals types and calls the core.

### 5.6 Add a `DomainEvent` variant (cross-boundary change)
1. Add the variant to `core::events::DomainEvent` (serde `#[serde(tag = "type")]` → the discriminator is the
   variant name; no hand-written string).
2. Mirror it in `ui/src/domain.ts`'s `DomainEvent` union (the **one** TS definition) and handle it in the
   `store/projection.ts` reducer's exhaustive switch.
3. The event-channel name (`"domain-event"`) is one named constant per side (`app/src/lib.rs`,
   `ui/src/api.ts`) — this cross-language pair is the *allowed* duplication (`CLAUDE.md` §15: "one named
   constant on each side"); do not add a third occurrence.

### 5.7 Add UI (always via `/impeccable`, `CLAUDE.md` §5)
1. Types from `domain.ts`; data from a `store/` hook; status display from `lib/status.ts`.
2. New presentational component under `components/<surface>/`; props-in/callbacks-out; no `invoke`.
3. Reuse existing primitives (shadcn `Button`, `StatusIndicator`, `ProcessControls`) — never re-roll markup.

### 5.8 Use the coordination layer — create → delegate → use (C6, Phase 9)

Coordination (scratchpads, todos, timers, leases, key-value) is **durable, project-scoped state in SQLite**
that agents share to orchestrate each other token-free — what makes Soloist a *metaharness* (`00`, `05` §1).
It is context **C6** (`core::coordination`), built in **Phase 9**; the MCP tool *names* are cited in `05` §7,
but their **parameter schemas are clean-room and undesigned** (`05` §12, decision D7) — design them per-tool
when Phase 9 lands, don't invent them here. All three stages route through the **same `Facade`**, so the UI,
MCP, and HTTP/CLI behave identically (one behavior, many fronts).

**Create** (any adapter, identical path):
1. An agent calls a coordination MCP tool (`scratchpad_write`, `todo_create`, `kv_set`, `lock_acquire`,
   `timer_set`) → the `mcp` handler (§5.3) → **one `Facade` method** → the C6 aggregate → its `*Repo`
   (SQLite, transactional). No domain logic in the handler.
2. Writes to a shared record are **revision-guarded** (optimistic concurrency, `04` §7): `*_write` takes an
   `expected_revision` and returns `RevisionConflict` on a stale write (matrix **G2**) — how concurrent agents
   avoid clobbering a scratchpad.
3. Identity & scope are resolved **in the core**, not the handler: a call acts on the **effective project
   scope** and is attributed to the **bound process** (`SOLOIST_PROCESS_ID` → `bind_session_process`; external
   callers `register_agent`; `whoami` reports it — `05` §7). A tool cannot touch another project's state.

**Delegate** (a lead agent orchestrating workers):
1. Lead spawns a worker with `spawn_agent`/`spawn_process` (C2/C4, MCP **F11**); the worker auto-binds via the
   injected `SOLOIST_PROCESS_ID`.
2. Lead hands work as **todos** — `todo_create`, then `todo_transfer`/`todo_lock`/`todo_set_blockers` to assign,
   reserve, and order it (**G3–G5**). A **lease** (`lock_acquire`, TTL + owner, **G6**) signals cooperative
   intent — "signals, not ownership" (`05` §7).
3. Lead **waits without polling**: `timer_fire_when_idle_all` (**G8**) resolves when its `waiting_on` processes
   go idle (the C4 idle FSM flips them) and delivers the timer's `body` to the lead as a fresh user turn (`01`
   data-flow; `timer_set` body semantics, `05` §7).

**Use & release** (the lifecycle invariant):
1. Workers read/write the same scratchpads/todos/kv: reads are cheap projections, writes go through the owning
   aggregate (CQRS-lite). Small structured shared state uses key-value (`kv_*`, default off, **G10**).
2. **Process-owned locks auto-release when the owning process closes** (todo locks + leases): the supervisor's
   stop hook calls the `LockReleaser` port on **any** terminal transition (matrix **B7/G5**; `NoopLockReleaser`
   until C6 lands, §5.2). A crashed or stopped worker never strands a lock.
3. It all **persists across an app restart** (SQLite, **G11**): todos/scratchpads survive even though live
   processes and PTY buffers don't (the ephemeral-vs-durable split, `04` §7).

> This is the **target** design — C6 is a placeholder until Phase 9. It is grounded in `05` §7 + `01`'s
> data-flow walkthroughs; per-tool param schemas and exact semantics are designed in Phase 8/9, not here.

---

## 6. Single source of truth, DRY & the test-fakes gap

Single-source is already strong (one `domain.ts`, one `lib/status.ts`, one command/event name per side).
The rules to **hold**:
- Every status/kind/event/command/limit/path is defined **once** (Rust enum in `core`; TS mirror in
  `domain.ts`). A numeric bound is a named `const`, never a literal at the comparison site.
- The Rust↔TS mirror is the only sanctioned duplication, and only as *one constant/type per side*. When the
  surface grows painful, evaluate generating TS from Rust (e.g. `ts-rs`) — flagged for the user (build/size
  trade-off), not adopted speculatively.

**Shared test fakes — resolved (R1/R5).** `core::testing` (the `FakeSpawner`, `MockClock`, `FakeTrustRepo`,
`FakeProjectRepo`, …) is exposed behind a Cargo `testing` feature —
`#[cfg(any(test, feature = "testing"))] pub mod testing;` — so `store`/`pty` (and the future `mcp`/`httpapi`
adapters) reuse the **one** fake set via `soloist-core = { path = "../core", features = ["testing"] }` in
their `[dev-dependencies]` instead of re-rolling fakes. The fakes live in per-concern submodules under
`core::testing/` (`clock`/`spawner`/`repos`/`runtime_state`/`lock_releaser`/`fixtures`, split in R5); the
feature is off by default, so they never compile into a production build. Tests stay inline per the project
decision — this changed *who can reach the fakes*, not *where tests live*.

---

## 7. Cleanup roadmap (phased — each phase ends green)

The codebase is at build-Phase 5 (`Done — pending verify`); Phases 1–4 are verified-pending. The cleanup is
sequenced so no phase regresses that code blindly: every phase **starts and ends with `just lint && just
test` green** (current baseline **106 tests**), changes one concern, and is independently reviewable. These
are **R-phases** (refactor), orthogonal to the build phases.

> **Decisions already locked by the user (2026-06-18):** tests stay **inline** (trim, don't relocate); the
> empty core modules **and** the four stub adapter crates **stay** as documented placeholders (§3.1).

> **Status (2026-06-19): R0–R6 are all complete.** Commits: R0 `ea4bad1` · R1 `4c80eb7` · R2 `c04859a` ·
> R3 `71eafac` · R4 `65cf819` · R5 `3f07350` · R6 (this convergence). See `PROGRESS.md` for per-phase
> evidence. The phase descriptions below are kept as the record of the executed cleanup.

### R0 — Blueprint & guardrails (this session; docs only, no code logic)
- Write this file; add the architecture section to `CLAUDE.md` (done with R0).
- Add `scripts/check-file-size.sh` to `just lint` + CI: **warn** (non-blocking first) on any non-test source
  `.rs`/`.ts`/`.tsx` over the ~400 *non-test*-line smell, the way `check-core-deps.sh` guards layering.
- **Done when:** docs merged; the file-size check runs in `just lint` and reports the current outliers.

### R1 — Reusable test support (single-source fakes)
- Re-gate `core::testing` behind a `testing` feature and make it `pub` (§6); switch `core`'s own tests to it
  unchanged; add the dev-dependency feature to `store`/`pty`.
- **Done when:** `core`, `store`, `pty` all build their tests against the one `core::testing` set; no fake is
  defined twice; `just test` green with the same count.

### R2 — Split the god-file (`supervisor.rs`)
- `supervisor.rs` is 491 code lines (+573 inline tests) over the ~400 smell. Pull cohesive concerns into
  `supervisor/` submodules (candidates: bulk ops, `reconcile_orphans`, the `Registration`/`StartSummary`/
  error types), leaving the root as the thin C2 surface. Inline tests move **with** their code (each
  submodule owns its tests — the project's inline-test decision).
- **Done when:** no `supervisor` source file exceeds the smell; the public surface (`lib.rs` re-exports) is
  unchanged; `just lint && just test` green.

### R3 — Composition root + ports parameter object
- Introduce a `CorePorts` struct (the set of `Arc<dyn Port>` the core needs) and a builder; refactor
  `Facade::new` to take it, **removing both** `#[allow(clippy::too_many_arguments)]` (facade.rs:51,
  supervisor.rs:138). Document `app::build_facade` as **the single composition root** and the rule: exactly
  one per binary; optional subsystems default to their `Noop` port (§8).
- **Done when:** no `too_many_arguments` allows remain; adding a future port = one field on `CorePorts`; the
  composition-root rule is in this file + `CLAUDE.md`; green.

### R4 — Purge scaffolding from the pure core
- `core::facade::spawn_demo_process` (+ `DEMO_PROJECT`/`DEMO_COMMAND`, `std::env::current_dir`) is demo
  scaffolding living in the *pure core*, kept alive only by `pty/tests/integration.rs:262` — duplicating
  `app/src/demo.rs`. Move the demo seam out of `core`: the integration test builds its own `Registration`
  (optionally via a `core::testing` helper); demo seeding lives **only** in the `app` adapter.
- Sweep `core` for any other host/demo concern, restating comments, or unused `pub` exports.
- **Done when:** `core` carries no demo/`std::env` scaffolding; the integration test still proves the same
  facade path; green.

### R5 — Honest-test audit
- Walk every test (Rust inline + vitest). Delete tautological/pretend tests; confirm each remaining test can
  fail for a real reason; record honestly any module whose real coverage is thin (no pretend test to fill
  the gap). Verify the small ones explicitly (e.g. `ui/src/lib/utils.test.ts`, 12 lines).
- **Done when:** every test asserts real behavior; the count change (if any) is explained in `PROGRESS.md`;
  green.

### R6 — Converge docs & ledger
- Reconcile any plan-doc drift this cleanup surfaced (e.g. `03` listed `serde_yaml` though we ship
  `serde_norway` — a stale-doc fix, **not** a Solo-behavior divergence, so it stays out of
  `KNOWN-DIVERGENCES.md`; also the post-R0–R5 structural moves: `ports/`, `supervisor/`, `core::testing/`,
  the file-size guard now live); update `PROGRESS.md` (status, evidence, next pointer) and
  `KNOWN-DIVERGENCES.md` only if a genuinely new intentional divergence was introduced (none this cleanup).
- **Done when:** docs match the tree; `PROGRESS.md` reflects the cleanup; all gates green.

**Sequencing rationale:** R0 sets the bar and the file-size signal; R1 makes the later phases' tests cheap
to keep honest; R2/R3/R4 are the structural edits (smallest blast radius first: split, then the constructor,
then scaffolding removal); R5 is best done after the structure settles; R6 closes the ledger. Each is a
single reviewable commit.

---

## 8. Adapter independence — "remove MCP and the app survives" (the guarantee, made concrete)

This is the user's explicit requirement, and it decomposes into two mechanisms — one per dependency
direction:

**Driving adapters (MCP, HTTP, CLI, the UI) are independent crates depending only on `core`/`ipc`** — the
core has **zero** knowledge of them, and the dependency-direction guard (K7) makes the reverse dependency
*impossible to introduce by accident*. But "removable" has **two shapes**, and they must not be conflated:

- **Out-of-process adapters — separate binaries (trivially removable).** `crates/mcp` (`soloist-mcp`) and
  `crates/cli` (`soloist`) compile to their own binaries that the app never links. To remove MCP entirely:
  drop `crates/mcp` from the workspace `members`, stop launching the sidecar from the composition root, and
  drop its `ipc` message types. `core`, `store`, `pty`, `app`, `httpapi` are untouched and still build/run —
  nothing in them references `mcp`. The app degrades to "no MCP integration," not "broken." Same for the CLI.
- **In-process adapters — linked into the `app` binary (must be feature-gated to be removable).** The
  loopback **HTTP API (`crates/httpapi`)** is a library crate compiled *into* `app` and run as a supervised
  task inside the app process; the **Tauri command surface** is likewise intrinsic to the app binary.
  Dropping `crates/httpapi` from the workspace does **not** by itself leave `app` building — `app` links it.
  So an in-process adapter that must be optional is gated **at compile time behind a Cargo feature**
  (`app/Cargo.toml` `[features] http = ["dep:soloist-httpapi"]`, the composition root starts it only under
  `#[cfg(feature = "http")]`) **or at runtime** (a setting that simply never starts the server task). The
  Tauri UI is not "removable" in this sense — it *is* the app. Rule: **a new driving adapter that lives
  in-process ships behind a feature flag or a runtime toggle from day one**, so "turn it off" never means
  "edit `app` and hope."

**Driven adapters (optional subsystems the core calls) use the Null Object pattern.** A subsystem the core
*uses* but that may be absent (lock releaser, runtime-state/orphan adoption, file watcher, notifier,
summarizer) is a **port with a `Noop` default**. The core always holds *a* `Arc<dyn Port>`; if the real
adapter isn't wired, it holds the `Noop`. The port set is bundled in **`core::ports::CorePorts`** (a
parameter object, R3) whose **builder defaults the optional subsystems to their `Noop` port**: the lock
releaser defaults to `NoopLockReleaser` (coordination lands in C6), and `build_facade` degrades
runtime-state to `NoopRuntimeState` when the data dir is unavailable — the supervisor never branches on
"is coordination present?", it just calls `release_all` and the `Noop` swallows it. A future optional port
is **one field on `CorePorts`** with a `Noop` default, so existing composition roots and tests are
untouched (§5.2 step 1).

**The composition root is the single place these choices are made.** `crates/app/src/lib.rs::build_facade`
is the one function that picks real-vs-`Noop` adapters, assembles a **`CorePorts`** (via its builder), and
hands it to `Facade::new`. There is **exactly one composition root per binary** (the app; later,
`soloist-mcp` has its own minimal one). Rules:
- No other code constructs adapters or decides real-vs-fake — they receive an assembled `Facade`/`CorePorts`.
- Optional subsystems absent from the builder fall back to their `Noop` port; the app still launches.
- Tests are "alternate composition roots": they assemble a `CorePorts` from `core::testing` fakes (§6).

The payoff: a subsystem can be added, removed, or swapped by editing one crate's membership + the
composition root, with the type system and CI proving the rest of the app is untouched.

---

## 9. Enforcement (what guards what)

| Invariant | Gate | Status |
|-----------|------|--------|
| `core` imports no adapter crate | `scripts/check-core-deps.sh` (`just lint`, CI) | live (K7) |
| No file-size god-files | `scripts/check-file-size.sh` (`just lint`, CI) | live (warn-only) |
| No `unwrap`/`expect`/`panic` in `core` long-running paths | `#![deny(clippy::unwrap_used,…)]` in `core` | live |
| No clippy warnings / formatting drift | `clippy -D warnings`, `rustfmt`, `tsc`, ESLint, Prettier | live |
| Closed-enum exhaustiveness across the boundary | exhaustive `match` (Rust) + exhaustive switch (`projection.ts`) | live |
| Behavior parity across adapters | every adapter routes to **one** `Facade` method (review-enforced) | convention |
| Honest tests | the §15/`04` discipline gate (per-phase review) | convention |

Conventions become gates when cheap to mechanize (the file-size check is the next one). Everything else is a
per-phase review item under the codebase-discipline gate (`CLAUDE.md` §7 item 6, §15).

---

## 10. The one-paragraph contract for a future session

Behavior lives in a **bounded context** in `crates/core`, behind **ports**, exposed through the **one
`Facade`**. OS/UI/transport/storage are **adapter crates** that depend only on `core` and route to that
`Facade` — never the reverse (CI-enforced). Optional subsystems are **ports with `Noop` defaults**, wired in
the **single composition root** (`app::build_facade`), so any one can be absent without breaking the app.
Every concept is defined **once** (Rust enums in `core`, the TS mirror in `domain.ts`); shared test fakes
live once in `core::testing`. Reach for a design pattern when its **trigger** (§4) fires, not before. Files
stay **small and single-purpose**; tests stay **inline but honest**. When in doubt, follow the recipe in §5;
when the recipe and a higher doc disagree, the higher doc wins and this file is fixed.
