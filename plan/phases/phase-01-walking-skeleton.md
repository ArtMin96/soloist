# Phase 1 — Walking Skeleton & Architecture

**Goal:** Make the architecture from [`04`](../04-engineering-architecture-and-patterns.md) **real** as
the thinnest possible end-to-end thread, *before* any feature. Define the core **ports** (traits), the
**event bus**, the **facade** (C8), the **mock `Clock`**, and a `Store` skeleton — then spawn **one**
process from a Tauri button, through the core, through a `ProcessSpawner`, and stream a status event
back to the UI. This is the structural spine every later phase plugs into.

**Delivers:** the architectural substrate for everything; parity K7 (dependency-direction). **Why a
phase:** building the ports/adapters skeleton first is what prevents the "big mutex + business logic in
the UI" rot the brief warns against.

## Scope
**In:** port traits (`ProcessSpawner`,`Clock`,`FileWatcher`,`Notifier`,`Store`,`Summarizer`,
`EventSink`); `DomainEvent` bus; `facade` command/query entry; mock + real adapters for `ProcessSpawner`
and `Clock`; SQLite `Store` skeleton (connection, WAL, one migration, one repo); the actor-task pattern
for a single process; the Tauri adapter wiring (`invoke`→facade, events→`emit`). **Out:** real lifecycle
rules, config, PTY, UI beyond a debug panel (later phases).

## Tasks
1. **Define ports** (traits) in `core` per `04` §1, with doc comments stating contract + invariants.
2. **Domain types** seed (`04` §4): `ProjectId`,`ProcessId`,`ProcStatus`,`ProcessKind`,`DomainEvent`.
   Newtype IDs; closed enums.
3. **Event bus** (`tokio::sync::broadcast`) + a `subscribe()`; define the snapshot-then-deltas contract.
4. **Facade (C8):** `spawn_demo_process()`, `stop(id)`, `snapshot()` — the only surface adapters call.
5. **Process actor (`04` §5):** one supervised task owning a (fake) child handle + cancellation token;
   emits `ProcessStatusChanged`. Prove start→running→stopped transitions and guaranteed cleanup on
   cancel.
6. **Adapters:** real `ProcessSpawner` (spawn `sleep`/`bash -c` via `tokio::process`, into a process
   group) **and** an in-memory fake; real `tokio` `Clock` **and** a mock clock that advances manually.
7. **SQLite `Store` skeleton (`store` crate):** open DB in the data dir (`SOLOIST_APP_DATA_DIR` or XDG),
   WAL, run a migration creating a `meta` table; one `MetaRepo` read/write to prove the port + adapter.
8. **Tauri adapter:** `invoke('spawn_demo')`/`invoke('stop')`; subscribe to the bus and `emit` events; a
   minimal debug panel listing demo processes + a Start/Stop button.
9. **Panic isolation harness (`04` §6):** wrap the actor in a supervisor that catches a panic, marks the
   unit `Error`, and keeps the app alive (test by making the fake child panic).
10. **Wire the dependency-direction guard** to fail if `core` imports any adapter crate (cements
    `04` §1).

## Interfaces introduced
```rust
trait ProcessSpawner { async fn spawn(&self, spec:&SpawnSpec)->Result<Child>; /* write,resize,kill */ }
trait Clock { fn now(&self)->Instant; async fn sleep(&self,d:Duration); fn interval(&self,d:Duration)->Interval; }
trait Store { /* repo accessors */ }
struct Facade { /* holds contexts + ports */ }
impl Facade { async fn spawn_demo_process(&self)->Result<ProcessId>; async fn stop(&self,ProcessId)->Result<()>; fn snapshot(&self)->Vec<ProcessView>; fn subscribe(&self)->Receiver<DomainEvent>; }
```

## Acceptance criteria
- Clicking "Start" in the debug panel spawns a real `sleep 60` (verify PID + its process group); a
  `ProcessStatusChanged(Starting→Running)` event reaches the UI; "Stop" cancels it and the **process
  group is gone** (PID check).
- The same flow runs in a **headless core test** using the fake spawner + **mock clock** with no real
  time elapsed.
- A deliberately panicking fake child marks that process `Error` without crashing the app.
- SQLite DB is created with WAL + the migration; `MetaRepo` round-trips a value.
- Dependency-direction guard is green; `core` has zero adapter imports.

## Test plan
- **Unit (headless, deterministic):** actor transitions + cleanup via fake spawner + mock clock;
  panic isolation; `MetaRepo` round-trip on an in-memory SQLite.
- **Integration:** real spawner spawns/kills `sleep`; assert pgroup reaped.
- **e2e (smoke):** Playwright clicks Start/Stop, asserts the row appears/updates.

## Risks & mitigations
- **Over-abstracting ports** → keep traits minimal; add methods only when a phase needs them.
- **Tauri event/IPC ergonomics** → settle the `api.ts` event contract here so later phases reuse it.

## Effort
~4–5 days. High leverage: every later phase is cheaper because this exists.
