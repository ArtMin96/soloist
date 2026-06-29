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
| F1 | `soloist-mcp` stdio transport + bundled helper | ✅ | 8 | v1 | Agent launches it; tools list |
| F2 | Setup snippet generation (Claude Code etc.) | ✅ | 8 | later | Generated `.mcp.json` works |
| F3 | Effective project scope (`select_project`/inferred) | ✅ | 8 | v1 | Tool acts on right project |
| F4 | Identity: `bind_session_process`/`register_agent`/`whoami`; `SOLOIST_PROCESS_ID` | ✅ | 8 | v1 | `whoami` resolves bound process |
| F5 | Project tools (`list_projects`,`select_project`,`get_project_status`,`get_project_stats`) | ✅ | 8 | v1 | Each returns live data |
| F6 | Process tools (`list_processes`,`get_process_status`,`start/stop/restart_process`,`rename`,`select`,`send_input`,`close`) | ✅ | 8 | v1 | Control a process over MCP |
| F7 | `send_input` with `wait_ms` rendered-tail | ✅ | 8 | v1 | Input + tail returned |
| F8 | Bulk tools (`start/stop/restart_all_commands`) | ✅ | 8 | v1 | Bulk over trusted commands |
| F9 | Output tools (`get_process_output`/`_raw`,`search_output`/`_raw`,`clear_output`,`flush_terminal_perf`) | ✅ | 8 | v1 | Read logs without UI |
| F10 | Services tools (`services_list`,`wait_for_bound_port`) | ✅ | 8 | v1 | Discover + wait for port |
| F11 | Agent/terminal tools (`spawn_process`,`spawn_agent`,`list_agent_tools`) | ✅ | 8 | v1 | Spawn a terminal/agent |
| F12 | Setup/support (`help`,`submit_solo_feedback`,`setup_agent_integration`) | ✅ | 8 | later | `setup_agent_integration` writes CLAUDE.md |
| F13 | Action tools honor trust + scope (scope authenticated via peer `SO_PEERCRED` → process group) | ✅❓ | 8 | v1 | Untrusted action refused; forged bind/select to a sibling project refused (`ForeignProcess`/`ForeignProject`) |
| F14 | Prompt-template MCP tools (list/read/create/update/delete/export; default off) | 🟡 | 8 | later | Tool group toggles on; round-trips a template |

## G. Coordination layer (Phase 9)

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|------:|--------|--------|
| G1 | Scratchpads CRUD + tags/append/archive/transfer/file-io | ✅ | 9 | v1 | Read/write a scratchpad |
| G2 | Scratchpad **revision-guarded** writes | ✅ | 9 | v1 | Stale write → conflict |
| G3 | Todos: create/list/get/update/complete/delete | ✅ | 9 | v1 | CRUD a todo |
| G4 | Todo tags, **blockers**, comments, transfer | ✅ | 9 | v1 | Blocker gates a todo |
| G5 | Todo locks (process-owned, auto-release on close) | ✅ | 9 | v1 | Lock frees when process closes |
| G6 | Lease locks (`lock_acquire/status/release`, TTL+owner) | ✅❓ | 9 | v1 | Lock auto-expires/releases |
| G7 | Timers (`timer_set` delivers `body` as fresh turn) | ✅ | 9 | v1 | Timer fires into agent |
| G8 | `timer_fire_when_idle_any/all` (token-free waiting) | ✅ | 9 | v1 | Fires when children idle |
| G9 | Timer mgmt (`cancel`/`pause`/`resume`/`list`) | ✅ | 9 | v1 | Manage timers |
| G10 | Key-value (`kv_set/get/delete/list`, default off) | ✅ | 9 | v1 | JSON state round-trips |
| G11 | Coordination state persists across app restart (SQLite) | ❓ | 9 | v1 | Todos survive relaunch |

## H. HTTP API & CLI (Phase 10)

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|------:|--------|--------|
| H1 | Loopback API `127.0.0.1:24678`; mutation auth header; localhost CORS | ✅ | 10 | v1 | Mutation needs header |
| H2 | Read endpoints (`/health`,`/status`,`/processes`,`/processes/:id/ports`,`/projects`) | ✅ | 10 | v1 | Each returns JSON |
| H3 | Mutation endpoints (process start/stop/restart; project bulk; `/focus`) | ✅ | 10 | v1 | `POST .../restart` works |
| H4 | `soloist` CLI over the API (status/start/stop/restart/logs/focus; spawn/open deferred — `05` §12) | ✅ | 10 | v1 | `soloist status` prints table |

## I. UX & shell (Phase 11)

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|------:|--------|--------|
| I1 | Sidebar process tree grouped by project → Agents/Terminals/Commands (collapse persists per project, reorder) | 🟡 | 5,11 | v1 | Grouped tree renders. Each opened project is a collapsible node (icon + name + running count + per-project bulk controls) over its non-empty kind subgroups; collapse persists per project and per subgroup. Reorder (drag) → Phase 11. |
| I2 | Command palette (`Ctrl+K`) | ✅ | 11 | v1 | Run any action |
| I3 | Jump palette (`Ctrl+E`) + attention jump (`Ctrl+Shift+E`) | ✅ | 11 | later | Jump to a destination |
| I4 | `soloist://` deep links | ✅ | 11 | later | Link opens target |
| I5 | Light/dark/system themes (app + terminal) | ✅ | 11 | v1 | Toggle restyles incl. xterm |
| I6 | Keyboard-first nav (remapped to Ctrl/Super) | ✅ | 11 | v1 | Dashboard usable no-mouse |
| I7 | Settings screen (Appearance/Terminal/Notifications/Agents/Tools/MCP/Hotkeys) | ✅ | 11 | v1 | Settings persist |
| I8 | Execution profiles (project-level shell/runtime) | ✅ | 11 | later | Command runs under chosen profile |
| I9 | Open in editor (`code`/`zed`/…); default terminal | ✅ | 11 | v1 | Opens project root |
| I10 | Env capture via `$SHELL -ilc env`, cached 10 min | ✅ | 11 | v1 | Version-manager PATH visible |
| I11 | First-launch guided demo project | 🟡 | 11 | later | Demo appears on first run |
| I12 | Activity Monitor view (cross-project; flat/tree; project/type/status/ports filters; sortable CPU/mem/port columns; subprocess actions) | 🟡 | 11 | later | Monitor lists processes + descendants; filter/sort works |
| I13 | Prompt templates UI (create/edit/search/duplicate; global+project scope; placeholder fill-in) | 🟡 | 11 | later | Template saved, filled, and applied to an agent |
| I14 | Nested child-agent display (agent-spawned agents nested under their parent in the sidebar) | 🟡 | 5,11 | later | Spawned agent appears under its parent |

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
| O5 | Scratchpad panel — disciplined `ScratchpadDoc`, revision-guarded edit, living-doc view | ❓ | orch-02 | v1 | Read/edit a scratchpad; stale edit → conflict |
| O6 | To-do board UI — blockers / locks / comments / status, blocker-gate visible | ❓ | orch-02 | v1 | Blocker gating + lock owner shown; complete refused when blocked |
| O7 | Timers & fire-when-idle panel — armed timers, `waiting_on`, max-wait countdown, injected-turn `body` preview | 🟡 | orch-03 | v1 | A `fire_when_idle` arm shows `waiting_on` + countdown |
| O8 | Wake-cycle visibility — timer fires → `body` delivered as a fresh turn (named with *why* it woke), surfaced on the lead | 🟡 | orch-03 | v1 | Fired timer's body appears on the lead; timer leaves the panel |
| O9 | `spawn_process` (arbitrary terminal over MCP) with its trust treatment | ✅ name / ❓ trust | orch-04 | v1 | Trusted spawn works; untrusted / cross-project refused |
| O10 | Cross-project `scratchpad_transfer` / `todo_transfer` with cross-scope authorization | ✅ name / ❓ scope | orch-04 | v1 | In-scope transfer works; cross-scope refused |
| O11 | Orchestrator capability — documented recipe + setup guidance + first-class status | ❓ | orch-05 | v1 | Recipe doc + `setup_agent_integration` guidance; E2E walk passes |
| O12 | Todo **comment authorship** — a comment records its creating bound actor (`author_actor_id` + display author), populated by the core on create; surfaced on the to-do board | 🟡 | orch-02 | v1 | A comment created by a bound process records its actor; the board shows who wrote each comment; reverses the `05` "no author attribution" decision |
| O13 | **Spawn orchestration-context preamble** — `spawn_agent`/`spawn_process` deliver a first-turn `[SOLO ORCHESTRATION CONTEXT]` preamble (the worker's identity + the coordination tools), mirroring the demo's `include_agent_instructions` | 🟡 | orch-04 | v1 | A spawned worker receives the preamble as its first turn and can use the primitives with no skills loaded; applies to the built `spawn_agent` (not gated on the O9 arbitrary-spawn trust work) |
| O14 | **`solo://` copy-link handoff** — a stable `solo://proj/<id>/scratchpad\|todo/<id>` link + a "Copy link" affordance + a core resolver so a receiving agent reads the target; promotes the orchestrator slice of I4 to v1 | 🟡 name (`05` §10) / ❓ shape | orch-02 | v1 | Copy a scratchpad's link; a bound agent given the link reads it; a malformed / foreign-scope link is refused |

> `later` (tracked, non-gating — do **not** gold-plate): a deep cross-project "Activity Monitor" (I12),
> prompt-template UI (I13), and LLM auto-summarization of worker output (E6, OFF by default).

## J. Packaging (Phase 12)

| ID | Feature | Src | Phase | Target | Verify |
|----|---------|-----|------:|--------|--------|
| J1 | `.deb` install on Ubuntu 22.04 (x86_64) | ✅ | 12 | v1 | apt install → launches |
| J2 | `.AppImage` runs on clean Ubuntu 20.04 (bundled webkit) | ✅ | 12 | v1 | Runs without manual deps |
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
