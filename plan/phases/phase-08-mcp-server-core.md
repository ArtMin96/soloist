# Phase 8 — MCP Server Core (C8 integration)

**Goal:** The integration surface that makes Soloist a metaharness: a standalone `soloist-mcp` (stdio)
binary letting external agents **see and act on** the stack — list/control processes, read rendered+raw
output, discover ports/readiness, spawn agents/terminals — all scoped to an **effective project** with
proper **identity** (ref §7).

**Delivers:** F1–F13. **Architecture:** the MCP adapter over the core via IPC (`04` §1); identity/scope
in C8. Coordination tools (scratchpads/todos/timers/locks/kv) are **Phase 9**.

## Scope
**In:** `soloist-mcp` binary; stdio transport; the IPC API (shared `crates/ipc`); effective-project
scope; identity (`bind_session_process`/`register_agent`/`whoami`, `SOLOIST_PROCESS_ID`); the **core**
tool groups — Project, Services, Process, Bulk, Output, Agent/Terminal, Setup/Support. **Out:**
Coordination/feature tools (Phase 9); HTTP API (Phase 10); setup-snippet generation polish (later).

## Architecture (ref §7, D4)
```
agent ──launches──► soloist-mcp ──IPC(UDS)──► running soloist app (core C8)
            (stdio MCP)             request/reply + subscribe
```
`soloist-mcp` holds **no state** — a thin MCP front over the app's IPC API. App not running → tools
return a clear "Soloist not running" error (optionally read last-known state from the runtime file).

## Tasks
1. **IPC API (`crates/ipc`):** versioned serde request/reply for everything the tools need (snapshot,
   logs slice, ports, metrics, start/stop/restart, spawn, identity). Server side in the app; client in
   `soloist-mcp` (and Phase 10's CLI/HTTP reuse the core, not this client).
2. **MCP server (`rmcp`, stdio):** register tools with **clean-room JSON Schemas** + LLM-facing
   descriptions; handshake/list_tools.
3. **Identity & scope (F3/F4, ref §7):** inject `SOLOIST_PROCESS_ID` into managed processes (Phase 3/4
   env); `bind_session_process` ties the MCP session to a `ProcessId`; `register_agent` for external
   callers; `whoami` returns resolved process/actor/effective-project; `select_project` sets scope.
4. **Project tools (F5):** `list_projects`, `select_project`, `get_project_status`, `get_project_stats`.
5. **Process tools (F6/F7):** `list_processes`, `get_process_status`, `start_process`, `stop_process`,
   `restart_process`, `rename_process`, `select_process`, `send_input` (text + raw control bytes;
   optional `wait_ms` returns the rendered tail), `close_process`.
6. **Bulk tools (F8):** `start_all_commands`, `stop_all_commands`, `restart_all_commands` — trusted
   commands in scope only.
7. **Output tools (F9):** `get_process_output` (rendered), `get_process_raw_output` (raw),
   `search_output`, `search_raw_output`, `clear_output` (buffer only, not the PTY), `flush_terminal_perf`,
   `get_process_ports`.
8. **Services tools (F10):** `services_list`, `wait_for_bound_port`.
9. **Agent/terminal tools (F11):** `spawn_process` (create+start a terminal/agent in scope),
   `spawn_agent` (alias), `list_agent_tools`.
10. **Setup/support (F12):** `help`, `submit_solo_feedback` (local no-op/stored), `setup_agent_integration`
    (write Soloist MCP docs into `AGENTS.md`/`CLAUDE.md`).
11. **Safety (F13, `04` §12):** read tools open; **action** tools honor the **trust gate** + **effective
    project scope** — a tool can't touch another project or run an untrusted command.

## Acceptance criteria
- Launch the app + a stack; run `soloist-mcp` and exercise via the MCP Inspector / a scripted client:
  `list_processes` returns the live stack; `get_process_output`/`_raw` return rendered/raw; `get_process_ports`
  lists a dev server's port; `get_project_stats` returns CPU/mem; `whoami` resolves the bound process.
- `send_input` with `wait_ms` delivers input and returns the rendered tail.
- `restart_process` on a crashed service recovers it (agent-driven recovery — the headline).
- App **not** running → clear non-crashing error.
- Action tools refuse an **untrusted** command and refuse cross-project targets.

## Test plan
- **Integration (CI, headless):** app with a fixture stack; scripted MCP client over stdio asserts each
  tool's contract; action tools mutate real state (observed via the app event stream); scope/trust
  enforcement asserted.
- **Manual:** register with a real Claude Code session; "check why the worker is down and restart it" →
  it uses `get_process_output` + `restart_process`.

## Risks & mitigations
- **MCP SDK/spec drift** → official `rmcp`, pinned; verify against current MCP docs (not memory);
  JSON-RPC fallback path.
- **Param schemas are ours (ref §7 gap)** → document each tool's schema; keep names Solo-compatible.
- **App-not-running UX** → graceful errors + optional last-known-state read.

## Effort
~5–7 days.
