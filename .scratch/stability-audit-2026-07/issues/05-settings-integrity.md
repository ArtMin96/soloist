# PRD-05 — No decorative settings: every control either works or doesn't exist

Status: ready-for-agent
Blocked by: none

- **Severity:** P1 (multiple user-visible controls persist but change nothing — erodes trust in
  the whole settings surface)
- **Area:** `crates/app/ui/.../settings` + `sidebar`, `crates/core/src/facade.rs`,
  `crates/app/src/lib.rs`, `crates/core/src/ports/mod.rs` (Summarizer)
- **Evidence:** AGENT-reported; E5 (MCP/HTTP master) VERIFIED; others corroborated by the wiring
  trace. Several overlap PROGRESS.md's known I7g gap.

## Problem
A cluster of settings persist and display but have no consumer:
1. **MCP / HTTP master toggles** (E5, VERIFIED) — `set_integration_settings` persists
   `mcp_enabled`/`http_api_enabled` (`commands/settings.rs:140`) but `lib.rs` spawns the MCP IPC
   server (`:291`) and HTTP server (`:305`) **unconditionally**; `integration_settings` is never
   read outside its own command.
2. **Auto-summarization opt-in** (E4) — `AgentsPanel.tsx:69-96` offers a summarizer tool+model
   opt-in, but `Summarizer` is an **empty trait with no methods** (`ports/mod.rs:370`), not a
   `CorePorts` field, never invoked. No summarizer loop exists. (Mitigation: E6/summarization is a
   `later`/OFF row — but the UI reads as functional.)
3. **Sidebar settings** (E6, known I7g) — of ten fields only `hide_empty_sections` and
   `show_settings_footer` take effect. Persist-only, no consumer: `process_cpu_threshold`,
   `process_mem_threshold`, `project_cpu_threshold`, `project_mem_threshold` (`ProcessMeta.tsx`
   shows metrics whenever Running, ignoring thresholds), `show_filter_input` (no filter rendered),
   `project_open_in_editor` / `_in_terminal` / `_reveal_in_file_manager` (no such context actions).

Contract: CLAUDE.md §15 (no dead code / no controls that mislead); a persisted-but-ignored setting
is a false affordance.

## Fix approach — per control (owner decisions recorded 2026-07-13)
This PRD is a decision + cleanup pass. Given the expanded scope of the toggle work below, consider
splitting the MCP/HTTP live-toggle into its own session.
- **MCP/HTTP master toggles — owner chose LIVE teardown (not startup-only).** Wire the servers so a
  runtime toggle takes effect immediately, no app restart:
  - Gate the initial spawns in `build_facade`/`lib.rs:291`,`:305` on the persisted
    `integration_settings` so a disabled server never starts at boot.
  - Keep a **cancellation handle** (e.g. `CancellationToken` + `JoinHandle`) for each server task
    in the composition root. On `set_integration_settings`, if a server was enabled→disabled,
    cancel its accept loop, close the socket/port, and drain in-flight connections gracefully; if
    disabled→enabled, (re)spawn it. This is more surface than startup gating (live socket/port
    lifecycle + in-flight handling), so budget for it and test the teardown/respawn paths.
  - Keep the servers as adapters chosen in the one composition root — the toggle command routes to
    a core setting; the composition root owns the handles (no server lifecycle logic in core).
- **Summarizer opt-in — owner chose HIDE/DISABLE.** Remove the summarizer tool+model controls (or
  render them clearly disabled / "coming soon") so the UI stops implying a working feature. Do
  **not** build the summarizer loop (E6 is `later`/OFF, and core must never hard-depend on an LLM).
- **Sidebar thresholds/filter/context-actions:** for each, either implement the small behavior
  (thresholds → only show metrics above threshold in `ProcessMeta`; filter input → a name filter;
  project context actions → `open_in_editor`/terminal/file-manager via the shell-open capability)
  **or** remove the control. Thresholds + filter are cheap to implement and genuinely useful;
  the three "open in external app" actions need a Tauri shell/opener capability — check the
  capability set (currently minimal) and either add a scoped opener permission or drop the actions.

## Test plan
- **MCP/HTTP (startup gate):** with `mcp_enabled=false`, the IPC socket is not created on startup
  (integration test on `build_facade` / a headless boot); same for HTTP `http_api_enabled=false` →
  port not bound. Closes an audit test-gap.
- **MCP/HTTP (live teardown):** with a server running, toggle it off → the socket/port is gone and
  refuses new connections (and in-flight ones drain, not abort mid-frame); toggle back on → it
  serves again. Integration test over a real socket/ephemeral port, no restart.
- **Sidebar (UI):** with a threshold set, `ProcessMeta` hides metrics below it and shows above
  (component test); the filter input filters the list; each implemented control has a behavior
  test. For any control **removed**, assert it's gone from the settings read model.
- **Summarizer:** the opt-in is either absent or rendered disabled (component test).

## Acceptance
- Every remaining settings control has an observable effect proven by a test. Nothing persisted is
  ignored. `just test` + `just lint` green. PROGRESS.md I7g gap closed or explicitly re-scoped.

## Out of scope
Building the summarization subsystem (E6, `later`) — the UI is only hidden/disabled here.
