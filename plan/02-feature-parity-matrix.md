# 02 — Feature Parity Matrix

"Faithful" made measurable. Every Solo capability (from the cited research in
[`05-solo-reference-and-sources.md`](05-solo-reference-and-sources.md)) → the phase that builds it →
a **v1** (required for success criteria) or **later** target → the acceptance check. Phase 13 walks
every `v1` row and records pass/fail; that report *is* the definition of "v1 done."

Source confidence per `05`: ✅ documented · 🟡 stated elsewhere · ❓ gap (our design).

## A. Projects & config (Phase 2)

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|------:|--------|--------|
| A1 | Load real `solo.yml` (`name`/`icon`/`processes{}`) | ✅ | 2 | v1 | Sample file → processes appear with correct fields |
| A2 | Per-process fields: `command`,`working_dir`,`auto_start`,`auto_restart`,`restart_when_changed`,`env` | ✅ | 2 | v1 | Each field honored at runtime |
| A3 | 1 MB file limit; empty/comment-only = empty config | ✅ | 2 | v1 | Oversize rejected; empty file valid |
| A4 | Validation with precise errors (no panic) | ❓ | 2 | v1 | Bad field → named error |
| A5 | JSON Schema for editor autocomplete | ❓ | 2 | later | `solo.schema.json` validates fixtures. **Delivered ahead of schedule (user request 2026-06-29):** `solo.schema.json` is generated from the `SoloYml` model (`schemars`, off-by-default `schema` feature) and committed at the repo root; a drift-guard + structural test (`config::schema`) runs in CI and `just lint`; generated `solo.yml` files carry a `# yaml-language-server: $schema=…` modeline (`plan/05 §12`). |
| A6 | Trust gate (untrusted blocks start/auto/restart/watch) | ✅ | 2 | v1 | Untrusted command cannot run by any path |
| A7 | Trust scoped to (project, command-variant hash); rename preserves | ✅ | 2 | v1 | Edit command → re-trust required; rename keeps trust |
| A8 | "Automatically trust command changes" setting | ✅ | 2 | later | User-saved change auto-trusts; external change does not. **Delivered ahead of schedule (user request 2026-06-29):** a **per-project**, default-**off** setting (`ProjectSettings.auto_trust_command_changes`); a user save auto-trusts via `Facade::write_shared_command`, while `ConfigEngine::sync` never trusts — proven headless (`facade::commands` tests incl. `an_external_solo_yml_edit_never_auto_trusts_even_with_the_setting_on`). Tauri command + Trust-section toggle wired; default/scope gap recorded in `plan/05 §12`. |
| A9 | Sync on change: debounce + hash-diff; add/update/remove; preserve renames | ✅ | 2 | v1 | Edit file → sync prompt with correct diff; no auto-start |
| A10 | Command auto-detection on first add | ✅ | 2 | **v1** | Open a folder with no `solo.yml` → one is auto-created from detected commands (npm/Cargo/Go/Procfile/Make/Just/Compose), trust-gated; nothing detected → a clean starter file. Delivered in the Phase-5 follow-up (user decision 2026-06-19). |
| A11 | Multiple projects + registry | ✅ | 2 | v1 | Two projects switchable |
| A12 | Local (app-state) vs shared (YAML) commands | ✅ | 2 | later | Local command never written to `solo.yml` |
| A13 | Project icon | ✅ | 2,5 | **v1** | Icon shows on project. A `solo.yml` `icon:` is resolved against the root (`ProjectView.icon`) and loaded into the sidebar avatar via the `project_icon` command (capped, image-only `data:` URL), with a name-initial monogram fallback. Pulled into v1 with the project-grouped sidebar (user decision 2026-06-20). |

## B. Process supervision (Phase 3)

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|------:|--------|--------|
| B1 | Three subtypes: Command / Agent / Terminal | ✅ | 3 | v1 | Each created with correct lifecycle |
| B2 | Status FSM (Stopped/Starting/Running/Crashed/Restarting/Stopping/Exhausted) | ✅❓ | 3 | v1 | Transitions match events |
| B3 | Start / stop / restart (per process) | ✅ | 3 | v1 | Controls affect only that process |
| B4 | Start-all / stop-all / restart-running (trusted only) | ✅ | 3 | v1 | Bulk ops over trusted commands |
| B5 | Run via login shell; correct `working_dir`/`env` | ✅ | 3 | v1 | Aliases/PATH resolve; cwd correct |
| B6 | Graceful stop: SIGTERM→grace→SIGKILL on process group | ❓ | 3 | v1 | Stop leaves zero child PIDs |
| B7 | Stop releases that process's todo locks; clears crash tracking | ✅ | 3 | v1 | Locks freed on stop |
| B8 | Orphan adoption/cleanup on relaunch (match project+name+command) | ✅ | 3 | v1 | Leftover child adopted or prompted |
| B9 | "Resume last session" for stopped agents | 🟡 | 3 | later | Stopped agent offers resume. **Delivered ahead of schedule (user request 2026-06-29):** a stopped resumable agent offers **Resume last session** beside Start; resume relaunches it with its provider's documented resume-last invocation (per-provider **Strategy** in `core::agents::resume` — Claude `--continue`, Codex `resume --last`, Gemini `--resume`, OpenCode/Copilot/Kimi `--continue`; Amp + Generic are recorded gaps, no fabricated flag), composed once at launch and replayed by `Supervisor::resume` without touching the fresh command. Surfaced on `ProcessView.resumable`. Headless evidence: `core::agents::resume` (6), `supervisor::resume_tests` (4), `facade_tests` resumable-per-provider, and a real-PTY `pty/tests/integration.rs::resume_relaunches_a_stub_agent_with_its_providers_resume_command`. UI (ProcessControls + TerminalPane via `/impeccable`, 4 vitest) → its real-window walk is the user-only step. Clean-room gap recorded in `plan/05 §12` + `KNOWN-DIVERGENCES.md` D-9. |
| B10 | Open a plain **Terminal** from the UI | ❓ | 3,7 | v1 | The launch picker opens a live interactive shell in the project dir; a second one is numbered. **Delivered 2026-07-21 (owner decision).** B1's Terminal subtype and its whole lifecycle shipped in Phase 3, and the sidebar's Terminals group in Phase 5, but **nothing in the app ever created one** — `Facade::launch_agent` was the sole production caller of `Registration::launched` and hard-coded `ProcessKind::Agent`, so the group could render terminals that could not exist. No matrix row covered UI creation (O9/F11 cover only `spawn_process` over MCP, still deferred to `orch-04`). Now `Facade::create_terminal` → one Tauri command → a terminal entry in the launch picker that `Cmd/Ctrl+T` (`new_agent_or_terminal`) already advertised. Ungated by construction (`Registration::launched` ⇒ no trust variant, no auto-start/-restart/-watch) and **UI-only** — no MCP/HTTP/CLI surface, so no caller supplies a command. Clean-room decisions (shell command, label numbering, scope) recorded in `plan/05 §12`. |

## C. Terminal I/O (Phase 4)

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|------:|--------|--------|
| C1 | Real PTY per process | ✅ | 4 | v1 | `vim`/agent TUI render & accept input |
| C2 | Full ANSI / color | ✅ | 4 | v1 | `ls --color` shows color |
| C3 | Interactive input (text + raw control bytes) | ✅ | 4 | v1 | Answer a `read`/agent prompt |
| C4 | Rendered output buffer | ✅ | 4 | v1 | Rendered screen text retrievable |
| C5 | Raw output buffer (control sequences) | ✅ | 4 | v1 | Raw stream retrievable |
| C6 | Resize (SIGWINCH/cols-rows) | ✅ | 4 | v1 | `tput cols` reflects resize |
| C7 | OSC parsing (title, bell) | ✅ | 4 | v1 | OSC title updates; bell detected |
| C8 | GPU/smooth rendering | 🟡 | 4 | later | webgl renderer; **DOM fallback** (xterm v6 removed canvas, D-10). **Delivered ahead of schedule (user request):** `@xterm/addon-webgl` lazy-loaded + activated after the terminal opens, reverting to the built-in DOM renderer when WebGL2 is unavailable or its context is lost (`onContextLoss`). Selection logic single-sourced in `ui/src/lib/terminalRenderer.ts` (5 vitest). Bundle: addon is its own ~123 kB/~35 kB-gzip on-demand chunk; main bundle +1.6 kB. Headless evidence: `lib/terminalRenderer.test.ts` (fallback-on-failure, context-loss-disposes, handle-dispose). Runtime FPS/visual = user-only walk (no display in CI). Gap in `plan/05 §12`. |
| C9 | Detach/attach with scrollback replay | ❓ | 4 | v1 | Reattach replays recent screen |

## D. Monitoring & self-healing (Phase 6)

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|------:|--------|--------|
| D1 | Per-process CPU & memory | ✅ | 6 | v1 | Busy proc shows moving CPU/RSS |
| D2 | Port discovery (`get_process_ports`) | ✅ | 6 | v1 | Dev server's port listed |
| D3 | Readiness (`wait_for_bound_port`) | ✅ | 6 | v1 | Block until port binds |
| D4 | Crash auto-restart, rate-limited **10/60s → exhausted** | ✅ | 6 | v1 | Repeated crash pauses after 10 |
| D5 | Restart banner + last crash output retained | ✅ | 6 | v1 | Banner appears before new output |
| D6 | File-watch restart (debounced, recursive, trusted-only) | ✅ | 6 | v1 | Touch watched file → 1 restart |
| D7 | File-watch default ignores (`.git`,`node_modules`,…) | ❓ | 6 | v1 | Editing ignored path → no restart |
| D8 | Native desktop notifications (crash/attention) | ✅ | 6 | v1 | Crash → libnotify toast |
| D9 | In-app toasts | 🟡 | 6 | later | In-app notification surface |
| D10 | Attention bell + unified unread (sidebar/title/dock) | 🟡 | 6 | later | Bell lights; click opens terminal |
| D11 | Auto-restart disabled during app shutdown | ✅ | 6 | v1 | Quit doesn't trigger restarts |
| D12 | Tracked descendant/child subprocess stats (CPU/mem/ports of spawned children) | 🟡 | 6 | later | A process's child shows its own CPU/RSS/port |

## E. Agents & idle (Phase 7)

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|------:|--------|--------|
| E1 | Agent tool config (Claude/Codex/Amp/Gemini/OpenCode/Generic) | ✅ | 7 | v1 | Configure & launch each type |
| E2 | `--version` auto-detect of installed CLIs | ✅ | 7 | v1 | Detect present agents |
| E3 | Per-tool: name, command, default args, prompt mode | ✅ | 7 | v1 | Defaults applied on launch |
| E4 | Launch picker (`Cmd/Ctrl+T`) + "agent with flags" modal | ✅ | 7 | v1 | Launch with edited flags |
| E5 | 5-state idle detection (IDLE/PERMISSION/THINKING/WORKING/ERROR) | ✅ | 7 | v1 | State tracks a real agent |
| E6 | Optional auto-summarization (headless, degradable) | ✅❓ | 7 | later | Summary when enabled; disabled OK |
| E7 | Agents spawning agents (cross-vendor) | ✅ | 7,9 | v1 | Lead spawns a worker via MCP |
| E8 | Agents authenticate via their **own** native flow (OAuth/API key) in the interactive PTY; Soloist manages no agent credentials | 🟡 | 7 | v1 | Fresh `claude` (no stored creds) completes its native login in its terminal; Soloist stores/injects no key/token; `$DISPLAY`/`BROWSER`/`ANTHROPIC_*` pass through |

## F. MCP server — core (Phase 8)

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|------:|--------|--------|
| F1 | `soloist-mcp` stdio transport + bundled helper | ✅ | 8 | v1 | Agent launches it; tools list. **"Bundled" became true for packaged installs 2026-07-03:** the `.deb` and `.AppImage` had shipped only the app binary (found via a user bug report — an installed app had no launchable helper at all), so both artifacts now carry `/usr/bin/soloist-mcp` and `/usr/bin/soloist-cli` via `bundle.linux.{deb,appimage}.files` + a `beforeBuildCommand` release build of both crates; the app additionally exports the helper to `<data dir>/bin` at startup so snippets carry one stable path on every format (an AppImage's mount dir changes each launch). Evidence: `dpkg -c` / AppDir listing of the rebuilt artifacts, `companion_bins` tests. |
| F2 | Setup snippet generation (Claude Code etc.) | ✅ | 8 | later | Generated `.mcp.json` works. **Delivered ahead of schedule (user request 2026-07-02):** Settings → Integrations generates per-client snippets from a data-driven table (Claude Code, Codex, Amp, OpenCode, Cursor, Windsurf, Cline — shapes verified against official client docs; Claude Desktop omitted: no Linux build, recorded in `plan/05 §12`). A new `mcp_setup_info` command resolves the helper path (data-dir export → sibling binary → PATH fallback, since 2026-07-03) and emits a `SOLOIST_APP_DATA_DIR` env entry only when overridden; copy button on the snippet. Evidence: `lib/integrations.test.ts` (7 vitest incl. JSON-parse validity per client), `IntegrationsPanel.test.tsx`, `commands::settings` helper-path tests, `docs/mcp-setup.md`. |
| F3 | Effective project scope (`select_project`/inferred) | ✅ | 8 | v1 | Tool acts on right project |
| F4 | Identity: `bind_session_process`/`register_agent`/`whoami`; `SOLOIST_PROCESS_ID` | ✅ | 8 | v1 | `whoami` resolves bound process |
| F5 | Project tools (`list_projects`,`select_project`,`get_project_status`,`get_project_stats`) | ✅ | 8 | v1 | Each returns live data |
| F6 | Process tools (`list_processes`,`get_process_status`,`start/stop/restart_process`,`rename`,`select`,`send_input`,`close`) | ✅ | 8 | v1 | Control a process over MCP |
| F7 | `send_input` with `wait_ms` rendered-tail | ✅ | 8 | v1 | Input + tail returned |
| F8 | Bulk tools (`start/stop/restart_all_commands`) | ✅ | 8 | v1 | Bulk over trusted commands |
| F9 | Output tools (`get_process_output`/`_raw`,`search_output`/`_raw`,`clear_output`,`flush_terminal_perf`) | ✅ | 8 | v1 | Read logs without UI |
| F10 | Services tools (`services_list`,`wait_for_bound_port`) | ✅ | 8 | v1 | Discover + wait for port |
| F11 | Agent/terminal tools (`spawn_process`,`spawn_agent`,`list_agent_tools`) | ✅ | 8 | v1 | Spawn a terminal/agent |
| F12 | Setup/support (`help`,`submit_solo_feedback`,`setup_agent_integration`) | ✅ | 8 | later | `setup_agent_integration` writes CLAUDE.md. **Delivered ahead of schedule (user request 2026-07-02):** a new always-on Setup group (tool surface 71 → 74). `help` returns the embedded agent guide from the core with **no app round-trip** (proven over real stdio with the app down); `submit_solo_feedback` stores locally (`feedback` table, schema v11; **D-13**); `setup_agent_integration` writes the guide into `AGENTS.md` (default) or `CLAUDE.md` as a marker-managed section — create/append/replace-in-place, idempotent re-runs. Evidence: `core::support` (15 tests), `facade::support`, `store::feedback`, ipc round-trips, app arms, mcp `tools/setup` + surface test; semantics in `plan/05 §12`. |
| F13 | Action tools honor trust + scope (scope authenticated via peer `SO_PEERCRED` → process **group**, or the peer's working **directory** for an agent Soloist did not launch) | ✅❓ | 8 | v1 | Untrusted action refused; forged bind/select to a sibling project refused (`ForeignProcess`/`ForeignProject`) |
| F14 | Prompt-template MCP tools (list/read/create/update/delete/export; default off) | 🟡 | 8 | later | Tool group toggles on; round-trips a template. **Delivered ahead of schedule (user request 2026-07-02):** six `prompt_template_*` tools behind a new default-**off** `McpFeatureGroup::PromptTemplates` (surface 74 → 80 when enabled; default gating proven over real stdio — the tools are absent). Templates: name unique per scope (project default / global), derived `{{placeholder}}`s, revision-guarded updates, portable `soloist.prompt-template/v1` export. Storage schema v12 with a `COALESCE(project_id, 0)` unique index (the SQLite NULL-UNIQUE trap; regression-tested). Evidence: core aggregate (11) + facade (6) + store (8, incl. 16-thread revision race) + ipc + app arms + mcp gating/projection tests; semantics in `plan/05 §12`. I13 (the UI view) stays `later`. |
| F15 | Prompt-template render (substitute `{{placeholder}}` values; exposed as an MCP tool, the MCP `prompts` primitive, and a UI fill/preview) | 🟡 | 8 | v1 | Render returns fully substituted text; a placeholder with no value is left literal in the output and reported; `prompts/get` with a missing required argument returns `-32602`; the `prompts` capability is **absent** from `initialize` while the `PromptTemplates` group is off. **Promoted to v1 by owner decision 2026-07-19**, reversing the "placeholder fill-in stays report-only" note on F14/I13 — those rows shipped storage and CRUD only, leaving `placeholders()` deriving names that nothing consumed. Solo's own fill-in is cited 🟡 at `plan/05` §10; the mechanism was never documented, so the substitution semantics are ours (clean-room) and recorded in `plan/05` §12. Scope is `TemplateKind::Prompt` only — resolved decision 4 (no agent-facing Scratchpad/Todo template tools) stands. Applying a rendered prompt to a running process is **not** in this row; it needs the trust gate and is tracked separately.

## G. Coordination layer (Phase 9)

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|------:|--------|--------|
| G1 | Scratchpads CRUD + tags/archive/transfer (free-form Markdown `body`; D-7 enforced-structure divergence **superseded** 2026-07-18, rich TipTap editor + templates) | ✅ | 9 | v1 | Read/write a scratchpad |
| G2 | Scratchpad **revision-guarded** writes | ✅ | 9 | v1 | Stale write → conflict |
| G3 | Todos: create/list/get/update/complete/delete (free-form Markdown `body` + `TodoStatus`; D-8 enforced-structure divergence **superseded** 2026-07-18) | ✅ | 9 | v1 | CRUD a todo |
| G4 | Todo tags, **blockers**, comments, transfer | ✅ | 9 | v1 | Blocker gates a todo |
| G5 | Todo locks (process-owned, auto-release on close) | ✅ | 9 | v1 | Lock frees when process closes |
| G6 | Lease locks (`lock_acquire/status/release`, TTL+owner) | ✅❓ | 9 | v1 | Lock auto-expires/releases |
| G7 | Timers (`timer_set` delivers `body` as fresh turn) | ✅ | 9 | v1 | Timer fires into agent |
| G8 | `timer_fire_when_idle_any/all` (token-free waiting) | ✅ | 9 | v1 | Fires when children idle |
| G9 | Timer mgmt (`cancel`/`pause`/`resume`/`list`) | ✅ | 9 | v1 | Manage timers |
| G10 | Key-value (`kv_set/get/delete/list`, default off) | ✅ | 9 | v1 | JSON state round-trips |
| G11 | Coordination state persists across app restart (SQLite) | ❓ | 9 | v1 | Todos survive relaunch |
| G18 | **Optional todo↔scratchpad association** — a live `scratchpad_id` column on todos (never required), MCP `scratchpad?` params on `todo_create`/`todo_update` only, `todo_get`/`todo_list` returning the linked scratchpad's id **and** name | ❓ | macos-native-ux | v1 | A todo created with a scratchpad reports it by id + name; **a todo with no scratchpad is valid and untouched by every path**; renaming the scratchpad keeps the link resolving (only the durable id is stored); on `todo_update` an **omitted** `scratchpad` leaves the link unchanged while an explicit `null` clears it; an unknown name is refused and writes nothing; deleting the scratchpad clears the link; a cross-project `todo_transfer` clears it; a cross-project `scratchpad_transfer` **moves the linked todos with it and keeps the link**, clearing blockers that name todos left behind and dropping their locks, leaving unlinked todos untouched, all in one transaction. Soloist extension — `plan/05` §12, `KNOWN-DIVERGENCES` **D-18** |

> **G18 is a Soloist extension, not a Solo parity row.** `plan/05` records no todo↔scratchpad
> association for Solo and no Solo page denies one either — the public record is **silent**, and per
> `CLAUDE.md` §9 that silence is the gap, decided in `plan/05` §12. It is `v1` because it shipped and
> the owner directed it, not because Solo is known to have it. Numbering continues past the
> G12–G17 backlog below, which was allocated first.

### Free-form follow-up backlog (unlocked by the D-7/D-8 reversal — planned, non-gating)

> These slices become straightforward once scratchpads/todos hold a free-form Markdown body (owner
> decision 2026-07-18, `KNOWN-DIVERGENCES` D-7/D-8 superseded). **None is built by the rich-editor-design
> phases A–F** — each is a `later` planned row in the owner-approved order (resolved decision 6) and needs
> its own session before build (§7, no gold-plating). `Phase` is unscheduled (`—`) until a slice is picked
> up. The MCP file-io tools stay gated behind the still-deferred project-root FS sandbox (`plan/05` §12).

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|:----:|--------|--------|
| G12 | `scratchpad_append` + `scratchpad_append_section` (append text, or append under a heading, to the body) | ✅ | — | later | Append extends the body without a full rewrite; revision-guarded |
| G13 | `scratchpad_find` + `scratchpad_tail` (search within a body; read the trailing N lines) | ✅ | — | later | `find` returns matching spans; `tail` returns the last N lines |
| G14 | Scratchpad import + file round-trip — MCP `scratchpad_save_to_file`/`_load_from_file` (behind the deferred project-root FS sandbox) + UI Import (UI **Export `.md`** shipped in the panel, O5) | ✅ | — | later | Round-trip a scratchpad to/from a file inside the project root |
| G15 | Create scratchpad from a terminal selection (capture selected PTY output into a new note) | 🟡 | — | later | Selecting terminal output offers "New scratchpad from selection" |
| G16 | Inline images in scratchpad / todo bodies | 🟡 | — | later | A pasted/attached image renders in the editor and persists |
| G17 | Todo **priority** field (High/Medium/Low) + bulk actions | 🟡 | — | later | A todo carries a priority; a bulk action applies to a selection |

## DG. Diagrams — Mermaid (Soloist extension, `solo.yml`-independent)

> **Soloist-only, not Solo parity.** `plan/05` records no diagram or Mermaid capability for Solo, and
> no Solo page denies one — the public record is **silent**, and per `CLAUDE.md` §9 that silence is the
> gap, decided in `plan/05` §12 and `KNOWN-DIVERGENCES` **D-20**. These rows are `v1` because the owner
> directed the feature (2026-07-24, the `mermaid-diagrams` initiative), not because Solo is known to
> have it. A Diagram is a first-class coordination document (sibling of scratchpads/todos) whose body
> is a raw Mermaid `source` string; one reusable renderer serves both the standalone panel and the
> in-note editor.

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|:----:|--------|--------|
| DG1 | Diagram document CRUD — first-class coordination doc holding raw Mermaid `source`, tags/archive, **revision-guarded**; durable (`DiagramId`, migration v18), project-scoped, survives restart | ❓ | mermaid-diagrams | v1 | Read/write a diagram; a stale write → `DiagramRevisionConflict`; a diagram survives relaunch |
| DG2 | `diagram_*` MCP tools (9 tools, default-**ON** group `Diagrams`), project-scoped, ungated by trust | ❓ | mermaid-diagrams | v1 | An agent `diagram_write`s a diagram and it appears live in the roster; a bound agent in project A cannot touch project B's diagrams |
| DG3 | Standalone **Diagrams tab** in the Orchestration pane — roster + source-editor/live-preview, live on `DiagramChanged` | ❓ | mermaid-diagrams | v1 | Create/edit/rename/archive a diagram in the tab; an AI-written diagram appears without a manual refresh |
| DG4 | Diagram panel **toolbox** — zoom/pan/fit+reset, copy source + SVG, export SVG/PNG/`.mmd`, fullscreen, per-diagram theme override (Mermaid frontmatter) | ❓ | mermaid-diagrams | v1 | Each toolbox action works; export writes a valid file; "Follow app" theme tracks light/dark |
| DG5 | **Mermaid in notes** — a ```` ```mermaid ```` fenced block renders in the scratchpad/todo TipTap editor with a source⇄preview toggle; round-trips as Markdown | ❓ | mermaid-diagrams | v1 | Type a mermaid fence → it renders; a parse error shows the editable source + message; `roundTrip(roundTrip(x)) == roundTrip(x)` |
| DG6 | One **reusable, theme-following, lazy-loaded** renderer shared by the panel and the editor — `base` theme from OKLCH tokens, `strict` security under the app CSP, code-split chunk | ❓ | mermaid-diagrams | v1 | The diagram recolors when the app theme flips; Mermaid loads in its own chunk (measured, not in the initial bundle); renders under the CSP unchanged |

## H. HTTP API & CLI (Phase 10)

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|------:|--------|--------|
| H1 | Loopback API `127.0.0.1:24678`; mutation auth header; localhost CORS | ✅ | 10 | v1 | Mutation needs header |
| H2 | Read endpoints (`/health`,`/status`,`/processes`,`/processes/:id/ports`,`/projects`) | ✅ | 10 | v1 | Each returns JSON |
| H3 | Mutation endpoints (process start/stop/restart; project bulk incl. `reload`; `spawn-agent`; `transfer-todo`/`transfer-scratchpad`; `/focus`) | ✅ | 10 | v1 | `POST .../restart` works |
| H4 | `soloist` CLI over the API (status/start/stop/restart/logs/focus/open/spawn) | ✅ | 10 | v1 | `soloist status` prints table. **Packaged 2026-07-03:** the `.deb`/`.AppImage` now ship the binary at `/usr/bin/soloist-cli` (it had not been packaged at all); the installed command is `soloist-cli` because the GUI app owns `/usr/bin/soloist` — recorded in `KNOWN-DIVERGENCES.md` D-14. |

## I. UX & shell (Phase 11)

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|------:|--------|--------|
| I1 | Sidebar process tree grouped by project → Agents/Terminals/Commands (collapse persists per project, reorder) | 🟡 | 5,11 | v1 | Grouped tree renders. Each opened project is a collapsible node (icon + name + running count + per-project bulk controls) over its non-empty kind subgroups; collapse persists per project and per subgroup. Reorder (drag) → Phase 11. |
| I2 | Command palette (`Ctrl+K`) | ✅ | 11 | v1 | Run any action |
| I3 | Jump palette (`Ctrl+E`) + attention jump (`Ctrl+Shift+E`) | ✅ | 11 | later | Jump to a destination. **Delivered ahead of schedule (user request 2026-06-30) — partial:** `Ctrl+E` Quick Jump fuzzy-searches **processes + open projects** (a process row focuses its terminal; a project row opens its settings), built on the shared palette shell + `ProcessCommandItem` and wired to the remappable `quick_jump` keymap action. **Still deferred:** attention-jump (`Ctrl+Shift+E`, unread-only), todo/scratchpad jump targets (need a per-project snapshot not pre-loaded at the App shell — D-12), and the `soloist://` copy-link (folded into I4). Headless evidence: `QuickJumpPalette.test.tsx` (4). Real-window walk is the user-only step. |
| I4 | `soloist://` deep links | ✅ | 11 | later | Link opens target |
| I5 | Light/dark/system themes (app + terminal) | ✅ | 11 | v1 | Toggle restyles incl. xterm |
| I6 | Keyboard-first nav (remapped to Ctrl/Super) | ✅ | 11 | v1 | Dashboard usable no-mouse |
| I7 | Settings screen (Appearance/Terminal/Notifications/Agents/Tools/MCP/Hotkeys) | ✅ | 11 | v1 | Settings persist |
| I8 | Execution profiles (project-level shell/runtime) | ✅ | 11 | later | Command runs under chosen profile |
| I9 | Open in editor (`code`/`zed`/…); default terminal | ✅ | 11 | v1 | Opens project root |
| I10 | Env capture via `$SHELL -ilc env`, cached 10 min | ✅ | 11 | v1 | Version-manager PATH visible |
| I11 | First-launch guided demo project | 🟡 | 11 | later | Demo appears on first run |
| I12 | Activity Monitor view (cross-project; flat/tree; project/type/status/ports filters; sortable CPU/mem/port columns; subprocess actions) | 🟡 | 11 | later | Monitor lists processes + descendants; filter/sort works |
| I13 | Templates UI in Settings (create/edit/delete/duplicate; grouped by `TemplateKind` Prompt/Scratchpad/Todo; global default-per-kind selector) — the Prompt section **is** the reserved prompt-templates view | ✅ | 11 | later | **Delivered 2026-07-18** (rich-editor-design Phase E, commit `156af8b`): a Settings → **Templates** tab manages the unified `Template` aggregate over the shared rich editor with revision-guarded autosave + conflict banner; the Prompt section lists/edits global prompt templates (satisfies this reserved I13 view); Scratchpad/Todo sections carry a default-template `NullableSelect` that seeds new empty documents through the one core seam. Per-project default selection stays deferred (global-only in v1, resolved decision 3); placeholder fill-in stays report-only. **Superseded 2026-07-20 by F15:** the Prompt section now lists and edits **both** scopes — project-scoped templates were invisible while being the MCP path's *default* scope, so a template an agent created could not be seen or corrected in the app. The five Tauri template commands and `DomainEvent::TemplateChanged` carry the owning project, and each kind renders one group per scope. Placeholder fill-in is no longer report-only either: the Prompt section renders a live preview through the same core command MCP reaches. The v1 scope of *this* row is unchanged (it stays `later`); F15 is what carries the widening. |
| I14 | Nested child-agent display (agent-spawned agents nested under their parent in the sidebar) | 🟡 | 5,11 | v1 ✅ | A spawned worker renders nested (collapsible) under its lead's row in the sidebar Agents group; re-roots flat when the lead closes; flat again next run (lineage is per-run). Delivered 2026-07-02 alongside O3's orchestration-pane tree |

### I7 decomposed — Settings detail (Phase 11a per-project · 11b global)

> I7 above is the umbrella row; these are its concrete sub-features, sourced field-by-field from the Solo
> demo "Your new agentic development environment" (Aaron Francis, `youtube.com/watch?v=kVyFCcP6B28`). Full
> field inventory + design in `plan/phases/phase-11a-project-settings.md` and `…-11b-global-settings.md`.
> Both surfaces share **one settings base** (`plan/06` §5.9): a generic `SettingsStore<K, D>` over a
> serde-default document — adding a setting is one field, not a new store.

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|------:|--------|--------|
| I7s | **Settings base:** generic `SettingsStore<K, D>` + serde-default document + `SettingsRepo<K, D>` port; reused by global (`K=()`) and per-project (`K=ProjectId`) | ✅ | 11a | v1 | Both surfaces persist through the one base; adding a field needs no new store/migration |
| I7a | Project **Overview** (directory + actions, `solo.yml` ✓Valid/invalid badge + refresh, running/total counts) | ✅ | 11a | v1 | Badge reflects real validity; actions open root |
| I7b | Project **run policy** (project auto-start gate, editor override, icon — `solo.yml`, rejects `.svg`) | ✅ | 11a | v1 | Persists; icon rejects `.svg`; override falls back to global default |
| I7c | Project **notifications** (crash & exit alerts, terminal alerts) | ✅ | 11a | v1 | Toggles persist (app-local) |
| I7d | **Commands** list + per-command editor (name/command, auto-start, auto-restart, terminal alerts, file-watch globs) + "Add command" modal | ✅ | 11a | v1 | Each field edits the right `solo.yml`/local target |
| I7e | Command **storage** shared (`solo.yml`) ⇄ local ("Make local"); local never written to `solo.yml` | ✅ | 11a | v1 | Move round-trips; local command leaves `solo.yml` byte-unchanged |
| I7f | Global **Appearance** (theme Light/Dark/System, interface font scale; terminal font/weight/scale/line-height/letter-spacing → xterm) | ✅ | 11b | v1 | Theme + terminal typography restyle app **and** xterm; persist |
| I7g | Global **Sidebar** (filter input, hide empty sections, project/process CPU+mem header thresholds, hover actions, settings footer) | ✅ | 11b | v1 | Each control changes the live sidebar projection; persists |
| I7h | Global **Hotkeys** (remappable keymap, scoped General/Sidebar/Terminal, search, Reset all to defaults; remap ⌘→Ctrl/Super) | ✅ | 11b | v1 | Remap takes effect + survives restart; reset restores defaults; same key OK across scopes |
| I7i | Global **Agents** (tool registry detect/add/edit/enable; auto-summarization tool+model, **OFF by default**) | ✅ | 11b | v1 | Registry edits persist; summarization stays opt-in |
| I7j | Global **Tools** (default editor, default terminal; editor overridable per-project) | ✅ | 11b | v1 | Defaults persist; project override wins |
| I7k | Global **Integrations** (MCP enablement + per-group toggles + setup snippet [stdio, D4]; HTTP API toggle + endpoint list [`24678`, H1]) | ✅ | 11b | v1 | MCP group toggle changes the served tool surface (reuses G10); HTTP toggle reflects Phase 10 |
| I7l | Global **Notifications** tab | ❓ | 11b | v1 | **NOT SHOWN in source — decide from `plan/05`/docs before building; do not invent** |
| I7m | Global **Account** tab | ❓ | 11b | later | **NOT SHOWN; N/A under D3 (no licensing). Proposed: app info / data dir / reset — needs decision** |

## O. Orchestrator (track `orch-00`–`orch-05`)

A standalone build track that makes the multi-agent **orchestrator** experience legible and first-class.
The orchestration *mechanism* (a lead spawns workers, hands out blockered todos, waits token-free on a
fire-when-idle timer, wakes to integrate) is **already built and `Verified`** — the passing
`crates/pty/tests/orchestration.rs` (E7). This track is therefore **UX + formalization + deferred tools,
not new primitives**: every row *consumes* the existing C6/C4/C2 behavior through the one `Facade`. Full
charter, dependencies, and per-phase definition of done: [`orchestrator/README.md`](orchestrator/README.md).

> **UX source (`🟡`):** the public Solo demo "Agent orchestration, simplified" (Aaron Francis,
> `youtube.com/watch?v=WAKGhlzpYgs`), re-verified frame-by-frame 2026-06-28 — matched for *feel* only,
> never assets/strings (clean-room, `CLAUDE.md` §9). "Orchestrator" is not a documented Solo concept; it
> is a Soloist-original composition recorded as a gap decision in [`05` §12](05-solo-reference-and-sources.md).
> `Src`: `✅` documented name · `🟡` stated by the demo · `❓` our design.

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|------|--------|--------|
| O1 | Orchestration read-model: one `Facade` query projecting the lead→worker tree, todos, timers, leases, scratchpads, kv per project | ❓ | orch-00 | v1 | Query returns the snapshot; reflects a mutation |
| O2 | Coordination `DomainEvent`s (todo / timer / lease / scratchpad / kv changed) for a live UI | ❓ | orch-00 | v1 | A mutation emits its event; UI updates without polling |
| O3 | Agent lineage: parent `ProcessId` recorded on `spawn_agent`; nested lead→worker tree (promotes `later` row I14) | 🟡 | orch-01 | v1 | A spawned worker nests under its lead |
| O4 | Live orchestration tree UI with per-agent activity (Working/Thinking/Idle/Permission/Error) | 🟡 | orch-01 | v1 | Tree renders lead + workers with live glyphs |
| O5 | Scratchpad panel — free-form Markdown `body` in a rich TipTap editor (slash commands, autosave, undo/redo, in-note find with wrap-around, list sort recent/name, archive control + Ctrl+Shift+W, Copy Markdown, Export `.md` via the native save dialog), revision-guarded edit, living-doc view (D-7 superseded 2026-07-18). **Extended by the `macos-native-ux` initiative (2026-07-19):** the outline is a real **scroll-spy table of contents** (active heading follows the reading position, rail auto-scrolls, `aria-current`), scratchpad **titles are humanized for display** (the raw handle stays visible as mono metadata — no stored title field) with **inline Finder-style rename** over the one core rename command | ❓ | orch-02, macos-native-ux | v1 | Read/edit a scratchpad; stale edit → conflict. TOC: the active item tracks the scroll position and the rail follows it; a slug name displays as prose while the handle stays addressable; rename commits on Enter, cancels on Esc, and a taken name surfaces as a field error |
| O6 | To-do board UI — blockers / locks / comments / status, blocker-gate visible. **Extended by the `macos-native-ux` initiative (2026-07-19):** the board **groups by scratchpad** by default behind a segmented `All \| By scratchpad` toggle (persisted collapse; unlinked todos in a first-class "No scratchpad" group — G18), and expanded todo **bodies render as read-only Markdown** through the existing lazy TipTap boundary (no second editor chunk) | ❓ | orch-02, macos-native-ux | v1 | Blocker gating + lock owner shown; complete refused when blocked. Board defaults to grouped; the toggle flattens it; an unlinked todo has a real home rather than an error bucket; a todo body renders as Markdown, not raw `##`/`-` markers |
| O7 | Timers & fire-when-idle panel — armed timers, `waiting_on`, max-wait countdown, injected-turn `body` preview | 🟡 | orch-03 | v1 | A `fire_when_idle` arm shows `waiting_on` + countdown |
| O8 | Wake-cycle visibility — timer fires → `body` delivered as a fresh turn (named with *why* it woke), surfaced on the lead | 🟡 | orch-03 | v1 | Fired timer's body appears on the lead; timer leaves the panel |
| O9 | `spawn_process` (arbitrary terminal over MCP) with its trust treatment | ✅ name / ❓ trust | orch-04 | v1 | Trusted spawn works; untrusted / cross-project refused |
| O10 | Cross-project `scratchpad_transfer` / `todo_transfer` with cross-scope authorization | ✅ | orch-04 | v1 | In-scope transfer works (HTTP `transfer-todo`/`transfer-scratchpad`); cross-scope refused over MCP (delivered 2026-07-01) |
| O11 | Orchestrator capability — documented recipe + setup guidance + first-class status | ❓ | orch-05 | v1 | Recipe doc + `setup_agent_integration` guidance; E2E walk passes |
| O12 | Todo **comment authorship** — a comment records its creating bound actor (`author_actor_id` + display author), populated by the core on create; surfaced on the to-do board | 🟡 | orch-02 | v1 | A comment created by a bound process records its actor; the board shows who wrote each comment; reverses the `05` "no author attribution" decision |
| O13 | **Spawn orchestration-context preamble** — `spawn_agent`/`spawn_process` deliver a first-turn `[SOLO ORCHESTRATION CONTEXT]` preamble (the worker's identity + the coordination tools), mirroring the demo's `include_agent_instructions` | 🟡 | orch-04 | v1 | A spawned worker receives the preamble as its first turn and can use the primitives with no skills loaded; applies to the built `spawn_agent` (not gated on the O9 arbitrary-spawn trust work) |
| O14 | **`solo://` copy-link handoff** — a stable `solo://proj/<id>/scratchpad\|todo/<id>` link + a "Copy link" affordance + a core resolver so a receiving agent reads the target; promotes the orchestrator slice of I4 to v1 | 🟡 name (`05` §10) / ❓ shape | orch-02 | v1 | Copy a scratchpad's link; a bound agent given the link reads it; a malformed / foreign-scope link is refused |

> `later` (tracked, non-gating — do **not** gold-plate): a deep cross-project "Activity Monitor" (I12)
> and LLM auto-summarization of worker output (E6, OFF by default). (The prompt-template UI, I13, was
> delivered 2026-07-18 as the Templates Settings tab.)

## J. Packaging (Phase 12)

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|------:|--------|--------|
| J1 | `.deb` install on Ubuntu 22.04 (x86_64) | ✅ | 12 | v1 | apt install → launches |
| J2 | `.AppImage` runs on clean Ubuntu 22.04+ (bundled webkit; 20.04 infeasible — `KNOWN-DIVERGENCES` D-11) | ✅ | 12 | v1 | Runs without a manual webkit install |
| J3 | Desktop entry + icon (our own art) + MIME for `solo.yml` | ❓ | 12 | v1 | Menu entry + icon present |
| J4 | In-app update check / release feed | 🟡 | 12 | later | Checks feed; manual update |
| J5 | Checksums / provenance | ❓ | 12 | later | SHA-256 published |

## K. Longevity & quality (Phase 13)

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|------:|--------|--------|
| K1 | Idle footprint ≈ "less RAM than a Chrome tab" | 🟡 | 13 | v1 | Measured RSS recorded vs budget |
| K2 | No PID/FD/memory leak over multi-hour soak | ❓ | 13 | v1 | Flat counts; zero zombies |
| K3 | Backpressure under chatty output | ❓ | 13 | v1 | Bounded memory; UI responsive |
| K4 | Self-supervised internal tasks restart | ❓ | 13 | v1 | Killed sampler self-restarts |
| K5 | Graceful degradation (summarizer/ports offline) | ❓ | 13 | v1 | Core unaffected |
| K6 | Crash recovery (force-quit → orphan reconcile, SQLite intact) | ✅❓ | 13 | v1 | Clean relaunch |
| K7 | Dependency-direction CI (core has no adapter imports) | ❓ | 13 | v1 | CI check green |

## Deliberately excluded

Licensing/Free-Pro/limits, license validation/analytics, Raycast extension, hosted update manifest/
account, macOS/Windows/arm64 builds, git worktrees/sandboxes, required cloud summarizer, Solo's
name/logo/assets. (See `00-vision-and-scope.md`.)
