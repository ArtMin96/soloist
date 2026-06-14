# 00 — Vision & Scope

## Why this exists

[Solo](https://soloterm.com/) is a macOS-only, closed-source desktop app (`com.soloterm.solo`,
v0.8.2, Tauri). Its download page lists Linux as "coming soon (Ubuntu 20.04+)" with no date and no
source to port. Soloist rebuilds Solo's functionality from its **public documentation and observable
behavior** (see [`05-solo-reference-and-sources.md`](05-solo-reference-and-sources.md)) so it runs
natively on Ubuntu now. Clean-room: our own code; no Solo source, assets, name, or branding.

## One-sentence definition

> Soloist is a native Linux **process-supervisor + agent-coordination workspace** that runs your dev
> stack and CLI coding agents from one dashboard, keeps them alive, and gives those agents a shared,
> project-scoped workspace (logs, status, todos, scratchpads, locks, timers) over MCP — all driven by a
> committable `solo.yml`.

## The mental model we must match — "metaharness"

Solo is **not** a coding agent, **not** a terminal emulator, **not** a worktree orchestrator. It runs
the agent CLIs you already use as ordinary **processes** and layers a shared coordination surface on
top. Three process subtypes (ref §2):

- **Command** — a managed shell command (dev server, worker). Trust-gated; auto-start; auto-restart;
  file-watch restart.
- **Agent** — an AI CLI (Claude Code, Codex, Gemini, Amp, OpenCode, Aider, Goose…) in an interactive
  terminal, with **idle detection** (5 states).
- **Terminal** — a plain interactive shell.

The coordination layer (what makes it a *metaharness*) lets a lead agent spawn workers, hand them
todos, take **locks**, share **key-value**/**scratchpad** state, and **wait on a timer until children
go idle** — token-free orchestration, all visible in the same UI. It does this **without** worktrees,
sandboxes, or branch isolation — agents are siblings in one shared workspace.

## In scope (faithful-parity targets)

Tracked precisely in [`02-feature-parity-matrix.md`](02-feature-parity-matrix.md). Summary by area:

- **Projects & config** — `solo.yml` (real schema: `name`/`icon`/`processes{}` with `command`,
  `working_dir`, `auto_start`, `auto_restart`, `restart_when_changed`, `env`); 1 MB limit; **trust
  gate**; **sync** via hash-diff + debounce; **command auto-detection** from repo files; multiple
  projects; local (app-state) vs shared (YAML) commands.
- **Process supervision** — three subtypes; status FSM; start/stop/restart (one + all); graceful
  stop (SIGTERM→grace→SIGKILL on the process group); **orphan adoption** on relaunch.
- **Terminal I/O** — real PTYs; **rendered + raw** output buffers; interactive input (text + control
  bytes); resize; full ANSI; OSC parsing (titles/bells).
- **Monitoring & self-healing** — per-process CPU/mem; port discovery + readiness; crash auto-restart
  **rate-limited 10/60s**; file-watch restart (debounced, trusted-only); native + in-app notifications;
  **attention bell** + unified unread.
- **Agents** — agent tool config (Claude/Codex/Amp/Gemini/OpenCode/Generic, `--version` autodetect);
  launch picker + "agent with flags"; **5-state idle detection**; **optional** auto-summarization.
- **MCP server** — separate stdio binary; **effective project scope**; identity via `SOLOIST_PROCESS_ID`
  + `bind_session_process`/`register_agent`/`whoami`; ~40+ tools across Project/Services/Process/Bulk/
  Output/Agent-Terminal/Coordination/Setup groups.
- **Coordination** — scratchpads (revision-guarded), todos (tags/blockers/locks/comments/transfer),
  timers (incl. fire-when-idle), lease locks, key-value — persisted in SQLite, exposed via MCP.
- **Local HTTP API + CLI** — loopback API (`127.0.0.1:24678`, auth header for mutations) and a
  `soloist` CLI that drives it.
- **UX** — sidebar process tree (Agents/Terminals/Commands); command palette + jump palette;
  `soloist://` deep links; light/dark/system themes; keyboard-first nav; settings; **execution
  profiles**; open-in-editor.
- **Packaging** — `.deb` + `.AppImage` for **Ubuntu 20.04+, x86_64** (D2); desktop entry, icon,
  notifications; in-app update check.

## Out of scope (YAGNI / not building)

- **Licensing, payments, Free/Pro tiers, license server, analytics** (D3).
- **macOS / Windows / arm64 builds** (D2). Tauri keeps these possible later.
- **Raycast extension**; Solo's hosted update manifest / account system.
- **Git worktrees, sandboxes, container isolation** — Solo doesn't do these either; out of scope by
  design.
- **A required cloud summarizer.** Auto-summarization is optional and uses the user's own agent CLI
  headless, or is disabled — never a hard dependency.
- **Pixel-identical UI** and any reuse of Solo's name/logo/assets.

## Success criteria — definition of "faithful, v1"

Every `v1` row of the parity matrix passes on a clean **Ubuntu 22.04, x86_64** packaged install,
specifically:

1. Write a `solo.yml`, launch Soloist, **trust** the commands, start a multi-process stack (incl. a
   real agent like Claude Code) with one action.
2. Crashing a process → red status + auto-restart (respecting the 10/60s cap) + a desktop notification.
3. Editing a watched file restarts the right command (debounced, trusted-only).
4. Open a terminal on any process and interact (answer an agent prompt) with full ANSI.
5. An external agent over MCP can list processes, read a process's logs, discover its ports, and
   restart a crashed service; a lead agent can spawn a worker, give it a todo, take a lock, and set a
   **fire-when-idle** timer.
6. A `soloist` CLI command and an HTTP `POST /processes/:id/restart` both work.
7. Installs from `.deb` and `.AppImage`; passes the **longevity gate** (Phase 13): a multi-hour soak
   with random crashes shows flat RSS/FD/task counts and zero leaked PIDs.

## Non-goals affecting design

- Parity first; extensions later. Single local user. Agents are external CLIs we run, not ship.
- We do **not** silently rewrite the user's `solo.yml`; local additions live in app state.
