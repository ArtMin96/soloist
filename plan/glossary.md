# Glossary

Shared vocabulary so every doc means the same thing by the same word. Domain terms first, then
engineering terms.

## Product / domain

- **Solo** — the original closed-source macOS app (`soloterm.com`). The thing we clone.
- **Soloist** — this project: the open Linux clone. Working name.
- **Faithful clone** — reproduces Solo's documented behavior (the parity matrix), not its source,
  assets, or exact pixels. Clean-room.
- **Metaharness** — Solo's self-description: the workspace *around* agents (terminals, logs, todos,
  scratchpads, locks, timers, MCP), not an agent itself. "The coding harness for your coding harness."
- **`solo.yml`** — repo-committable project config: `name`, `icon`, and a `processes` **map** keyed by
  name (`command`, `working_dir`, `auto_start`, `auto_restart`, `restart_when_changed`, `env`). Ref §3.
- **Process** — anything Soloist supervises. Three **subtypes**:
  - **Command** — a managed shell command (dev server, worker); trust-gated; auto-start/-restart; watch.
  - **Agent** — an AI CLI in an interactive terminal; has idle/activity state.
  - **Terminal** — a plain interactive shell.
- **Agent (tool)** — a CLI the user installed (Claude Code, Codex, Gemini, Amp, OpenCode, Aider,
  Goose…); configured in Settings; we run it, never ship/replace it.
- **Project** — a filesystem folder (repo root) that sets the working dir, watches/syncs `solo.yml`,
  and auto-detects commands.
- **Trust gate** — repo-sourced commands are **Untrusted** until the user confirms; untrusted blocks
  manual start, auto-start, restart, file-watch and crash auto-restart. Trust is local, scoped to
  (project, command-variant hash). Ref §4.
- **Sync** — when `solo.yml` changes, Soloist debounces FS events, hash-diffs, and adds/updates/removes
  commands (preserving renames); changes to command/dir/env re-require trust. Sync never auto-starts.
- **Command auto-detection** — on first add (no `solo.yml`), suggest commands from repo files
  (package.json, Procfile, Makefile, Docker Compose, framework markers…). Ref §9.
- **Visibility — Shared vs Local** — Shared commands live in `solo.yml` (committed); Local commands
  live only in app state (private), never written to YAML.
- **PTY** — pseudo-terminal making a child believe it has a real terminal (ANSI, interactive input).
- **Rendered vs raw output** — *rendered* = the terminal-screen text; *raw* = the byte stream incl.
  control/escape sequences. Solo exposes both via MCP; we keep both buffers.
- **Process group / pgid** — OS grouping we spawn each process into so we signal the whole tree
  (SIGTERM→grace→SIGKILL) — clean shutdown, no orphans.
- **Orphan adoption** — on relaunch after a crash/force-quit, match leftover processes (project + name
  + command) and adopt, else prompt Kill/Leave. Ref §4.
- **Idle detection** — per-agent activity FSM with 5 states **IDLE / PERMISSION / THINKING / WORKING /
  ERROR**, from per-provider heuristics (visible output, OSC title). Drives timers + notifications.
- **Auto-summarization** — optional compact natural-language summary of an agent's recent output via a
  headless model; in Soloist it's optional/degradable, never required.
- **MCP (Model Context Protocol)** — how external agents query/act on Soloist. `soloist-mcp` (stdio)
  exposes ~40+ tools across Project/Services/Process/Bulk/Output/Agent-Terminal/Coordination/Setup +
  feature groups (scratchpads/todos/timers/key-value).
- **Effective project scope** — the project an MCP call acts on, set by `select_project` or inferred
  from the session's bound process.
- **`SOLOIST_PROCESS_ID`** — env var injected into managed processes; lets an agent's MCP session
  **bind** (`bind_session_process`) so timers/locks/todo-locks/cleanup attach to the right process.
  External callers use `register_agent`; `whoami` reports the resolved identity/scope.
- **Scratchpad** — project-scoped Markdown note for long-lived context; **revision-guarded** writes
  prevent clobbering concurrent edits.
- **Todo** — project-scoped task with tags, **blockers** (dependencies), **locks**, comments, transfer.
- **Lease / lock** — cooperative, short-lived, project-scoped coordination signal (with TTL + owner),
  auto-released when the owning process closes. "Signals, not ownership."
- **Timer / fire-when-idle** — schedule a future "user turn" for an agent; `fire_when_idle_any/all`
  wakes a lead agent when watched children go idle — token-free waiting instead of polling.
- **Key-value** — project-scoped small JSON state for coordination (not logs/long text).
- **Execution profile** — a project-level environment definition (which shell/runtime a command/agent
  executes in). On Linux: shells (zsh/bash/fish) as interactive login shells.
- **Attention bell / unread** — a title-bar bell + unified unread state (sidebar/title/dock) when a
  process needs attention.
- **Loopback HTTP API** — `127.0.0.1:24678`; read endpoints + mutations (auth header); the `soloist`
  CLI is a thin client of it.

## Engineering

- **Ports & Adapters (hexagonal)** — the pure domain core defines **ports** (traits); Tauri/MCP/HTTP/
  SQLite/PTY/OS are **adapters**. Core depends on nothing app-specific.
- **Bounded context** — a domain module with one responsibility and a published interface (C1–C8).
- **Actor / supervision tree** — each process is one task that solely owns its resources; tasks are
  supervised and panic-isolated so one failure can't sink the app.
- **FSM (finite state machine)** — explicit legal transitions for `ProcStatus`, `AgentActivity`,
  `Trust`; illegal transitions rejected.
- **CQRS-lite** — commands mutate via the owning context; queries read cheap projections.
- **Optimistic concurrency** — `expected_revision` on scratchpad/todo writes (see Scratchpad).
- **Backpressure** — bounded channels + coalescing so a chatty process can't OOM the app.
- **Walking skeleton** — Phase 1's end-to-end thread (spawn one process through every layer) that
  proves the architecture before features.
- **Soak / longevity test** — multi-hour run asserting flat RSS / FD / task counts and zero leaked
  PIDs (Phase 13 gate).
- **Parity matrix** — `02-feature-parity-matrix.md`; the checklist that defines "done."
- **Reference (§N)** — a section in `05-solo-reference-and-sources.md` (e.g. "ref §7" = MCP).
- **Driving vs driven adapter** — a *driving* adapter calls into the core (Tauri UI, MCP, HTTP, CLI); a
  *driven* adapter is called by the core through a port (PTY spawner, SQLite store, clock, watcher). Both
  depend on `core`; `core` depends on neither. Ref `06` §1.
- **Out-of-process vs in-process adapter** — out-of-process driving adapters are separate binaries
  (`soloist-mcp`, the `soloist` CLI) the app never links, so removing one is dropping a binary; in-process
  adapters (the HTTP API, the Tauri command surface) are linked into the `app` binary, so making one
  optional requires a Cargo feature or runtime toggle. Ref `06` §8.
- **Composition root** — the single place a binary picks real-vs-`Noop` adapters and assembles the
  `Facade` (`crates/app/src/lib.rs::build_facade`). Exactly one per binary; tests are alternate roots built
  from `core::testing` fakes. Ref `06` §8.
- **Null-Object port** — an optional driven subsystem ships a `Noop*` default impl of its port (e.g.
  `NoopLockReleaser`, `NoopRuntimeState`) so the core always holds *a* port and never branches on "is it
  wired?". The mechanism behind graceful degradation. Ref `04` §8, `06` §4.
- **`CorePorts`** — (planned, R3) the parameter-object struct holding the set of `Arc<dyn Port>` the core
  needs, passed to `Facade::new` so adding a port is one field, not another constructor argument. Ref `06` §7.
- **Placeholder module** — an empty `pub mod foo;` permitted *only* when `foo` maps to a documented future
  bounded context (`01`/`06` §3); a roadmap marker carrying a doc comment, not dead code. Ref `06` §3.
