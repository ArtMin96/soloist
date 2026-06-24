# 05 — Solo Reference & Sources (ground truth)

This is the **canonical, cited record** of how the real Solo behaves, gathered from soloterm.com's
public docs (a 183-URL sitemap), blog, comparison pages, changelog, and one independent hands-on
review. Every phase cites this file instead of re-deriving behavior. Facts are marked:

- ✅ **Documented** — stated on an official `/docs` page (URL given).
- 🟡 **Stated elsewhere** — marketing/blog/changelog/review (less precise).
- ❓ **Gap** — not publicly documented; a design decision for us (flagged where it lands).

> Honesty note: nobody on our side has run the macOS app or read its source (it's closed). The MCP
> tool *names* are documented; their exact JSON parameter schemas are **not** — those are ❓ and we
> design our own. We never copy Solo code or assets.

---

## 1. The product model — "metaharness"

✅🟡 Solo is **not** a coding agent, **not** a terminal emulator, **not** a git-worktree orchestrator.
It is a **process-supervision + coordination layer** that runs the agent CLIs you already use as
ordinary managed processes and gives them a shared, project-scoped workspace over MCP.
Source: [blog/the-agentic-metaharness](https://soloterm.com/blog/the-agentic-metaharness),
[alternatives/conductor](https://soloterm.com/alternatives/conductor),
[alternatives/cmux](https://soloterm.com/alternatives/cmux).

The six things the metaharness owns (verbatim, blog): a **process graph**; **readable output/status**;
**scratchpads**; **todos/blockers/comments/locks/key-value**; **timers/idle-watchers**;
**notifications/restart behavior**. This list is effectively our feature spec.

🟡 Explicitly **no** parallel isolated branches, **no** worktrees, **no** sandboxes/containers. Agents
are **sibling processes in one shared workspace** that coordinate cooperatively.

---

## 2. Process model — three subtypes

✅ "Process" is the umbrella; three subtypes. Source:
[docs/getting-started/concepts](https://soloterm.com/docs/getting-started/concepts).

| Subtype | What | Lifecycle traits |
|---------|------|------------------|
| **Command** | named shell command (`npm run dev`, a worker) | trust-gated; auto-start; auto-restart; file-watch; from `solo.yml` **or** local app state |
| **Agent** | an AI CLI in an interactive terminal (Claude Code, Codex, Gemini, Amp, OpenCode…) | has **activity/attention state** (idle detection, §6); "Resume last session" |
| **Terminal** | a plain interactive shell | freeform typing |

✅ A **project** is a filesystem folder (repo/workspace root) that (1) sets the working dir, (2)
watches/syncs `solo.yml`, (3) auto-detects project type to suggest commands. Free tier (not relevant to
us — D3): 4 projects / 20 processes.

---

## 3. `solo.yml` — exact schema

✅ Source: [docs/projects/solo-yml](https://soloterm.com/docs/projects/solo-yml). Verbatim example:

```yaml
name: storefront                 # optional: display name on first load
icon: assets/project-icon.png    # optional: image path relative to project root
processes:                       # REQUIRED: a MAP keyed by process name (not a list)
  Web:
    command: npm run dev         # required: shell command
    working_dir: null            # optional: path relative to project root
    auto_start: false            # optional (default ❓ — see gap below)
    auto_restart: false          # optional, default false
    restart_when_changed: []     # optional: glob list → file-watch restart
    env: {}                      # optional: env key/values
```

- Top-level keys: `name`, `icon`, `processes`. Per-process: `command`, `working_dir`, `auto_start`,
  `auto_restart`, `restart_when_changed`, `env`.
- ✅ File size **limited to 1 MB**. Empty/comment-only file = empty config (valid).
- ✅ **Rename detection:** a rename is recognized as an unambiguous remove/add pair with the **same
  command string**; the row (and its trust) is preserved.
- ❓ **`auto_start` default is NOT authoritatively documented.** The example shows `false`; one summary
  inferred `true`. **Decision for us:** default `auto_start: true` (matches "auto-starts your stack"
  marketing) but make it explicit in our schema docs. Do not treat Solo's default as known.
- ❓ **No `agents:` block, no global `env:`, no readiness field in YAML.** Agents are processes/spawned
  via execution profiles; readiness is runtime-only (§7 `wait_for_bound_port`). `env` layering
  precedence over captured shell env (§5) is undocumented — we define it (§ decisions).
- ❌ **NOT in YAML (lives in app state):** local (non-shared) processes, trust state, agent tool
  definitions, window/layout. `solo.yml` carries only shared command definitions.

> ⚠️ This corrects the earlier draft, which modeled `processes` as a list with `cwd`/`restart`/`ready`/
> `visibility`. The real schema is the table above.

---

## 4. Lifecycle, trust & sync

### Start / stop / restart
✅ Source: [docs/commands/start-stop-restart](https://soloterm.com/docs/commands/start-stop-restart).
- **Start** only if the command is **trusted** and within limits.
- **Stop** stops the process, **releases any todo locks held by that process**, and removes it from
  crash-recovery tracking.
- **Restart** = stop + start with the latest saved command config and terminal size. Untrusted →
  blocked.
- ❓ SIGTERM-vs-SIGKILL ordering / grace period **not documented** (MCP calls stop "graceful"). We
  define: SIGTERM to the process group → grace window → SIGKILL (§ Phase 3 / architecture).

### Auto-start
✅ Source: [docs/commands/auto-start](https://soloterm.com/docs/commands/auto-start). Auto-starts only
when ALL true: it's a **command** (not terminal/agent); `auto_start` enabled; **trusted**; limits
allow. Untrusted + auto-start → reported **blocked**, not run.

### Crash auto-restart
✅ Source: [docs/commands/auto-restart](https://soloterm.com/docs/commands/auto-restart).
- Relaunches a **trusted** command after unexpected exit; keeps last crash output + shows a **restart
  banner** before new output.
- **Rate limit (concrete):** after **10 restarts in a 60-second window**, auto-restart **pauses** for
  that command and shows an "exhausted" indicator. (No exponential backoff documented — just this gate.)
- Disabled during app shutdown. Untrusted never auto-restarts.

### File-watch auto-restart
✅ Source: [docs/commands/file-watch-auto-restart](https://soloterm.com/docs/commands/file-watch-auto-restart).
- Globs in `restart_when_changed`, evaluated **relative to project root**; `*` **matches across path
  separators**; recommend explicit patterns (`src/**/*.ts`, `config/**`, `**/*.go`).
- Watches project dir **recursively** for **create + modify**; events **debounced/coalesced** into a
  quiet window, then a **full restart cycle** + emits a file-restart event.
- **Command-only, trusted-only.** Empty/invalid glob list → no watcher created.
- Independent from crash auto-restart (separate rate-limit).
- ❓ No documented ignore-list (`.git`/`node_modules`). We add sensible default ignores.

### Orphaned processes
✅ Source: [docs/commands/orphaned-processes](https://soloterm.com/docs/commands/orphaned-processes).
On restart after crash/force-quit, Solo prunes stale orphan records and **adopts** running orphans only
when project path + process name + command config all match; otherwise a dialog offers **Kill / Kill
All / Leave running**. Historical output from the disconnected window may be unrecoverable.

### Trust / security
✅ Source: [docs/commands/trust-security](https://soloterm.com/docs/commands/trust-security),
[docs/projects/yml-change-notifications](https://soloterm.com/docs/projects/yml-change-notifications).
- Untrusted command → **manual start, auto-start, restart, file-watch, AND crash auto-restart are all
  blocked.**
- Trust is **local to the machine**, scoped to **project + remembered command variant**. Renaming can
  preserve trust; changing **command string / working_dir / env** invalidates it.
- "**Automatically trust command changes**" setting re-trusts on sync **only** when the sync came from a
  user action that creates/saves the command.
- **Sync:** debounces FS events, compares **file hashes**; a sync may add/update/remove commands and
  preserves rows on unambiguous renames; re-trust required after changes to command / working_dir /
  auto-start / auto-restart / watch / env. **Sync updates config only — it does not auto-start or
  restart anything.**

---

## 5. Shell environment & PATH
✅ Source: [docs/environment/shell-environment](https://soloterm.com/docs/environment/shell-environment).
- Shell resolved via `$SHELL` → passwd entry → `/bin/sh` fallback.
- Solo runs the shell as **`-ilc env`** (interactive login), parses output, **caches 10 minutes** — so
  version managers (nvm/rbenv/etc.) are visible. Changing startup files doesn't affect already-running
  children.
- On capture failure: falls back to the app's env and prepends common Homebrew paths. (On Linux we'd
  prepend `~/.local/bin`, `/usr/local/bin`, etc.)
- ❓ Precedence of `solo.yml` `env` over captured env undocumented → we define: per-process `env`
  overrides captured shell env, which overrides app env.

---

## 6. Agents: tools, launching, idle detection
✅ Sources: [docs/agents/setting-up-tools](https://soloterm.com/docs/agents/setting-up-tools),
[launching-agents](https://soloterm.com/docs/agents/launching-agents),
[idle-detection](https://soloterm.com/docs/agents/idle-detection),
[auto-summarization](https://soloterm.com/docs/agents/auto-summarization).

- **Built-in tool types:** Claude, Codex, Amp, Gemini, OpenCode, Generic (+ Copilot CLI, Kimi CLI in
  v0.7.1). Solo does **not** install the CLIs. Per-tool config: Name, Command, Default arguments
  (appended every launch), Tool-type mode (auto-detect/manual), Prompt mode for generic (`stdin` or
  appended arg). **Auto-detect probes `--version`** for `claude`, `codex`, `amp`, `gemini`, `opencode`.
  - 🟡 **CLI commands for the two extra built-in types (our grounding — Solo names the *type*, not the
    binary).** Copilot CLI = `copilot` (npm `@github/copilot`, GA 2026-02; `--version` confirmed); Kimi
    CLI = `kimi` (MoonshotAI `kimi-cli`). Grounded by web search, not Solo docs. They are seeded as
    launchable built-in tools but stay **outside** the `--version` auto-detect set above (Solo documents
    probing only the five; we do not invent that it probes Copilot/Kimi, which also sidesteps the
    unconfirmed `kimi --version`).
- **Launching:** `Cmd+T` picker; right-click → Add agent; "Agent with flags" modal to edit flags for a
  single launch. Agents launch in the selected project's dir; many concurrently.
- **Idle detection (5 states): `IDLE`, `PERMISSION`, `THINKING`, `WORKING`, `ERROR`.** Heuristics differ
  per runtime: Claude/OpenCode use visible output; Codex/Amp watch **OSC title stability**; Gemini
  tracks OSC title status.
- **Auto-summarization:** sends a **compact rendered-text snapshot** (not full transcript) to a
  summarizer; Claude/Codex/Gemini use **native headless** invocations; default models `sonnet`,
  `gpt-5-codex`, `flash-lite`; cadence 15s / 30s / 1min. Caveat: "a quiet terminal is not always
  completed work."
- **Agent authentication — Solo manages NONE of it (🟡, [agents](https://soloterm.com/agents)).** An
  agent is just its CLI run as a process; the CLI keeps using whatever auth the user already configured
  on the machine. Verbatim: *"Your agents keep using the accounts, API keys, and subscriptions you
  already configured locally. Solo does not farm OAuth tokens or route your work through a vendor
  account."* and *"Solo does not need to impersonate you, collect provider OAuth tokens, or sit between
  you and the agent account you pay for."* So Solo never stores, prompts for, or injects API keys / OAuth
  tokens — Claude Code's **own** native login (browser/loopback OAuth, or paste-code fallback) runs in
  the agent's interactive terminal, and the CLI persists its own credentials (Claude Code: plaintext
  `~/.claude/.credentials.json` on Linux, mode 0600 — its file, not ours; per
  [code.claude.com/docs/en/authentication](https://code.claude.com/docs/en/authentication)).
- ⚠️ Design implication: idle detection drives **timers** (fire-when-idle) and **notifications**.
  Auto-summarization needs an LLM → for our clone it must be **optional/configurable** (use the user's
  own agent CLI in headless mode or disable). Don't hard-require a cloud model.
- ⚠️ Auth design implication for **Soloist**: because we run the agent **directly on the host** (no
  sandbox/container — `00-vision-and-scope.md`), the CLI's credentials already live where it looks; we
  inherit Solo's stance exactly — **manage no agent credentials**. The only requirements are ours
  already-built terminal substrate: launch the agent **interactively** in a real PTY (never headless
  `-p` for the main agent process) and pass the env through (`$DISPLAY`/`BROWSER` for the browser step,
  any `ANTHROPIC_*` the user set) so the native login completes in-terminal. (Contrast: tools that run
  the agent in an isolated box — e.g. `madarco/agentbox` — must *stage/forward* host credentials into
  the box; that whole apparatus is N/A for our local-execution model. Researched 2026-06-18.)

---

## 7. MCP server — the integration surface
✅ Sources: [docs/integrations/mcp-server](https://soloterm.com/docs/integrations/mcp-server),
[docs/mcp-tools/overview](https://soloterm.com/docs/mcp-tools/overview) + per-category pages.

- **Transport: stdio only.** "No public MCP host or port for normal clients." Clients launch Solo's
  bundled **`mcp` helper** binary. Solo generates setup snippets for Claude Code, Cursor, Windsurf,
  Cline, Claude Desktop. Non-default data dir → snippet includes `SOLOTERM_APP_DATA_DIR`.
- **Identity & scope:** tools act on an **effective project scope** set by `select_project` or inferred
  from the MCP session / bound process. Solo-launched agents auto-bind via **`bind_session_process`**
  using the **`SOLO_PROCESS_ID`** env var Solo injects. External callers use **`register_agent`**.
  **`whoami`** reports the resolved process/actor/scope. Binding ties **timers, locks, todo-locks,
  scratchpad activity, and cleanup** to the right process (locks auto-release when the bound process
  closes).
- **Tool gating:** core groups always on when MCP enabled (Project, Services, Process, Bulk, Output,
  Agent/Terminal, Coordination, Setup/Support). Feature groups have toggles: Scratchpads, Todos, Timers
  inherit; **Key-Value defaults OFF**. v0.8.2 added optional **prompt-template** tools.

### Full tool catalog (names ✅ documented; param schemas ❓ ours to design)

- **Project:** `list_projects`, `select_project`, `get_project_status`, `get_project_stats` (CPU/mem).
- **Services:** `services_list`, `wait_for_bound_port`.
- **Process:** `list_processes`, `get_process_status`, `rename_process`, `select_process`,
  `start_process`, `stop_process`, `restart_process`, `send_input` (text or raw control bytes; optional
  `wait_ms` returns rendered tail), `close_process`.
- **Bulk:** `start_all_commands`, `stop_all_commands`, `restart_all_commands` (trusted commands only).
- **Output:** `get_process_output` (rendered), `get_process_raw_output` (with control sequences),
  `search_output`, `search_raw_output`, `clear_output` (buffer only, not PTY), `flush_terminal_perf`,
  `get_process_ports` (detected localhost ports/URLs — readiness/discovery).
- **Agent/Terminal:** `list_agent_tools`, `spawn_process`, `spawn_agent` (alias, v0.7.1),
  `bind_session_process`, `whoami`.
- **Coordination:** `register_agent`, `lock_acquire`, `lock_status`, `lock_release` (project-scoped
  **lease** locks — "signals, not ownership"; ❓ TTL/renewal undocumented).
- **Scratchpads (~14–18):** `scratchpad_list`, `_tags_list`, `_read`, `_write` (**revision-guarded**),
  `_rename`, `_add_tags`, `_remove_tags`, `_append`, `_clear`, `_delete`, `_archive`, `_transfer`,
  `_save_to_file`, `_load_from_file` (+ v0.7.1 `_edit`, `_append_section`, `_tail`, `_find`). Leading H1
  is the title; revisions prevent clobbering newer edits.
- **Todos (~19):** `todo_create`, `_list`, `_tags_list`, `_get`, `_update`, `_add_tag`, `_remove_tag`,
  `_transfer`, `_set_blockers`, `_add_blocker`, `_remove_blocker`, `_complete`, `_lock`, `_unlock`,
  `_delete`, `_comment_create`, `_comment_update`, `_comment_delete`, `_comment_list`. Process-owned
  locks release when the bound process closes.
- **Timers (7):** `timer_set` (on fire, **`body` is delivered verbatim to the owning agent as a fresh
  user turn**), `timer_fire_when_idle_any`, `timer_fire_when_idle_all`, `timer_cancel`, `timer_pause`,
  `timer_resume`, `timer_list`. Require a bound owning actor. Responses include `already_idle`,
  `waiting_on`.
- **Key-Value (4, default off):** `kv_set`, `kv_get`, `kv_delete`, `kv_list` (project-scoped **JSON**;
  "small structured state, not logs/long text").
- **Setup/Support:** `help`, `submit_solo_feedback`, `setup_agent_integration` (writes Solo MCP docs
  into `AGENTS.md` / `CLAUDE.md`).

---

## 8. Local HTTP API + `solo` CLI
✅ Source: [docs/integrations/raycast-http-api](https://soloterm.com/docs/integrations/raycast-http-api),
changelog.
- Binds **`127.0.0.1:24678`** (port editable only while disabled; auto-fallback if taken). Mutations
  require header **`X-Solo-Local-Auth: 1`**. CORS limited to localhost.
- **Read:** `GET /health`, `/status`, `/processes`, `/processes/:id/ports`, `/projects`.
- **Mutate:** `POST /processes/:id/start|stop|restart`; `POST /projects/:id/start-all|stop-all|reload|
  start-auto|restart-running|restart-all`; `POST /focus`.
- 🟡 A **`solo` CLI exists** and talks to this local API (v0.7.1+): version/status, process
  list/get/start/stop/restart/rename/spawn, recent output, filter by status, delete processes, todo/
  scratchpad workflows. (So the CLI is a thin HTTP client, not a separate engine.)

---

## 9. Command auto-detection
✅ Source: [docs/projects/command-auto-detection](https://soloterm.com/docs/projects/command-auto-detection).
Runs **only on initial project add when no `solo.yml` exists**. Reads project-root files (+ targeted
subdir checks like Phoenix `assets/package.json`, root `.csproj`). Sources: package.json scripts
(prioritizes `dev`/`start`/`serve`/`build`/`test`; detects Next/Nuxt/Prisma), Procfile, Make/Just/Task,
PM2 ecosystem, turbo.json, nx.json, plus Laravel/Rust/Spring/FastAPI/Flask/Django/Rails/Go/.NET/
Phoenix/Docker Compose. Dev servers pre-selected for auto-start/auto-restart; build/test offered
unchecked.

> **Soloist scope:** matrix **A10** is **v1** (pulled in by user decision 2026-06-19). When a picked
> folder has no `solo.yml`, Soloist auto-detects from project-root files and **auto-creates** a
> `solo.yml` (it never rewrites an existing one — §3); detected dev/start/serve commands get
> `auto_start`, build/test do not; nothing detected → a clean starter file. Detected commands register
> trust-gated — auto-creation never bypasses the trust gate (§4).

---

## 10. UI surface (from changelog + review; imagery unverified)
🟡 Sources: [changelog](https://soloterm.com/changelog),
[eshlox.net review](https://eshlox.net/solo-changed-how-i-work-with-terminals), comparison pages.
- **Left sidebar = process tree**, grouped into collapsible subgroups **Agents / Terminals / Commands**
  (collapse state per project), drag-reorder, nested child agents, optional subprocess counts.
- **Per-row status:** running/crashed indicator; **green = running, red = crashed**; CPU/mem; agent
  working/idle/permission/error.
- **Main pane:** selected process's interactive PTY (full ANSI, **GPU renderer** since v0.6.0); stopped
  process shows an in-pane **Start** (or "Resume last session" for agents).
- **Attention bell** in title bar; **unified unread** across sidebar/title bar/dock badge; clicking a
  crash notification opens that terminal.
- **Command palette** `Cmd+K`, quick actions `Cmd+P`, **jump** `Cmd+E`, attention-jump `Cmd+Shift+E`,
  new item `Cmd+T`. Deep links **`solo://`** to projects/processes/todos/scratchpads.
- **Activity Monitor view** (🟡 changelog v0.6.1): a dedicated **cross-project** view of running
  commands/terminals/agents **and tracked subprocesses** — project/type/status/ports filters, **flat or
  tree** layout, **sortable** CPU/mem/descendant-port columns, and quick subprocess actions. Distinct
  from the per-project sidebar tree. Source: [changelog](https://soloterm.com/changelog).
- **Prompt templates** (🟡 changelog v0.8.2): a dedicated view to create/edit/search/filter/duplicate
  reusable prompts, moved between **global and project** scope, with **placeholder** fill-in before a
  prompt is applied (also exposed as optional MCP tools, §7). Source:
  [changelog](https://soloterm.com/changelog).
- **Scratchpads & Todos panels** with Markdown editors, search/filter/sort/archive, checkbox task lists,
  "terminal selection → scratchpad".
- **Trust review screen** showing command + working dir + env before approval; "Trust all commands".
- **First launch** shows a **guided demo project**.
- ⚠️ **Limitation:** closing Solo **stops all processes** — no detached/background persistence (unlike
  tmux).

### Keyboard shortcuts (macOS Cmd-based; we remap to Ctrl/Super on Linux)
✅ [docs/keyboard-shortcuts/default-reference](https://soloterm.com/docs/keyboard-shortcuts/default-reference):
`Cmd+K` palette, `Cmd+P` quick actions, `Cmd+E` jump, `Cmd+Shift+E` attention jump, `Cmd+T` new,
`Cmd+W` close, `Cmd+,` settings, `Cmd+F` terminal search, `Cmd+Left/Right` focus sidebar/terminal,
`Cmd+[`/`]` history, font zoom (`Cmd+=`/`-` terminal, `Cmd+Shift+=`/`-` app), `Option+1–9` project,
`Cmd+1–9` process; sidebar arrows; header keys S/A/P/R (start-auto/all, stop-all, restart-running);
row keys S/R/C.

### Settings tabs
✅ [docs/settings/overview](https://soloterm.com/docs/settings/overview): Appearance, Terminal,
Notifications, Sidebar, Notes & todos, Hotkeys, Agents, Tools (default editor/terminal), MCP, Account.
Most settings auto-save.

---

## 11. Distribution & versions
🟡 Sources: [download](https://soloterm.com/download), [changelog](https://soloterm.com/changelog),
[homepage](https://soloterm.com/).
- macOS universal `.dmg`, "signed & notarized", ~64.9 MB download (25 MB marketing). **Tauri**, system
  WebKit. Min macOS 11. Windows/Linux "coming soon" (Ubuntu 20.04+ target).
- In-app **"Check for updates"**; updater backend unnamed (Tauri updater inferred ❓).
- Latest **v0.8.2 (2026-06-05)**; fast cadence. v0.8.2 added **execution profiles** (project-level,
  incl. Windows/WSL), **prompt templates**, "Resume last session". v0.6.0 added the **GPU renderer**.
- 🟡 `SOLOTERM_APP_DATA_DIR` env names the app data dir. ❓ On-disk storage format for todos/
  scratchpads/KV/locks/trust is undocumented (SQLite likely).

---

## 12. Confirmed gaps → our explicit decisions
Each gap is something Solo does NOT publicly document; we choose and own the answer (cross-ref
`03-tech-stack-and-decisions.md` and `04-engineering-architecture-and-patterns.md`):

| Gap | Our decision (default) |
|-----|------------------------|
| `auto_start` default | `true`, documented explicitly |
| Stop signal semantics | SIGTERM to process group → 5s grace → SIGKILL |
| `env` precedence | process `env` > captured `-ilc` shell env > app env |
| Shell-environment capture (I10) | Solo documents capturing `$SHELL -ilc env`, caching it 10 minutes, and falling back to the app env with common bin dirs prepended; the mechanics are ours. The capture runs `$SHELL -ilc 'env -0'` (the shell resolved as `$SHELL` → passwd entry → `/bin/sh`, matching the spawner) and parses the **NUL-delimited** output so a value with `=` or newlines is unambiguous, keeping only entries with a valid variable name and dropping the capturing shell's own session bookkeeping (`PWD`/`OLDPWD`/`SHLVL`/`_`) so it cannot mislead a child. The capture is bounded by a **3 s timeout** (output drained on a thread so a large env cannot wedge the pipe; a hung shell is killed and reaped). The pure resolver (core) holds a **single, global, ~10-minute cache** (one capture per window regardless of project, single-flighted so a burst of starts triggers one shell; a success is cached, a **failure is not** so the next spawn retries) and layers the captured env under the process's own `env` overrides; the spawner inherits this process's app env as the base, giving the documented precedence (process > captured > app). On capture failure the resolver contributes only a `PATH` override that prepends `~/.local/bin` and `/usr/local/bin` (the `~` expanded against the app `HOME`) to the app `PATH`. The environment is resolved at **each spawn** (the actor's single spawn chokepoint), so a restart picks up a refreshed capture. Ours (clean-room) |
| File-watch ignores | default-ignore `.git`, `node_modules`, `target`, `dist`, `.venv` |
| File-watch on a non-running command | reload a *running* command only; a change while it is stopped/crashed/exhausted does nothing (never resurrects a user-stopped or restored-resting command) |
| Restart banner scope | Solo documents "keep last crash output + restart banner before new output" for *crash* auto-restart. We retain the terminal scrollback and draw the banner on **every** relaunch of a process — crash auto-restart, file-watch restart, manual restart, and a user start after a stop — since the boundary is equally useful and a uniform rule avoids special-casing one trigger. The banner is neutral (`restarted`): the injection point in the terminal stream does not know the cause |
| Lease lock TTL | explicit TTL + renew; auto-release on bound-process close. Ours: a lease is project-scoped and owned by the caller's **bound process** (`SOLOIST_PROCESS_ID`); a requested TTL is **bounded to 1 second … 1 hour** (the floor keeps a just-acquired lease briefly live; the ceiling, per the longevity rules, means holding longer requires renewing), with the **default and the bounds defined once in the core** so every frontend shares them; re-acquiring a key you already hold **renews** it; expiry is compared against a persistable wall clock (a new `Clock::now_unix_millis`), applied **lazily** on the next read. Auto-release is three ways: explicit `lock_release`, TTL expiry, or the owning process closing (the supervisor's `LockReleaser` hook). Because process ids are minted per run (the counter restarts each launch), a persisted lease can never be matched safely to a later run's processes — so **launch reconciliation clears every lease** (nothing from a fresh run holds one yet); leases do not meaningfully survive a restart, unlike the content aggregates (todos/scratchpads/kv) |
| MCP `lock_acquire` | acquires the project-scoped lease `key` for the caller's bound process; **non-blocking** ("signals, not ownership") — a key another process holds returns that holder (`outcome: "held"`) rather than waiting. The acquire is **atomic** (one conditional store write), so two processes racing for a free key cannot both be granted it — exactly one wins and the other is told the holder. An unbound session (no `SOLOIST_PROCESS_ID`) is refused, since there would be no owner to auto-release it on close. `ttl_ms` defaults when omitted (the default lives in the core). Solo documents the tool name; the param schema and semantics are ours |
| MCP `lock_status` | reports the current holder of the project-scoped lease `key`, or none if it is free or has expired (pruning an expired row). A read — needs the project scope but not a bound process |
| MCP `lock_release` | releases the project-scoped lease `key` only if the caller's bound process holds it (returns whether it did); a caller cannot release a lease another process holds (owner-close handles those). Ours |
| Timer fire & delivery | a timer is **owned by the caller's bound process** (`SOLOIST_PROCESS_ID`). On fire it delivers its stored `body` to that process as a **fresh user turn**: the body verbatim followed by a carriage return so an agent CLI submits it. Firing is **one-shot** (a fired timer is removed). Delivery is best-effort — if the owner has since closed, the (already-claimed) timer is simply not delivered. The scheduler is a self-supervised, `Clock`-driven loop that claims each due timer **atomically**, so a concurrent pause/cancel cannot race it into firing a suspended timer. Solo documents the tool names and "delivered verbatim … as a fresh user turn"; the carriage-return submission, one-shot, and best-effort owner-gone handling are ours |
| MCP `timer_set` | arms a plain timer that fires after a relative `after_ms` (turned into an absolute persistable deadline in the core). Omitting `after_ms` (or zero) means **fire as soon as possible**; the delay is **bounded to a 24-hour ceiling** (longevity). Needs a bound process (the delivery target). Param schema + bounds ours |
| MCP `timer_fire_when_idle_any` / `_all` | fires when **any** / **all** of the watched `processes` are idle, or a **max-wait backstop** elapses (default **1 hour**, ceiling 24 h — so a stuck worker can never block the timer forever). "Idle" is the C4 idle FSM's `Idle` state, consumed by the scheduler as `AgentActivityChanged` events. A watched process that has **left the registry** counts as idle (it can no longer work — avoids deadlock); a running-but-unclassified or non-agent watched process is **not** idle (the backstop still fires it). The response reports `already_idle` (condition met at set time) and `waiting_on` (the watched processes not yet idle). Watched processes **need not be in scope**: a timer only ever delivers to its own owner, and idle state is already open through the read tools, so watching observes nothing new — only the owner (delivery target) is bound/scope-authenticated. Ours (clean-room) |
| MCP `timer_cancel` / `_pause` / `_resume` / `_list` | management of the caller's own timers, **scoped to the bound process** (a caller manages only timers it owns; `cancel`/`pause`/`resume` return whether they affected one, `list` returns the owner's timers). **Pause freezes the time that remains** until the deadline; **resume re-arms** the timer with that remaining time (so a paused-then-resumed timer is not instantly overdue). A paused timer never fires. Ours |
| Timer durability vs reconcile | timers persist in **SQLite** (migration v5) like the other aggregates, but — exactly like leases — a timer is **process-owned** and per-run process ids are recycled, so a timer left by a previous run names an owner that no longer exists and could never be delivered. **Launch reconciliation clears every timer.** So G11's "coordination state survives an app restart" is satisfied by the **content** aggregates (todos / scratchpads / key-value, still to build), not by the process-owned timers and leases, which do not meaningfully outlive the run that created them |
| Scratchpad discipline & identity (G1) | A scratchpad is a **disciplined, typed document**, not free-form Markdown: `ScratchpadDoc { objective, context, plan[], acceptance_criteria[], risks[], status, notes? }`, defined once in the core, validated on write (no required field blank; the three lists each need ≥1 non-blank entry), and rendered to one canonical Markdown layout (H1 = the scratchpad's name). This is a deliberate divergence from Solo's free-form note (per the project owner) — see `KNOWN-DIVERGENCES.md` D-7. Identity is a **durable, store-assigned `ScratchpadId`** (stable across rename and restart, migration v6) addressed by a unique **`name`** handle per project; scratchpads are project-scoped shared content (not process-owned) and **survive a restart** (G11), so launch reconciliation never clears them. Ours |
| MCP `scratchpad_write` (G1/G2) | creates or replaces `(project, name)` with the disciplined document, **revision-guarded** (optimistic concurrency, G2): `expected_revision` omitted = create (refused if one exists), or the current revision = update (refused on mismatch, bumping to `expected+1`). The check-and-write is one atomic store step. A malformed document is refused (`InvalidScratchpad`); a stale revision is `RevisionConflict { expected, actual }` (actual `None` when absent). Returns the written document at its new revision plus its canonical rendering. Solo documents the tool name; the typed schema, validation, and revision semantics are ours |
| MCP `scratchpad_read`/`_list` | `read` returns one scratchpad by `name` — the structured document, its tags/archived/revision, and the canonical Markdown rendering — or `UnknownScratchpad`. `list` returns every scratchpad in the effective project as one-line summaries (name, tags, revision, archived, objective gist). Solo's free-form read modes (full/headings/section) are unneeded against a structured document; reading a single section is a client-side field access. Ours |
| MCP `scratchpad_rename`/`_add_tags`/`_remove_tags`/`_tags_list`/`_archive`/`_delete` | management of a scratchpad within the effective project: `rename` changes the `name` handle (durable id unchanged; `ScratchpadNameTaken` if the target is used, `UnknownScratchpad` if absent); `add_tags`/`remove_tags` are atomic read-modify-write of the tag set (idempotent), returning the updated document; `tags_list` is the distinct tags across the project; `archive` toggles a listing flag (keeps the document); `delete` removes it. All project-scoped, ungated by trust (content, not execution). Ours |
| MCP scratchpad deferrals | the free-form-oriented tools Solo lists (`_append`/`_edit`/`_append_section`/`_tail`/`_find`/`_clear`) do not map cleanly onto the disciplined document and are **tracked deferrals**; cross-project `_transfer` and the host file-io tools (`_save_to_file`/`_load_from_file`) are **deferred to a focused follow-up** (file-io needs a project-root-scoping security pass before it touches the host filesystem from an MCP tool). Not in the G1/G2 slice; recorded so nothing is lost |
| Todo discipline & identity (G3) | A todo is a **disciplined, typed document**, not a free-form item: `TodoDoc { title, description, acceptance_criteria[], risks[], status }` (the same enforced-schema directive as scratchpads — see `KNOWN-DIVERGENCES.md` D-8), validated on write (`title`/`description` non-blank; `acceptance_criteria`/`risks` each ≥1 non-blank → `InvalidTodo`). Around it sit live columns mutated by dedicated atomic ops — tags, blockers, comments, and a process-owned lock — kept out of the revision-guarded document so a tag/comment change never collides with a specification edit. Identity is a **durable, store-assigned `TodoId`** (stable across runs, so a sibling todo can name it as a blocker; migration v7). Todos are durable shared content that **survives a restart** (G11); only their process-owned lock is cleared on launch. Ours |
| MCP `todo_create`/`_update`/`_get`/`_list`/`_delete` (G3) | `create` makes a todo from the disciplined doc at revision 1; `update` replaces the doc **revision-guarded** (`expected_revision`; mismatch → `TodoRevisionConflict`, like scratchpads G2); `get`/`list` return the read model (`list` as one-line summaries incl. a derived `blocked` flag); `delete` removes it. Project-scoped, ungated by trust (content, not execution) — an external single-project caller can use them without binding a process. Solo documents the tool names; the typed schema and revision semantics are ours |
| MCP `todo_complete` + the blocker gate (G4) | `status` (`Open`/`Blocked`/`InProgress`/`Done`) is the label an agent declares; the **gate** is the todo's unmet **blockers**. `todo_complete` (and an `update` to `Done`) is refused with `TodoBlocked { by }` while any blocker still exists and is not done — so a todo "stays gated until its blockers complete" (G4 Verify). A **deleted** blocker counts as met (never deadlocks). The gate lives in the blocker set, not the status label, so there is one source of truth for blocked-ness. Ours (clean-room) |
| MCP `todo_set_blockers`/`_add_blocker`/`_remove_blocker`/`_add_tag`/`_remove_tag`/`_tags_list` (G4) | atomic read-modify-write of a todo's live columns: setting/adding a blocker validates it exists in the project (`UnknownBlocker`) and is not the todo itself (`SelfBlocker`); removing is idempotent. Tags add/remove are idempotent (sorted set); `tags_list` is the distinct tags across the project. All project-scoped, ungated by trust. Ours |
| MCP `todo_comment_create`/`_update`/`_delete`/`_list` (G4) | comments are project-scoped content (no author attribution, no bound process required); `create` assigns the next per-todo comment id and returns it with the updated todo; `update`/`delete` address a comment by that id (`UnknownComment` if absent); `list` returns the todo's comments. Ours |
| MCP `todo_lock`/`_unlock` (G5) | a todo's lock is **process-owned**: `lock` records the caller's bound process as `locked_by` ("signals, not ownership" — a lock another process holds is reported, not stolen; the atomic conditional write means a race grants exactly one); `unlock` clears it only if the caller holds it. The lock **auto-releases when the owning process closes** (the supervisor's `LockReleaser` close hook, shared with leases via a `CompositeLockReleaser`) and **every lock is cleared on launch** (per-run ids recycled) — but the durable todo persists. Needs a bound process (the owner). Ours |
| MCP todo deferrals | cross-project `todo_transfer` raises the same cross-scope question as the scratchpad `_transfer` and is a **tracked deferral** for the same focused follow-up; G4's Verify (the blocker gate) does not depend on it. Recorded so nothing is lost |
| Auto-summarization model | optional; use user's configured agent CLI headless, else disabled |
| Idle-detection thresholds & cues | Solo documents the *signals* per provider (visible output; OSC-title stability; OSC-title status) but not the exact quiet window, permission-prompt strings, or title status keywords. Ours: idle after **3** consecutive unchanged samples (~3 s at the ~1 Hz sampler); a small set of strong, model-agnostic approval cues for `Permission`, recognised only once output settles (conservative — prefers a missed prompt to a false block, which would deadlock a fire-when-idle workflow); generic title keywords (thinking/working/error) for the title-status provider. Our approximation, not copied. See `KNOWN-DIVERGENCES.md` D-5 |
| MCP param schemas | clean-room JSON Schemas, documented per tool |
| MCP `rename_process` | renames a process's **display label** only (the read-model `label`); it never alters the command, the trust record (keyed on the command variant), or identity/scope. Scoped to the session's effective project, so a caller cannot rename another project's process, but ungated by trust since a rename runs nothing. Emits `ProcessRenamed`. Solo documents the tool name, not the semantics — ours |
| MCP `close_process` | stops the process's group (the normal SIGTERM→grace→SIGKILL→reap path) and then **removes it from the registry entirely**, discarding its in-memory scrollback — distinct from `stop_process`, which leaves the process resting. Scoped to the session's project; ungated by trust. The group is reaped **before** the entry is forgotten, so no child is abandoned. Emits `ProcessRemoved`. The "remove, not just stop" reading is ours |
| MCP `select_process` | records an **informational** default-target process for the session, reported by `whoami`. Unlike `select_project` it confers no scope or authority — every scoped tool takes an explicit process id and the read tools already expose every process — so it is **not** authenticated against the peer process group; it only validates that the process exists. A convenience marker, our own decision |
| MCP `services_list` | the **command** processes of the session's effective project (agents and terminals omitted), each as its read-model row (status, discovered ports, readiness). Scoped to the project, so a caller sees only its own services |
| MCP `wait_for_bound_port` | waits until the process is listening on the port, returning a structured outcome — `bound` plus a reason (`timed_out`/`not_running`) when it is not — rather than erroring on a timeout, since "not up yet" is actionable. The wait is bounded well under the IPC request window, so a large requested timeout cannot tie up the connection (it returns `bound:false` instead) |
| MCP `flush_terminal_perf` | a no-op in Soloist. The rendered and raw scrollback buffers are written synchronously as PTY output is read, so an output read always reflects the latest; the only output coalescing is the frontend's per-frame terminal repaint, which never affects what the MCP/HTTP tools read. The tool exists for client compatibility and confirms the process exists |
| MCP `search_output`/`search_raw_output` | case-sensitive substring match over the rendered (resp. raw, lossy-UTF-8) lines; matching lines returned in order, result count bounded |
| MCP bulk command semantics | `start_all_commands` starts every **trusted** command in scope regardless of `auto_start` (Solo lists it as "trusted commands only"; the auto-start-only path is the dashboard's, exposed separately as `start-auto`/`start_all`). `stop_all_commands` stops only running **commands**, leaving agents and terminals running. `restart_all_commands` brings the trusted command set up fresh — running ones cycle, resting ones start — distinct from `restart-running` (running only); Solo names both but not the stopped-command behaviour, so the "also start the stopped ones" reading is ours. Untrusted commands are reported, never run |
| MCP effective project when none is selected or bound | resolve to the sole loaded project when exactly one is open; otherwise the scope is ambiguous and a scoped tool returns "no project in scope" until the caller `select_project`s. The select-explicit and infer-from-bound-process paths are per §7; the single-project default is our own convenience |
| MCP session↔process binding authenticity | Solo injects a process id (`SOLOIST_PROCESS_ID`) the agent's MCP client binds with, but does not document how it *authenticates* that binding. Ours (F13): the IPC adapter reads the connecting peer's kernel credentials (`SO_PEERCRED` → pid → its process **group**) per connection and the core matches that group to the managed process the caller runs in. `bind_session_process` is refused (`ForeignProcess`) unless the bound process's group leader is the peer's group; `select_project` is refused (`ForeignProject`) unless a process in the caller's own group belongs to the target project. A Soloist-launched agent's `soloist-mcp` child inherits the agent's process group — the group the supervisor recorded for that managed process — so the legitimate auto-bind matches while a forged binding to a sibling project's process does not. Cross-project isolation for the scoped action tools (process control, bulk start/stop/restart, `clear_output`, `spawn_agent`) therefore holds even with ≥2 projects on the shared `0700` socket. **External callers** (`register_agent`, no managed process in their group) cannot bind or select, so they get the open read tools plus, when exactly **one** project is loaded, the unambiguous single-project scope for mutating tools (same authority as the local user on the `0700` socket); with ≥2 projects open they have no authenticated scope and the scoped mutating tools refuse. The OS credential detail lives only in the adapter; the core compares plain process-group ids. Resolved in `KNOWN-DIVERGENCES.md` D-6 |
| HTTP mutation auth & status mapping | Mutations require `X-Soloist-Local-Auth: 1` (renamed from Solo's `X-Solo-Local-Auth`, D5 clean-room), enforced by an axum `route_layer` over the **mutation sub-router only** so the read routes stay open on loopback; a missing or wrong header is **401**. Supervisor outcomes map to HTTP: unknown process **404**, untrusted command **403** (the core trust gate), durable-store failure **500**; `stop` / `stop-all` are idempotent **200**. Each endpoint routes to the one core method the UI and MCP already use. Ours |
| HTTP bulk endpoint → core mapping | `start-auto` → `start_all` (the trusted `auto_start` subset), `start-all` → `start_all_commands` (every trusted command), `stop-all` → `stop_all` (every live process), `restart-running` → `restart_running`, `restart-all` → `restart_all_commands`. The start-auto vs start-all split mirrors the MCP bulk-command-semantics row above; Solo lists the endpoint names, the core-method mapping is ours |
| HTTP `reload` endpoint | **Tracked deferral.** Solo lists `POST /projects/:id/reload` but not its semantics. A correct reload must re-read `solo.yml` and **reconcile** the supervisor's registrations (update changed specs in place, add new, drop removed): `config.sync()` only refreshes the engine snapshot + trust review, and `supervisor.register()` mints a fresh id (re-running open would duplicate commands), so a naive "sync + restart-all" would restart with stale specs. Held for a focused follow-up that adds the registration-reconcile path; H3's Verify (`POST .../restart` works) and the nine implemented mutation endpoints do not depend on it. Recorded so nothing is lost |
| HTTP `/processes/:id/output` read endpoint | Added in Phase 10 (slice 3) for the CLI's `logs`: `GET /processes/:id/output?lines=N` returns a process's recent rendered output lines (oldest first) as a JSON array, thin over the **same** `Facade::process_output` the MCP output tools use — the default count and the ceiling are enforced in the core, an unknown id reads as an empty list (like `/ports`), and it is open on loopback (a read). Completes the read surface the CLI needs; Solo lists a `solo logs` but no HTTP shape for it, so the endpoint is ours |
| `soloist` CLI → endpoint mapping (H4) | The CLI is a thin HTTP client over `ipc::http` (port via `read_runtime()` → `DEFAULT_PORT` when the file is absent; `X-Soloist-Local-Auth` on mutations; a refused connection → a clear "Soloist is not running"). `status [--status running\|crashed]` → `GET /processes`, filtered and tabulated client-side; `start\|stop\|restart <name>` → resolve the name to an id via `GET /processes` (an ambiguous `label` across projects is **refused, never guessed** — the core's never-guess scope discipline) then `POST /processes/:id/<action>`; `start\|stop\|restart all` → the project bulk endpoints (`start-all`/`stop-all`/`restart-all`), resolving the project via `GET /projects` — the sole open project, or `--project <name>` when ≥2 are open (mirrors the MCP single-project default above); `logs <name> [-n N]` → `GET /processes/:id/output`; `focus` → `POST /focus`. Solo documents the subcommands, not the mapping — ours |
| `soloist spawn` | **Tracked deferral.** Launching an agent over HTTP has no endpoint: the core's `spawn_agent` is session-scoped (it needs a bound session plus trust/scope), and the loopback API has no session concept, so a spawn endpoint needs its own project/session-scoping + trust design first. Held for a focused follow-up; H4's Verify (`soloist status` prints the table) does not depend on it. Distinct from the separate `spawn_process` arbitrary-terminal deferral. Recorded so nothing is lost |
| `soloist open` | **Tracked deferral.** Solo's `open` is terse: "raise the app" already overlaps `focus` (`POST /focus`), and "open a project folder" needs a `load_project` HTTP endpoint that does blocking fs and spawns actors. Deferred to keep the slice tight; add a project-open endpoint in a focused follow-up if wanted. Recorded so nothing is lost |
| MCP feature-group enablement (G10) | Solo documents that the MCP core tool groups are always on and the feature groups toggle (Scratchpads/Todos/Timers inherit; **Key-Value defaults OFF**), but not the mechanism. Ours: enablement is a durable setting (`McpToolGroups` — Scratchpads/Todos/Timers default on, Key-Value off) read in the core via `Facade::mcp_tool_groups` / `set_mcp_tool_group`. The toggle governs the **MCP surface**, not the core feature, so it gates only the `soloist-mcp` server — not the `Facade` methods or a future settings UI (which would wrongly disable a UI panel). The server reads the enablement once at startup over a global `McpToolGroups` IPC read (open on the `0700` socket, not project-scoped) and composes only the enabled feature-group sub-routers (core groups always); a disabled group's tools are therefore absent from `list_tools` and uncallable. A settings change applies on the next MCP-client reconnect (a session resolves its tool surface on connect). If the app is unreachable at startup, the server falls back to the defaults so it still lists its core tools. Ours (clean-room) |
| Settings persistence | App preferences persist in **SQLite** as a single global record (`settings` table, `id = 1` singleton CHECK, migration v9) holding the `Settings` document as JSON, so the persisted shape is the domain type and serde document defaults keep a record an older build wrote readable. The `SettingsStore` aggregate applies the documented defaults when no record exists. The first setting is the MCP feature-group enablement; theme / terminal / notification settings (I5/I7) extend the same document. Ours |
| Storage layer | **SQLite** (todos/scratchpads/KV/locks/trust/settings); runtime process state in memory |
| Terminfo / `TERM` | `TERM=xterm-256color` (no custom terminfo) |

---

## 13. Source index
Primary: [soloterm.com/docs](https://soloterm.com/docs) (full tree via
[sitemap.xml](https://soloterm.com/sitemap.xml)) · [changelog](https://soloterm.com/changelog) ·
[blog/the-agentic-metaharness](https://soloterm.com/blog/the-agentic-metaharness) ·
[agents](https://soloterm.com/agents) · comparison/alternatives pages ·
[GitHub org soloterm](https://github.com/soloterm) (note: `soloterm/solo` there is the unrelated
**Laravel PHP TUI**, not this app — the macOS app is closed-source). Independent:
[eshlox.net review](https://eshlox.net/solo-changed-how-i-work-with-terminals),
[YouTube "vogel" review](https://www.youtube.com/watch?v=uHA9cHuaJ4E) (metadata only).
