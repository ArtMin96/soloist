# Soloist — Architecture Overview

> **Read-first digest.** This is the scannable map of how Soloist is built. It is an *overview*, not the
> source of truth: the authoritative docs are **`plan/04-engineering-architecture-and-patterns.md`** (the
> design *principles*) and **`plan/06-codebase-blueprint-and-cleanup.md`** (the concrete blueprint, the
> add-a-X recipes, and the cleanup roadmap). If this file ever disagrees with those, they win and this
> file is corrected. Process rules live in `CLAUDE.md` (§16 = the structural invariants).

Soloist is a native-Linux **process-supervisor + agent-coordination workspace** (a clean-room rebuild of
macOS Solo). Stack: **Tauri v2 + Rust core + React/TS + xterm.js**. Style: **Hexagonal (Ports & Adapters)**
— a pure, framework-free domain core with 8 bounded contexts, actor-model supervision, event-driven +
CQRS-lite, SQLite for durable state.

---

## 1. Architecture — Hexagonal, two dependency directions

```
  DRIVING ADAPTERS  (call the core)                        DRIVEN ADAPTERS  (the core calls)
  ┌───────────────────────────────────┐                   ┌────────────────────────────────────┐
  │  Tauri UI ── crates/app            │                   │  ProcessSpawner / PtyIo ── crates/pty│
  │  MCP      ── crates/mcp     [→P8]  │                   │  Store / repos          ── crates/store│
  │  HTTP     ── crates/httpapi [→P10] │                   │  Clock                  ── core (tokio)│
  │  CLI      ── crates/cli     [→P10] │                   │  FileWatcher/Notifier/Summarizer [→Pn]│
  └─────────────────┬─────────────────┘                   └───────────────────┬────────────────┘
                    │ one Facade call                                          ▲ trait (port)
                    ▼                                                          │
            ╔═══════════════════════════════════════════════════════════════════════╗
            ║                    crates/core  —  PURE DOMAIN                          ║
            ║   C1…C8 bounded contexts · ports (traits) · DomainEvent bus · Facade    ║
            ║   imports NO tauri / rmcp / axum / rusqlite / notify-rust  (CI-enforced)║
            ╚═══════════════════════════════════════════════════════════════════════╝
```

**The one rule:** everything points *at* `core`; `core` points at nothing app-specific.
`scripts/check-core-deps.sh` (parity **K7**) enforces it. That single rule is what makes the whole app
headless-testable and is the mechanical guarantee behind *"remove MCP → app still builds & runs."*

### Crate roles

| Crate | Kind | Owns | May depend on | Status |
|-------|------|------|---------------|--------|
| `core` | domain | C1–C8, ports (traits), domain types, event bus, `Facade` | `tokio`/`serde`/`thiserror`/`vte` — **never** an adapter crate | live (C1–C3, C8) |
| `store` | driven adapter | SQLite: `Store`/`ProjectRepo`/`TrustRepo`/`RuntimeState` + migrations | `core`, `rusqlite` | live |
| `pty` | driven adapter | `ProcessSpawner`/`PtyIo`/`ProcessControl`/`OrphanControl` over `portable-pty`+`nix` | `core`, `portable-pty`, `nix` | live |
| `app` | driving + host | Tauri shell, command/event wiring, **the composition root**, bundled UI | `core`, `store`, `pty`, `httpapi`, `tauri` | live |
| `mcp` | driving adapter | `soloist-mcp` stdio binary → core over `ipc` | `core`, `ipc`, `rmcp` | stub → P8 |
| `httpapi` | driving adapter | loopback `127.0.0.1:24678` over `axum` | `core`, `ipc`, `axum` | stub → P10 |
| `cli` | driving adapter | `soloist` CLI = thin HTTP client | `ipc`, `clap` (not `core`) | stub → P10 |
| `ipc` | shared contract | app↔mcp UDS transport + request/reply types | `serde` only | stub → P8 |

A new external integration is a **new/existing adapter crate** — never logic added to `core` or to another
adapter. Adapters hold **no** business state and make **no** domain decisions.

---

## 2. Domain separation — 8 bounded contexts

```
  adapters ──► C8 Facade ──► C1…C7      (adapters touch C8 ONLY; no cycles between contexts)
```

| Ctx | Modules in `core` | Owns | Status |
|-----|-------------------|------|--------|
| **C1** Projects & Config | `config/` `projects` `trust` `hash` `debounce` | `solo.yml` load/validate/sync, project registry, trust gate, hashing, debounce | live (P2) |
| **C2** Process Supervision | `supervisor/` `process` `orphans` | registry, `ProcStatus` FSM, start/stop/restart, bulk ops, orphan reconcile | live (P3) |
| **C3** Terminal I/O | `terminal/` | PTY read loop, rendered+raw buffers, OSC parse, attach replay | live (P4) |
| **C4** Agents & Idle | `agents` `idle` | agent-tool defs, launch, 5-state idle FSM, optional summary | placeholder → P7 |
| **C5** Monitoring | `metrics` `portscan` | CPU/mem sampling, `/proc` port discovery, readiness | placeholder → P6 |
| **C6** Coordination | `coordination` | scratchpads, todos, timers, leases, key-value | placeholder → P9 |
| **C7** Notifications | `notify` | crash/attention/idle toasts, unread/bell state | placeholder → P6 |
| **C8** Integration façade | `facade` `identity` | the public command/query API; MCP identity & effective scope | live (`facade`) |

Cross-cutting in `core`: `events` (the `DomainEvent` bus), `ports` (every trait + its `Noop` default),
`ids` (newtype IDs), `sync` (poison-safe lock helper), `testing` (shared fakes).

**Isolation guarantees.** A bug in summarization (C4) cannot corrupt the process registry (C2);
coordination state (C6) persists in SQLite independently of live processes (C2), so todos/scratchpads
survive restarts while PTY buffers don't.

**Placeholder-module rule.** An empty `pub mod foo;` is allowed **only** when `foo` maps to a context above
(and `plan/01`'s module table); it carries a doc comment naming its context + phase and exports nothing
until then. That is a roadmap marker — **not** dead code. A leftover empty file with no mapping is forbidden.

### Coordination flow — create → delegate → use (C6, the metaharness)

Coordination (scratchpads, todos, timers, leases, key-value) is **durable, project-scoped SQLite state**
agents share to orchestrate each other token-free. All paths route through the **one `Facade`** (UI, MCP,
HTTP behave identically):

- **Create** — an agent calls a coordination MCP tool (`scratchpad_write`, `todo_create`, `lock_acquire`,
  `timer_set`, `kv_set`) → `mcp` handler → one `Facade` method → C6 aggregate → SQLite repo. Shared-record
  writes are **revision-guarded** (`expected_revision` → `RevisionConflict`); the call is scoped to the
  effective project and attributed to the **bound process** (`SOLOIST_PROCESS_ID`).
- **Delegate** — a lead agent `spawn_agent`s a worker (auto-binds), hands work via `todo_create` +
  `todo_transfer`/`_lock`/`_set_blockers`, signals intent with a **lease** (`lock_acquire`, TTL+owner), and
  **waits without polling** via `timer_fire_when_idle_all` — which delivers its `body` to the lead as a fresh
  turn when the watched workers go idle.
- **Use & release** — workers read/write the shared records; **process-owned locks auto-release when the
  owning process closes** (no stranded locks on crash); state **survives app restart** (SQLite), unlike live
  processes and PTY buffers.

Target design — **C6 is a placeholder until Phase 9**; tool *param schemas* are clean-room/undesigned (`plan/05`
§12). Full recipe: **`plan/06` §5.8**; data-flow walkthroughs: `plan/01`; tool catalog: `plan/05` §7.

### Frontend domain split (`crates/app/ui/src`)

| Path | Role | Rule |
|------|------|------|
| `domain.ts` | the **single** TS mirror of core enums / `DomainEvent` | one definition per type; mirrors serde output |
| `api.ts` | typed `invoke`/`listen` + Channel | **every** command/event name string lives here once |
| `store/` | read-model: pure reducers (`projection`, `grouping`) + hooks | reducers pure + unit-tested |
| `lib/` | presentational helpers (`status.ts` = the single `ProcStatus`→glyph/color/label map) | no IPC, no state |
| `components/` | small presentational components (`sidebar/`, `terminal/`) | props-in/callbacks-out; no logic, no `invoke` |

---

## 3. Design patterns — what, where, when

Reach for a pattern when its **trigger** fires — not preemptively (YAGNI).

| Pattern | Lives in | Reach for it when… |
|---------|----------|--------------------|
| **Ports & Adapters** | `ports.rs` + adapter crates | any OS/UI/transport/storage concern → trait in core, impl in an adapter |
| **Facade / Anti-corruption layer** | `core::facade::Facade` (C8) | an adapter needs to *act* → one `Facade` call, never a context internal |
| **Actor + supervision** | `supervisor/actor.rs` (one task per process) | a resource needs one owner racing events (child vs stop) → task + bounded `mpsc` |
| **Finite state machine** | `ProcStatus::transition` (+ future `AgentActivity`, `Trust`) | state has legal transitions → `Result<New, IllegalTransition>` |
| **Observer (event bus)** | `events::EventBus` (broadcast) | a change must reach N adapters → emit a `DomainEvent` |
| **CQRS-lite** | `Facade::snapshot` (query) vs `supervisor.start` (command) | reads must not block writes |
| **Repository** | `store` repos; future Todo/Scratchpad/Kv/Lock | a durable aggregate → one focused trait, SQLite behind it |
| **Newtype + closed enum** | `ids.rs`, `process.rs` | a domain id/state → never a bare `String`/`int` |
| **Null Object** | `Noop{LockReleaser,RuntimeState,OrphanControl}` | a **driven** subsystem is optional → ship a `Noop` so core runs without the real adapter |
| **Parameter Object / Builder** | *to add (R3)* — `CorePorts` for `Facade::new` | a constructor passes >4 collaborators (`too_many_arguments`) |
| **Registry** | *to add* — MCP tool registry (P8), agent-tool defs (P7) | a growing set of "one of many" handlers → register, don't extend a giant `match` |
| **Strategy** | *to add* — per-provider idle heuristics (P7), per-agent launch (P7) | behavior varies by a closed set of providers → one trait, one impl per provider |
| **Optimistic concurrency** | *to add* — scratchpad/todo `expected_revision` (P9) | concurrent writers to one durable record → revision guard |
| **Lease / lock** | *to add* — coordination (P9) | cooperative cross-agent intent → TTL + owner `ProcessId`, auto-release on close |

---

## 4. Adapter independence — "remove MCP and the app survives"

Two mechanisms, one per dependency direction:

- **Driving adapters (MCP, HTTP, CLI, UI) are independent crates** that depend only on `core`/`ipc`. To
  remove MCP entirely: drop `crates/mcp` from the workspace members + stop launching the sidecar in the
  composition root. Nothing else references it (the dependency-direction guard makes the reverse impossible
  by accident). The app degrades to "no MCP," not "broken."
- **Driven adapters (optional subsystems the core calls) use the Null Object pattern** — a port with a
  `Noop` default. The core always holds *a* `Arc<dyn Port>`; if the real adapter isn't wired, it holds the
  `Noop` and never branches on "is it present?". (This is why `build_facade` degrades runtime-state to
  `NoopRuntimeState` when the data dir is unavailable.)

**One composition root per binary.** `crates/app/src/lib.rs::build_facade` is the *only* place real-vs-`Noop`
adapters are chosen and the `Facade` is assembled. Tests are alternate composition roots built from
`core::testing` fakes.

---

## 5. Single source of truth

Every status / kind / event / command / limit / path is defined **once** — Rust enum in `core`, the TS
mirror in **one** `domain.ts`, one command/event-name constant per side. A numeric bound is a named `const`,
never a literal at the comparison site. The Rust↔TS pair is the *only* sanctioned duplication.

Shared **test fakes** (`FakeSpawner`, `MockClock`, `FakeTrustRepo`, …) live once in `core::testing` —
reused across crates via its `testing` feature (the cleanup fixes the current `#[cfg(test)]`-private gap).

---

## 6. Cleanup roadmap (R-phases — each ends `just lint && just test` green)

The tree is at build-Phase 5 (`Done — pending verify`). The cleanup is sequenced as small reviewable
commits that don't blindly regress verified-pending code. **Decisions locked:** tests stay **inline** (trim,
don't relocate); the empty core modules **and** the 4 stub crates **stay** as documented placeholders.

| R | Goal | Done when |
|---|------|-----------|
| **R0** | Blueprint + guardrails (docs only) | this file, `plan/06`, `CLAUDE.md` §16 merged; `scripts/check-file-size.sh` warns in `just lint`/CI |
| **R1** | Reusable test fakes | `core::testing` behind a `testing` feature, reused by `store`/`pty`; no fake defined twice |
| **R2** | Split the god-file | no `supervisor` source file over the ~400 non-test-line smell; public surface unchanged |
| **R3** | Composition root + ports param object | `CorePorts` struct; both `#[allow(too_many_arguments)]` removed |
| **R4** | Purge core scaffolding | `core` carries no demo / `std::env` scaffolding (demo lives only in `app`) |
| **R5** | Honest-test audit | every test exercises real behavior; tautological/pretend tests deleted |
| **R6** | Converge docs & ledger | plan-doc drift fixed; `PROGRESS.md` + `KNOWN-DIVERGENCES.md` current |

Full detail and the add-a-X recipes: **`plan/06-codebase-blueprint-and-cleanup.md`**.

---

## 7. The one-paragraph contract

Behavior lives in a **bounded context** in `crates/core`, behind **ports**, exposed through the **one
`Facade`**. OS/UI/transport/storage are **adapter crates** that depend only on `core` and route to that
`Facade` — never the reverse (CI-enforced). Optional subsystems are **ports with `Noop` defaults**, wired in
the **single composition root** (`app::build_facade`), so any one can be absent without breaking the app.
Every concept is defined **once**; shared test fakes live once in `core::testing`. Reach for a design
pattern when its **trigger** fires, not before. Files stay **small and single-purpose**; tests stay
**inline but honest**. When in doubt, follow the recipe in `plan/06` §5; when a recipe and a higher doc
disagree, the higher doc wins.
