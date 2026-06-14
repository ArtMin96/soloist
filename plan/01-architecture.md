# 01 — System Architecture (structure & topology)

This is the **concrete system layout**: binaries, crates, modules, and runtime data flows. For the
*why* (patterns, domain boundaries, longevity rules) see
[`04-engineering-architecture-and-patterns.md`](04-engineering-architecture-and-patterns.md); for the
*behavior* see [`05-solo-reference-and-sources.md`](05-solo-reference-and-sources.md).

## Binaries (mirrors Solo's `Solo` / `solo-cli` / `mcp`)

| Binary | Crate | Role |
|--------|-------|------|
| `soloist` | `crates/app` (Tauri) | desktop app: window + WebKit UI; **hosts the domain core**; serves the loopback HTTP API |
| `soloist-mcp` | `crates/mcp` | standalone MCP server (stdio); thin adapter to the core over local IPC |
| `soloist` (CLI) | `crates/cli` | terminal control; **thin HTTP client** of the loopback API (ref §8) |

## Runtime topology

```
agent (Claude Code) ──launch as Agent process──┐
                                                │ stdio MCP
external agent ──► soloist-mcp ──IPC(UDS)──►    │
                                                ▼
 ┌──────────────────────── soloist (Tauri app) ────────────────────────┐
 │  WebKitGTK UI  ◄──Tauri invoke/emit──►  DOMAIN CORE (crates/core)     │
 │                                          C1 Projects&Config           │
 │  loopback HTTP API 127.0.0.1:24678 ────► C2 Process Supervision       │
 │  (soloist CLL + Raycast-style clients)   C3 Terminal I/O              │
 │                                          C4 Agents & Idle             │
 │                                          C5 Monitoring                │
 │                                          C6 Coordination ──► SQLite   │
 │                                          C7 Notifications ──► libnotify│
 │                                          C8 Integration façade        │
 └───────────┬───────────────┬───────────────┬──────────────────────────┘
             ▼ PTY+pgroup     ▼ PTY+pgroup    ▼ PTY+pgroup
        dev server        queue worker     CLI agent
```

All three adapters (UI, MCP, HTTP/CLI) drive the **same core commands** — one behavior, many fronts.

## Crate layout

```
crates/
  core/    # C1–C8 + ports (traits) + domain types + event bus. NO tauri/mcp/axum/sqlite imports.
  store/   # SQLite impl of the Store port (repos + migrations)
  pty/     # ProcessSpawner impl (portable-pty + nix)
  app/     # Tauri binary + command/event adapter + HTTP API + bundled UI (ui/)
  mcp/     # soloist-mcp binary (stdio MCP adapter)
  httpapi/ # loopback HTTP API adapter (used by app)
  cli/     # soloist CLI (HTTP client)
  ipc/     # app<->mcp UDS transport + request/response message types shared by app/mcp/httpapi/cli
```

The dependency rule (adapters → core, never the reverse) is CI-enforced (ref `04` §10).

## Domain modules in `crates/core` (the bounded contexts)

| Ctx | Module | Responsibility | Ports used |
|-----|--------|----------------|-----------|
| C1 | `config` + `projects` + `trust` | `solo.yml` parse/validate/**sync(hash+debounce)**; auto-detection; project registry; trust store | Store, FileWatcher |
| C2 | `supervisor` | process registry; `ProcStatus` FSM; start/stop/restart; restart policy; orphan adoption | ProcessSpawner, Clock |
| C3 | `terminal` | PTY read loops; **rendered + raw** buffers; input; resize; OSC parse | ProcessSpawner |
| C4 | `agents` + `idle` | agent tool defs; launch; **5-state idle FSM**; optional summarization | Summarizer, Clock |
| C5 | `metrics` + `ports` | CPU/mem sampling; `/proc` port discovery; readiness | Clock |
| C6 | `coordination` (`scratchpads`,`todos`,`timers`,`locks`,`kv`) | durable coordination aggregates | Store, Clock |
| C7 | `notify` | crash/attention/idle toasts; unread/attention-bell | Notifier, EventSink |
| C8 | `facade` + `identity` | public command/query API; effective-project scope; `SOLOIST_PROCESS_ID` | — |
| — | `events` | typed `DomainEvent` bus (`broadcast`) | — |

## State model (authoritative, in the core)

```rust
Project { id, name, root, icon, config: SoloYml, editor, exec_profile }
Process {
  id, project, name, kind: Command|Agent|Terminal,
  spec: ProcessSpec,                 // command, working_dir, env, auto_start, auto_restart, watch
  status: ProcStatus, pid, pgid, exit_code, restart_window: Vec<Instant>,
  trust: Untrusted | Trusted{variant_hash},
  visibility: Shared | Local,        // YAML vs app-state
  activity: Option<AgentActivity>,   // agents only
  metrics: { cpu_pct, rss, ports }, log: Ring<LogLine>, raw: ByteScrollback,
}
// durable (SQLite): Trust, Project registry, Settings, AgentTool defs,
//                   Scratchpad, Todo(+comments,blockers,locks), Lease, KvEntry
```
Ephemeral vs durable split is defined in `04` §7. The frontend renders **projections**, never owns
authoritative state.

## Data-flow walkthroughs

- **Start stack:** adapter → `facade.stack_start(project)` → C2 starts each **trusted, auto_start**
  command in its own PTY+pgroup (C3) → `ProcessStatusChanged` + `LogLine`/`PtyOutput` events → all
  adapters update. C5 begins sampling.
- **Crash → restart:** child exits non-zero → C2 `Crashed` → C7 toast → restart policy checks the
  **10-in-60s** window → respawn (`Restarting→Running`) or `RestartExhausted`.
- **File-watch restart:** C1 watcher matches a changed path to a command's `restart_when_changed` →
  debounced → `C2.restart(id)` (trusted-only).
- **Agent idle → timer fires:** C4 idle FSM flips an agent to `Idle` → C6 `timer_fire_when_idle_all`
  whose `waiting_on` includes it resolves → delivers `body` to the owning agent as a fresh user turn.
- **MCP get_logs:** external agent → `soloist-mcp` → IPC → C3 buffer slice → back to the agent.
- **HTTP restart:** `POST /processes/:id/restart` (`X-Soloist-Local-Auth: 1`) → `facade.restart` → same
  core path as the UI button.
- **Scratchpad write:** MCP `scratchpad_write(expected_revision)` → C6 checks revision in SQLite →
  commit or `RevisionConflict` (optimistic concurrency, ref `04` §7).

## Concurrency (summary; full rules in `04` §5/§6/§8)

One `tokio` runtime; each process is a supervised **actor task** owning its child/PTY/stdin/exit-watch;
contexts talk via messages + the event bus; bounded channels + backpressure; cancellation tokens;
panic-isolated tasks; deterministic shutdown that reaps every process group.
