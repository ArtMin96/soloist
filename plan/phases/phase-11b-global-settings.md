# Phase 11b — Global Settings

**Goal:** Build the **global Settings window** — the app-wide preference surface with tabs **Appearance ·
Notifications · Sidebar · Hotkeys · Agents · Tools · Integrations · Account**. This is the detailed
build-out of Phase 11's I7 ("Settings screen", Task 6), extracted here so every tab and field is listed.

**Delivers:** the Settings window + all tab panels, persisted in the **existing global `Settings`
document** (`crates/core/src/settings.rs`, one singleton record, migration v9). **Architecture:** Tauri
adapter + frontend over `Facade` settings methods; **extend the `Settings` document field-by-field** (one
sub-document per tab), each `#[serde(default)]` for forward-compat. The MCP/HTTP integration toggles reuse
the existing `McpToolGroups` enablement work already landed in Phase 11.

## Position & provenance
- **Sits after Phase 11 (UX Polish), before Phase 12 (Packaging).** Depends on Phase 5 (dashboard/theme
  tokens), Phase 7 (agent tool registry + summarization), Phase 8/9 (MCP), Phase 10 (HTTP API), and Phase
  11 (theming, keyboard nav, the `Settings`/`SettingsStore` aggregate). Does **not** depend on 12/13.
- **Source of every Solo fact below:** the screencast *"Your new agentic development environment"* by Aaron
  Francis, `https://www.youtube.com/watch?v=kVyFCcP6B28`, the global-settings tour at **7:16–9:36**
  (`Appearance`, `Sidebar`, `Hotkeys`, `Agents`, `Tools`, `Integrations` opened on camera). Read
  frame-by-frame. **`Notifications` and `Account` were visible in the tab bar but never opened** — their
  fields are marked **NOT SHOWN** and must be decided, not invented (no fabrication, §9).
- **Doc follow-ups (intentionally not done here — "touch only phases"):** split `plan/02` I7 into the
  eight tab rows, and record these video-sourced facts in `plan/05` §12 (clean-room §9). Do before Verify.

## Build target vs Solo (locked decisions to honor)
| Area | Solo (video) | Soloist build target |
|------|--------------|----------------------|
| License header | "Licensed · Renews 1/29/2027" | **N/A — D3 licensing dropped.** No license row. |
| Hotkeys modifier | macOS `⌘`/`⌥` | **Remap to Ctrl/Super** per Phase 11 I6 / `plan/05` §10. |
| Integrations · MCP | "MCP Server · Port 45678" (TCP) | **D4: `soloist-mcp` over stdio (no TCP port).** Toggle governs the per-group MCP tool surface; show setup snippet, not a port. |
| Integrations · HTTP API | "HTTP API · Port 24678" | **Matches H1: `127.0.0.1:24678`.** Keep. |

## Settings inventory (every global setting — the contract)
All persist in the one `Settings` record (auto-save; Solo: "Most settings auto-save").

### Tab: Appearance
| Group | Setting | Control | Value range (video) |
|-------|---------|---------|---------------------|
| Application | Theme | segmented — "App color scheme" | Light / Dark / **System** |
| Application | Interface font scale | A·A·A… size picker — "Adjust the size of all UI elements" | discrete steps |
| Terminal | Focus on click | toggle — "Single-click the terminal to focus instead of double-click" | on/off |
| Terminal | Font family | dropdown — "Monospace fonts installed on your system" | e.g. JetBrains Mono |
| Terminal | Font weight | "Weight for regular terminal text" | 100–900 (400) |
| Terminal | Bold font weight | "Weight for bold terminal text" | 100–900 (600) |
| Terminal | Terminal font scale | A·A·A… — "Adjust the font size of the terminal" | discrete steps |
| Terminal | Line height | "Adjust the spacing between terminal lines" | 1.0–1.8 (1.1) |
| Terminal | Letter spacing | "Adjust the spacing between terminal characters" | 0.5–1.3 (0.9) |
| Terminal | *(live preview)* | "Terminal preview" sample render | — |

→ Theme + every terminal value must restyle the **xterm.js** theme/renderer (I5), not just CSS.

### Tab: Sidebar
| Group | Setting | Control | Value range (video) |
|-------|---------|---------|---------------------|
| Filter | Show filter input | toggle — "Show the filter input at the top of the sidebar." | on/off |
| Sections | Hide empty sections | toggle — "Hide sections with no processes (e.g. Agents, Terminals)." | on/off |
| Project headers | Project CPU usage | "Show when CPU reaches" | Always / 25 / 50 / 100 / 200% / Never |
| Project headers | Project memory usage | "Show when memory reaches" | Always / 500MB / 1 / 2 / 8GB / Never |
| Project headers | Open in editor | toggle — hover action on project header | on/off |
| Project headers | Open in terminal | toggle — hover action | on/off |
| Project headers | Reveal in Finder¹ | toggle — hover action | on/off |
| Process headers | Process CPU usage | "Show when CPU reaches" | Always / 10 / 30 / 60 / 90% / Never |
| Process headers | Process memory usage | "Show when memory reaches" | Always / 100 / 500MB / 1 / 2GB / Never |
| Footer | Show settings footer | toggle — "Display Settings button at the bottom of the sidebar. Still accessible via command palette and hotkey." | on/off |

¹ "Reveal in Finder" → Linux "Show in file manager" (don't copy macOS wording).

### Tab: Hotkeys
"KEYBOARD SHORTCUTS" — search box + **Reset all to defaults**. Help text: *"Click any hotkey to edit, or
hover and press x to disable it. Process and project shortcuts can share the same key since they activate
in different contexts. System shortcuts like copy, paste, and quit cannot be changed."* Bindings are
**scoped** (a `Sidebar` / `Terminal` badge per row). A full, remappable keymap — sections + representative
bindings seen (remap `⌘`→Ctrl/Super for us):

- **GENERAL** ("App-wide actions, palettes, and system shortcuts"): Open command palette (`⌘K`), Quick
  actions (`⌘P`), Quick jump (`⌘E`), New agent/terminal (`⌘T`), Open settings (`⌘,`), Open terminal search
  (`⌘F`), Close agent/terminal (`⌘W`).
- **SIDEBAR** (navigation): Next/Previous project group (`⌘↓`/`⌘↑`), Next/Previous section (`⌥↓`/`⌥↑`),
  Jump to Agents/Commands/Terminals (`⌥A`/`⌥C`/`⌥T`), Collapse / go to section (`←`), Jump to parent
  project (`⌘←`), Expand project (`→`), Restart (`R`).
- **TERMINAL** ("Shortcuts active while the terminal is focused"): Previous/Next process (`⌘↑`/`⌘↓`),
  Increase/Decrease terminal font size (`⌘=`/`⌘-`).

→ Implement as a registry of named actions with per-action **scope + binding**, conflict allowed across
scopes, system shortcuts non-editable. Persist overrides only (defaults stay code-defined, single-source).

### Tab: Agents
| Group | Setting | Control | Notes |
|-------|---------|---------|-------|
| Agent tools | tool rows | each: name + invocation, `[Edit]`, enable toggle | seen: Claude (`claude`), Codex (`codex`), Gemini (`gemini`), OpenCode (`opencode`), Claude Danger (`claude --dangerously-skip-permissions`) |
| Agent tools | `Detect` | button — probe installed agent CLIs | populates the registry |
| Agent tools | `Add tool` | button — add a custom agent definition | name + command |
| Auto-summarization | Summarizer tool | dropdown — "Generate a one-line summary when an agent or terminal becomes idle" | e.g. Claude |
| Auto-summarization | Model | text + `Save` — "Passed to Claude via its model flag" | e.g. `haiku` |

→ Reuse the **Phase 7 agent tool registry**. Auto-summarization stays **OFF by default** (locked decision,
§3); this tab is where it's opted in. Core must never hard-depend on an LLM.

### Tab: Tools
| Setting | Control | Notes |
|---------|---------|-------|
| Default editor | dropdown + config gear — "Used when opening projects. Can be overridden per-project." | resolves under the project editor override (11a) |
| Default terminal | dropdown + config gear — "Used when opening projects from the sidebar." | e.g. Ghostty → our Linux terminals |

### Tab: Integrations
| Integration | Control | Detail rows | Our target |
|-------------|---------|-------------|-----------|
| MCP Server | toggle (Solo shows "Port 45678") — "Allow AI assistants like Claude to control processes." | "Exposed MCP tools (16)", "Setup: CLI tools (Claude Code, Amp, OpenCode, Codex)", "Setup: IDEs & apps (Cursor, Windsurf, Cline, Claude Desktop)" | **stdio (no port), D4**; toggle + per-group enablement reuse `McpToolGroups`; show setup snippet |
| HTTP API | toggle ("Port 24678") — "Expose a REST API on localhost for Raycast and other tools." | "API Endpoints (15)" | **`127.0.0.1:24678`, H1**; endpoint list from Phase 10 |

### Tab: Notifications — **NOT SHOWN IN VIDEO**
Tab exists in the bar; never opened on camera. Likely the global counterpart of the per-project crash/exit
& terminal alerts (11a) plus delivery options (sound, OS notifications). **Decide from `plan/05` /
soloterm.com docs or ask the owner before building — do not invent fields.**

### Tab: Account — **NOT SHOWN IN VIDEO**
Tab exists; never opened. In Solo this is licensing/account; **N/A under D3.** Proposed Soloist use: app
info (version, data dir `SOLOIST_APP_DATA_DIR`), reset-to-defaults, export/import settings. **Needs an
explicit decision — do not fabricate.**

## Scope
**In:** the Settings window shell + all **shown** tabs (Appearance, Sidebar, Hotkeys, Agents, Tools,
Integrations); extend the `Settings` document + `Facade` methods + Tauri commands; auto-save; xterm.js
restyle from Appearance; hotkey remap registry. **Out:** Notifications/Account tab *contents* (blocked on a
decision — stub the tabs, don't guess); per-project settings (11a); packaging (12); parity walk (13).

## Tasks
1. **Extend the `Settings` document via the shared settings base** (`crates/core/src/settings.rs`; pattern
   + recipe `plan/06` §5.9) with one `#[serde(default)]` sub-struct per tab (`appearance`, `sidebar`,
   `hotkeys`, `agents`, `tools`, `integrations`), each a single source of truth with closed enums for
   discrete pickers (theme, thresholds). The global record is `SettingsStore<(), Settings>` — the **same**
   generic base Phase 11a uses for per-project (`K = ProjectId`), so neither surface re-rolls persistence.
   Keep `mcp_tool_groups` as the Integrations·MCP backing. Defaults match the video.
2. **`Facade` getters/setters per tab** routing through `SettingsStore` (one behavior, many frontends —
   the settings UI and any CLI/MCP read the same record). Thin pass-throughs, policy in the aggregate.
3. **Tauri commands + frontend** (`plan/06` §5.5): a `SettingsWindow` shell with a tab strip; one small
   presentational panel component per tab over a projected read-model; **no business logic in components**
   (§15–16). Auto-save on change; the Sidebar/Appearance settings drive the live dashboard projection.
4. **Appearance → xterm.js** (I5): theme + font family/weight/scale/line-height/letter-spacing apply to
   the terminal renderer and the app tokens together, through the Phase 0/11 token layer.
5. **Hotkeys registry** (I6): named actions with `(scope, binding, editable)`; defaults code-defined,
   only overrides persisted; search, per-row edit, hover-to-disable, **Reset all to defaults**; remap to
   Ctrl/Super; system shortcuts locked.
6. **Agents tab** wires the Phase 7 tool registry (Detect/Add/Edit/enable) and the summarization opt-in
   (tool + model), **OFF by default** (§3).
7. **Integrations tab** reflects real state: MCP enablement (per-group `McpToolGroups`) + stdio setup
   snippet (D4); HTTP API toggle + endpoint list (H1/Phase 10). No fake ports.
8. **Stub Notifications & Account tabs** with an explicit "to be defined" placeholder *in the UI only*
   (not invented settings) until the owner decides; do not persist guessed fields.

## Acceptance criteria
- Every **shown** field persists in the singleton `Settings` record and survives restart; auto-save works.
- Switching Theme (incl. System) and changing any terminal typography value restyles the app **and**
  xterm.js immediately and after restart.
- A remapped hotkey takes effect and survives restart; "Reset all to defaults" restores code defaults;
  system shortcuts cannot be changed; same key in different scopes does not conflict.
- Toggling MCP enablement / a feature group changes what the `soloist-mcp` server exposes (reuses the
  landed `McpToolGroups` behavior); HTTP API toggle reflects the Phase 10 server.
- Notifications/Account tabs render a "to be defined" state and persist **no** invented fields.
- Adding `Settings` fields keeps old stored records readable (serde-default back-compat test).

## Test plan
- **Playwright:** set each shown field → reload → persists; theme/system restyles xterm; hotkey remap +
  reset + scope-conflict; MCP group toggle changes exposed tools; Notifications/Account show the stub.
- **Integration (core):** `Settings` document serde-default back-compat after adding fields; `Facade`
  getter/setter round-trips through `SettingsStore`; Integrations·MCP reads route to `McpToolGroups`.

## Risks & mitigations
- **Inventing un-shown settings** → Notifications/Account stay stubbed until a documented decision; cite
  `plan/05`/soloterm docs, never guess (§9).
- **Copying macOS wording/shortcuts** (clean-room §9) → relabel "Reveal in Finder", remap `⌘`→Ctrl/Super,
  no Solo assets.
- **Document bloat / migrations** → one singleton record extended with serde-default fields (no new table
  per tab); a single migration bump if a column is needed.
- **xterm restyle perf** → apply terminal options once per change, coalesced; honor the 60fps budget (§6).

## Effort
~5–7 days (Hotkeys registry + Appearance↔xterm wiring are the largest; Tools/Integrations are thin once
the `Settings` document is extended).
