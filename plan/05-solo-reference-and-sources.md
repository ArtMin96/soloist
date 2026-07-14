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
🟡 Fidelity (Solo v0.9.3): Solo's changelog notes a fix so reconciliation no longer risks acting on a
**PID/PGID the OS recycled** to an unrelated group. We honor that class of safety: each recorded group is
stamped with a stable process identity (kernel `boot_id` + the leader's `/proc/<pid>/stat` start-time) and
a record is adopted or killed only when that identity still matches the live group — a reused pgid is
dropped, never killed. The identity mechanism is clean-room (Solo documents the fix, not its
implementation); the observable fail-closed nuances are recorded in `KNOWN-DIVERGENCES.md` D-16.

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
  require header **`X-Solo-Local-Auth: 1`**. CORS limited to localhost. *(Soloist diverges: every
  route — reads too — requires a per-launch random token plus a loopback `Host` guard; see
  `KNOWN-DIVERGENCES.md` D-17.)*
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
| MCP `timer_fire_when_idle_any` / `_all` | fires when **any** / **all** of the watched `processes` are idle, or a **max-wait backstop** elapses (default **1 hour**, ceiling 24 h — so a stuck worker can never block the timer forever). "Idle" is the C4 idle FSM's `Idle` state, consumed by the scheduler as `AgentActivityChanged` events. A watched process that has **left the registry** counts as idle (it can no longer work — avoids deadlock); a running-but-unclassified or non-agent watched process is **not** idle (the backstop still fires it). The response reports `already_idle` (condition met at set time) and `waiting_on` (the watched processes not yet idle). Watched processes **need not be in scope**: a timer only ever delivers to its own owner, and watching observes only a coarse idle/not-idle signal for the backstop — never a process's output (which is project-scoped, PRD-06) — so only the owner (delivery target) is bound/scope-authenticated. Ours (clean-room) |
| MCP `timer_cancel` / `_pause` / `_resume` / `_list` | management of the caller's own timers, **scoped to the bound process** (a caller manages only timers it owns; `cancel`/`pause`/`resume` return whether they affected one, `list` returns the owner's timers). **Pause freezes the time that remains** until the deadline; **resume re-arms** the timer with that remaining time (so a paused-then-resumed timer is not instantly overdue). A paused timer never fires. Ours |
| Timer durability vs reconcile | timers persist in **SQLite** (migration v5) like the other aggregates, but — exactly like leases — a timer is **process-owned** and per-run process ids are recycled, so a timer left by a previous run names an owner that no longer exists and could never be delivered. **Launch reconciliation clears every timer.** So G11's "coordination state survives an app restart" is satisfied by the **content** aggregates (todos / scratchpads / key-value, still to build), not by the process-owned timers and leases, which do not meaningfully outlive the run that created them |
| Scratchpad discipline & identity (G1) | A scratchpad is a **disciplined, typed document**, not free-form Markdown: `ScratchpadDoc { objective, context, plan[], acceptance_criteria[], risks[], status, notes? }`, defined once in the core, validated on write (no required field blank; the three lists each need ≥1 non-blank entry), and rendered to one canonical Markdown layout (H1 = the scratchpad's name). This is a deliberate divergence from Solo's free-form note (per the project owner) — see `KNOWN-DIVERGENCES.md` D-7. Identity is a **durable, store-assigned `ScratchpadId`** (stable across rename and restart, migration v6) addressed by a unique **`name`** handle per project; scratchpads are project-scoped shared content (not process-owned) and **survive a restart** (G11), so launch reconciliation never clears them. Ours |
| MCP `scratchpad_write` (G1/G2) | creates or replaces `(project, name)` with the disciplined document, **revision-guarded** (optimistic concurrency, G2): `expected_revision` omitted = create (refused if one exists), or the current revision = update (refused on mismatch, bumping to `expected+1`). The check-and-write is one atomic store step. A malformed document is refused (`InvalidScratchpad`); a stale revision is `RevisionConflict { expected, actual }` (actual `None` when absent). Returns the written document at its new revision plus its canonical rendering. Solo documents the tool name; the typed schema, validation, and revision semantics are ours |
| MCP `scratchpad_read`/`_list` | `read` returns one scratchpad by `name` — the structured document, its tags/archived/revision, and the canonical Markdown rendering — or `UnknownScratchpad`. `list` returns every scratchpad in the effective project as one-line summaries (name, tags, revision, archived, objective gist). Solo's free-form read modes (full/headings/section) are unneeded against a structured document; reading a single section is a client-side field access. Ours |
| MCP `scratchpad_rename`/`_add_tags`/`_remove_tags`/`_tags_list`/`_archive`/`_delete` | management of a scratchpad within the effective project: `rename` changes the `name` handle (durable id unchanged; `ScratchpadNameTaken` if the target is used, `UnknownScratchpad` if absent); `add_tags`/`remove_tags` are atomic read-modify-write of the tag set (idempotent), returning the updated document; `tags_list` is the distinct tags across the project; `archive` toggles a listing flag (keeps the document); `delete` removes it. All project-scoped, ungated by trust (content, not execution). Ours |
| MCP scratchpad free-form & file-io tools | **Resolved (2026-07-01).** The free-form-oriented verbs Solo lists (`_append`/`_edit`/`_append_section`/`_tail`/`_find`/`_clear`) are an **intentional divergence — not implemented**: they presuppose a free-form buffer, so several have no clean mapping onto the disciplined document and some would violate its invariants (`_clear` vs the non-blank rule; `_append_section` vs the fixed sections). The revision-guarded whole-document `scratchpad_write` is the deliberate replacement. The host file-io tools (`_save_to_file`/`_load_from_file`) are **formally declined** — no MCP tool reads or writes an arbitrary host path until a dedicated project-root FS-sandbox security pass (not planned). Recorded in `KNOWN-DIVERGENCES` D-7. Cross-project `_transfer` is delivered separately by the O10 transfer slice. |
| Todo discipline & identity (G3) | A todo is a **disciplined, typed document**, not a free-form item: `TodoDoc { title, description, acceptance_criteria[], risks[], status }` (the same enforced-schema directive as scratchpads — see `KNOWN-DIVERGENCES.md` D-8), validated on write (`title`/`description` non-blank; `acceptance_criteria`/`risks` each ≥1 non-blank → `InvalidTodo`). Around it sit live columns mutated by dedicated atomic ops — tags, blockers, comments, and a process-owned lock — kept out of the revision-guarded document so a tag/comment change never collides with a specification edit. Identity is a **durable, store-assigned `TodoId`** (stable across runs, so a sibling todo can name it as a blocker; migration v7). Todos are durable shared content that **survives a restart** (G11); only their process-owned lock is cleared on launch. Ours |
| MCP `todo_create`/`_update`/`_get`/`_list`/`_delete` (G3) | `create` makes a todo from the disciplined doc at revision 1; `update` replaces the doc **revision-guarded** (`expected_revision`; mismatch → `TodoRevisionConflict`, like scratchpads G2); `get`/`list` return the read model (`list` as one-line summaries incl. a derived `blocked` flag); `delete` removes it. Project-scoped, ungated by trust (content, not execution) — an external single-project caller can use them without binding a process. Solo documents the tool names; the typed schema and revision semantics are ours |
| MCP `todo_complete` + the blocker gate (G4) | `status` (`Open`/`Blocked`/`InProgress`/`Done`) is the label an agent declares; the **gate** is the todo's unmet **blockers**. `todo_complete` (and an `update` to `Done`) is refused with `TodoBlocked { by }` while any blocker still exists and is not done — so a todo "stays gated until its blockers complete" (G4 Verify). A **deleted** blocker counts as met (never deadlocks). The gate lives in the blocker set, not the status label, so there is one source of truth for blocked-ness. Ours (clean-room) |
| MCP `todo_set_blockers`/`_add_blocker`/`_remove_blocker`/`_add_tag`/`_remove_tag`/`_tags_list` (G4) | atomic read-modify-write of a todo's live columns: setting/adding a blocker validates it exists in the project (`UnknownBlocker`) and is not the todo itself (`SelfBlocker`); removing is idempotent. Tags add/remove are idempotent (sorted set); `tags_list` is the distinct tags across the project. All project-scoped, ungated by trust. Ours |
| MCP `todo_comment_create`/`_update`/`_delete`/`_list` (G4) | `create` assigns the next per-todo comment id and returns it with the updated todo; `update`/`delete` address a comment by that id (`UnknownComment` if absent); `list` returns the todo's comments. **Authorship (O12 — decided 2026-06-28, implemented in orch-02):** a comment **records its creating bound actor** (`author_actor_id` + a display author), populated by the core on `create` and surfaced on the to-do board — **reversing this row's earlier "no author attribution" decision**, a correction toward the demo (its on-screen `todo_get` shows `author`/`author_actor_id`). An external caller with no bound process can still comment, its author left unattributed. The G4 slice today carries no author; orch-02 adds the field. Ours |
| MCP `todo_lock`/`_unlock` (G5) | a todo's lock is **process-owned**: `lock` records the caller's bound process as `locked_by` ("signals, not ownership" — a lock another process holds is reported, not stolen; the atomic conditional write means a race grants exactly one); `unlock` clears it only if the caller holds it. The lock **auto-releases when the owning process closes** (the supervisor's `LockReleaser` close hook, shared with leases via a `CompositeLockReleaser`) and **every lock is cleared on launch** (per-run ids recycled) — but the durable todo persists. Needs a bound process (the owner). Ours |
| MCP `todo_transfer` / `scratchpad_transfer` (O10) | **Resolved (2026-07-01).** Cross-project transfer is delivered: `Facade::{todo,scratchpad}_transfer_in(from, name/id, to)` moves the durable aggregate to the target project via a new atomic repo `transfer` (todo keeps its doc/comments/tags/revision/id and **clears blockers + lock** — both reference the source project; scratchpad keeps everything and refuses a name already used in the target). **Cross-scope authorization (clean-room — Solo documents the tool, not the auth):** the session-scoped `{todo,scratchpad}_transfer(session, …, to)` resolves the source from the caller's own effective scope and requires the target to be **independently peer-authenticated** (`authentic_scope` — a process the caller runs in belongs to `to`), else `ForeignProject`; the identity is taken from the **authenticated session, never a wire-supplied id** (the never-widen-scope rule). Because an MCP session authenticates (via `SO_PEERCRED`) to a **single** project, a genuine cross-project transfer initiated over MCP is refused by design — the reachable success path is the local/trusted `*_transfer_in` surface, exposed over the loopback HTTP API as `POST /projects/:id/transfer-todo` / `transfer-scratchpad` (the local user's authority, addressing both projects by explicit id; the target must be loaded — an unknown target is refused as `UnknownProject` so a bad id never orphans the aggregate, `404` over HTTP). The two MCP tools carry clean-room JSON Schemas (`{todo|name, to_project}`), are in the `EXPECTED_TOOL_SURFACE` guard, and honestly document that a cross-project move is refused over a single-project session (D-6/D-8). G4's blocker gate never depended on it. |
| Orchestrator (clean-room composition, O1–O14) | "Orchestrator" is **not** a documented Solo concept (absent from this doc, `02`, `04`, `06`); it is a Soloist-original composition of documented primitives — `spawn_agent` (F11), todos+blockers+locks (G3–G5), leases (G6), timers + `timer_fire_when_idle` (G7–G9), the idle FSM (E5), output reads (F9), scratchpads (G1/G2) — surfaced as a first-class capability. Recorded here as an explicit **matrix expansion** (rows **O1–O14**, `02` §O, charter `orchestrator/README.md`) rather than attributed to Solo (`CLAUDE.md` §9). The orchestration *mechanism* is already built and `Verified` (`crates/pty/tests/orchestration.rs`, E7); the track adds UX, the deferred sub-tools (O9/O10), and documentation — **no new coordination primitive**. The UX north star is the demo's *feel* only (clean-room). Ours |
| Orchestration read-model & coordination events (O1/O2) | The orchestration UI renders a **pushed read-model**, so the core exposes `Facade::orchestration_snapshot(project)` — an `OrchestrationSnapshot` **derived on read** (never a cached second copy, `04` §2) projecting the project's agent tree (each managed process with `ProcStatus` + its `AgentActivity`; `parent` filled once O3 lands lineage), todos (full views), armed/paused timers, live leases, and scratchpad/kv summaries — assembled purely from the existing C2 registry, C4 idle tracker, and the C6 aggregate reads (a project-scoped lease and timer read are added as **additive reads**; no write path changes). A live UI also needs change-notifications, so the core adds `DomainEvent`s carrying **ids only** (the UI re-queries the snapshot coalesced per frame, never a per-event payload): `TodoChanged{project,id}`, `TimerArmed`/`TimerFired`/`TimerCleared{owner,id}`, `LeaseChanged{project,key}`, `ScratchpadChanged{project,name}`, `KvChanged{project,key}`; `AgentActivityChanged` (C4) is reused for the tree. **Emission seam:** the one C8 `Facade` write methods emit each event (so a mutation from **any** adapter — including an agent over MCP — is seen identically, with the C6 aggregates left pure), except `TimerFired`, emitted by the C6 `TimerScheduler` (which fires autonomously and already holds the bus). Close-driven releases (a process's leases/todo-locks auto-released on close) are **not** re-emitted — they are observed via the existing process-lifecycle events the read-model already re-queries on. `ScratchpadChanged` is keyed by `name` (the scratchpad surface's addressing handle, consistent with kv/lease `key` and todo `id`). Timer `pause`/`resume` notifications — `TimerPaused`/`TimerResumed{owner,id}` — added in orch-03 (implemented 2026-06-29); they complete the O2 event set and drive the timers panel's live status. Ours (clean-room) |
| Spawn orchestration-context preamble (O13) | Solo's demo spawns a worker with `include_agent_instructions`, delivering a first-turn briefing so the worker self-onboards. Ours (decided 2026-06-28, implemented in orch-04): `spawn_agent`/`spawn_process` deliver a first-turn **`[SOLO ORCHESTRATION CONTEXT]` preamble** — the worker's identity plus the coordination tools (`whoami`, scratchpads, todos, locks/leases, kv, timers) — so a spawned worker uses the primitives with no skills loaded. Today only `SOLOIST_PROCESS_ID` is injected. The preamble **text is ours** (clean-room — not Solo's strings); it applies to the already-built `spawn_agent` and is **not** gated on the O9 arbitrary-spawn trust work. Net-new behavior toward the demo. Ours |
| Agent MCP client configuration (launched agents) | Solo does **not** auto-register its MCP server with agents it launches: per [docs/integrations/mcp-server](https://soloterm.com/docs/integrations/mcp-server) (checked 2026-07-03), users configure each client once from the generated Settings snippets, with a one-click **Run** action for supported CLI clients. Ours (decided 2026-07-03): the same — `Facade::launch_agent` injects no MCP config; the Integrations snippets (F2) are the setup path, and the packaged installs ship the helper so those snippets work outside a dev checkout (F1 note). Solo's one-click **Run** configure action is tracked as a *later* candidate alongside F2, not built. |
| Worker spawn depth | Solo does not document whether an agent-spawned worker may itself spawn workers. Ours (decided 2026-07-02): **delegation is one level deep** — a session bound to a process recorded as a spawned worker this run has `spawn_agent` refused with a typed, caller-fixable error (`WorkerMayNotSpawn` → MCP tool error), keeping the orchestration tree lead→workers and preventing runaway recursive spawning. **STRICT for the run:** the refusal persists after the worker's lead closes (the lineage edge lives as long as the worker; the tree's re-root-on-read is a display rule, not a promotion). The gate lives in the core, before the launch, so a refusal spawns and records nothing. The HTTP/CLI spawn route (`POST /projects/:id/spawn-agent`) is the local user's ungated authority and untouched. Clean-room, ours |
| `solo://` deep-link handoff (O14) | Solo documents `solo://` deep links to projects/processes/todos/scratchpads (§10); the orchestrator's core human handoff is pasting a scratchpad/todo link to an agent. We **promote the orchestrator slice of `solo://` (the `later` row I4) to v1** (decided 2026-06-28, implemented in orch-02): a stable `solo://proj/<id>/scratchpad\|todo/<id>` link, a UI "Copy link" affordance, and a **core resolver** so a receiving bound agent reads the target — a **malformed or foreign-scope link is refused** (the never-guess scope discipline). The link *scheme* is documented for Solo; its exact shape and the resolver are ours. The broader deep-link UX (jump-to-process, app-raise) stays `later` (I4). Ours |
| Wake-reason prefix on timer delivery (O8) | The scheduler prepends a compact, clean-room header to every delivered timer body so the woken agent can tell "all peers finished" from "max-wait backstop elapsed" without relying on the UI. Format: `[Soloist timer #<id>] <reason>` where reason is one of "scheduled delivery" (`FireCond::At`), "all N watched agents are idle" (WhenIdleAll, quorum met), or "max-wait backstop elapsed (when-all-idle / when-any-idle, N watched)". The header is followed by a newline and then the original body; the whole string ends with `\r` so the agent's readline submits it. Solo's demo shows `[Solo timer #id] [wait for all: all watched idle: …]` on-screen; our format is independent clean-room text, not a copy. Ours |
| GPU terminal renderer & fallback (C8) | Solo documents a **GPU renderer** since v0.6.0 (§10/§11) but not the renderer-selection mechanism. Ours: the xterm.js **WebGL** addon (`@xterm/addon-webgl`), activated after the terminal opens, with the addon **lazy-loaded** via dynamic import (its own ~123 kB / ~35 kB-gzip chunk, fetched only when a terminal first mounts — size budget, `CLAUDE.md` §6). It **falls back to xterm's built-in DOM renderer** when WebGL2 is unavailable at activation (no GPU/driver/blocked context) and reverts to DOM if the GPU context is lost at runtime (`onContextLoss` → dispose). There is **no canvas fallback tier**: xterm v6 removed the canvas renderer, so DOM is the only fallback (`KNOWN-DIVERGENCES.md` D-10). Selection logic lives once in `lib/terminalRenderer.ts`. Ours (clean-room) |
| Auto-summarization model | optional; use user's configured agent CLI headless, else disabled |
| Idle-detection thresholds & cues | Solo documents the *signals* per provider (visible output; OSC-title stability; OSC-title status) but not the exact quiet window, permission-prompt strings, or title status keywords. Ours: idle after **3** consecutive unchanged samples (~3 s at the ~1 Hz sampler); a small set of strong, model-agnostic approval cues for `Permission`, recognised only once output settles (conservative — prefers a missed prompt to a false block, which would deadlock a fire-when-idle workflow); generic title keywords (thinking/working/error) for the title-status provider. Our approximation, not copied. See `KNOWN-DIVERGENCES.md` D-5 |
| MCP param schemas | clean-room JSON Schemas, documented per tool |
| MCP `rename_process` | renames a process's **display label** only (the read-model `label`); it never alters the command, the trust record (keyed on the command variant), or identity/scope. Scoped to the session's effective project, so a caller cannot rename another project's process, but ungated by trust since a rename runs nothing. Emits `ProcessRenamed`. Solo documents the tool name, not the semantics — ours |
| MCP `close_process` | stops the process's group (the normal SIGTERM→grace→SIGKILL→reap path) and then **removes it from the registry entirely**, discarding its in-memory scrollback — distinct from `stop_process`, which leaves the process resting. Scoped to the session's project; ungated by trust. The group is reaped **before** the entry is forgotten, so no child is abandoned. Emits `ProcessRemoved`. The "remove, not just stop" reading is ours |
| MCP `select_process` | records an **informational** default-target process for the session, reported by `whoami`. Unlike `select_project` it confers no scope or authority — every scoped tool takes an explicit process id and `list_processes` still discloses every process's identity (out-of-scope rows redacted to identity, PRD-06) — so it is **not** authenticated against the peer process group; it only validates that the process exists (it reads no output). A convenience marker, our own decision |
| MCP `services_list` | the **command** processes of the session's effective project (agents and terminals omitted), each as its read-model row (status, discovered ports, readiness). Scoped to the project, so a caller sees only its own services |
| MCP `wait_for_bound_port` | waits until the process is listening on the port, returning a structured outcome — `bound` plus a reason (`timed_out`/`not_running`) when it is not — rather than erroring on a timeout, since "not up yet" is actionable. The wait is bounded well under the IPC request window, so a large requested timeout cannot tie up the connection (it returns `bound:false` instead) |
| MCP `flush_terminal_perf` | a no-op in Soloist. The rendered and raw scrollback buffers are written synchronously as PTY output is read, so an output read always reflects the latest; the only output coalescing is the frontend's per-frame terminal repaint, which never affects what the MCP/HTTP tools read. The tool exists for client compatibility and confirms the process exists |
| MCP `search_output`/`search_raw_output` | case-sensitive substring match over the rendered (resp. raw, lossy-UTF-8) lines; matching lines returned in order, result count bounded |
| MCP bulk command semantics | `start_all_commands` starts every **trusted** command in scope regardless of `auto_start` (Solo lists it as "trusted commands only"; the auto-start-only path is the dashboard's, exposed separately as `start-auto`/`start_all`). `stop_all_commands` stops only running **commands**, leaving agents and terminals running. `restart_all_commands` brings the trusted command set up fresh — running ones cycle, resting ones start — distinct from `restart-running` (running only); Solo names both but not the stopped-command behaviour, so the "also start the stopped ones" reading is ours. Untrusted commands are reported, never run |
| MCP effective project when none is selected or bound | resolve to the sole loaded project when exactly one is open; otherwise the scope is ambiguous and a scoped tool returns "no project in scope" until the caller `select_project`s. The select-explicit and infer-from-bound-process paths are per §7; the single-project default is our own convenience |
| MCP session↔process binding authenticity | Solo injects a process id (`SOLOIST_PROCESS_ID`) the agent's MCP client binds with, but does not document how it *authenticates* that binding. Ours (F13): the IPC adapter reads the connecting peer's kernel credentials (`SO_PEERCRED` → pid → its process **group**) per connection and the core matches that group to the managed process the caller runs in. `bind_session_process` is refused (`ForeignProcess`) unless the bound process's group leader is the peer's group; `select_project` is refused (`ForeignProject`) unless a process in the caller's own group belongs to the target project. A Soloist-launched agent's `soloist-mcp` child inherits the agent's process group — the group the supervisor recorded for that managed process — so the legitimate auto-bind matches while a forged binding to a sibling project's process does not. Cross-project isolation for the scoped action tools (process control, bulk start/stop/restart, `clear_output`, `spawn_agent`) therefore holds even with ≥2 projects on the shared `0700` socket. **External callers** (`register_agent`, no managed process in their group) cannot bind or select, so they get the open read tools plus, when exactly **one** project is loaded, the unambiguous single-project scope for mutating tools (same authority as the local user on the `0700` socket); with ≥2 projects open they have no authenticated scope and the scoped mutating tools refuse. The OS credential detail lives only in the adapter; the core compares plain process-group ids. Resolved in `KNOWN-DIVERGENCES.md` D-6 |
| HTTP auth & status mapping | **Every** route (reads and mutations) requires the header `X-Soloist-Local-Auth: <token>` carrying the launch's **per-launch random token** (PRD-06, superseding the old constant `"1"` mutation-only gate — see `KNOWN-DIVERGENCES.md` D-17), compared in constant time; the token is written to the `0600` runtime file inside the `0700` data dir, so only the owning user reads it. A **`Host`-header guard** rejects a non-loopback `Host` with **403** (DNS-rebinding defence). A missing or wrong token is **401**. Supervisor outcomes map to HTTP: unknown process **404**, untrusted command **403** (the core trust gate), durable-store failure **500**; `stop` / `stop-all` are idempotent **200**. Each endpoint routes to the one core method the UI and MCP already use. Ours |
| HTTP bulk endpoint → core mapping | `start-auto` → `start_all` (the trusted `auto_start` subset), `start-all` → `start_all_commands` (every trusted command), `stop-all` → `stop_all` (every live process), `restart-running` → `restart_running`, `restart-all` → `restart_all_commands`. The start-auto vs start-all split mirrors the MCP bulk-command-semantics row above; Solo lists the endpoint names, the core-method mapping is ours |
| HTTP `reload` endpoint | **Resolved (2026-07-01).** `POST /projects/:id/reload` → `Facade::reload_project` → `ProjectService::reload`, the one core reconcile every adapter routes to. It re-reads `solo.yml` via `config.sync` (which also announces `ConfigChanged`), then applies the diff to the supervisor registry through new primitives (`Registry::command_id_by_name`/`update_command_spec`/`remove_if_resting`; `Supervisor::update_command`/`deregister_if_resting`). **Clean-room semantics — Solo documents the endpoint, not its behavior:** an **added** command is registered resting + untrusted (reload never starts anything, mirroring `restore`); an **updated** spec is replaced **in place**, keeping the process id (never duplicated) and never killing a running process — the new, now-untrusted variant takes effect on its next restart, which the trust gate re-checks; a **renamed** command is relabelled in place, preserving trust (keyed on the variant); a **removed** command is dropped only if resting, left running otherwise. A byte-identical file is a no-op (`None`); an unknown project is `UnknownProject` → `404`. Read-model refresh piggybacks on `ConfigChanged` + the per-process `ProcessSpawned`/`ProcessRemoved`/`ProcessRenamed` deltas — no new event. |
| Project removal | **Ours (2026-07-03) — Solo documents no project-removal behavior anywhere (no changelog entry, no docs page, no MCP tool), so the semantics are clean-room.** `ProjectService::remove` is the one core removal every adapter routes to (`Facade::remove_project` → the desktop confirm dialog's `project_remove` command, `DELETE /projects/:id` on the loopback HTTP API, and `soloist-cli remove-project <name>`; **deliberately no MCP tool** — an agent must not be able to delete a project, and Solo's documented tool catalog has none either). Semantics (owner-confirmed): **stop-then-remove** — every process of the project is closed first (`Supervisor::close_all`: stop messaged to all so the SIGTERM grace windows overlap, then each entry reaped and forgotten via the one per-process `close`, announcing `ProcessRemoved` per process **before** the project's own event, so no child is ever abandoned); the config engine's sync state is evicted; the durable record is deleted and SQLite **cascades** to all project-scoped state (trust, leases, timers, scratchpads, todos, key-value, project settings, project-scoped prompt templates — global state untouched); then `DomainEvent::ProjectRemoved { id }` announces it (the file-watch reactor also re-syncs on it, dropping the root's OS watch). **Files on disk are never touched** — the folder and `solo.yml` remain; re-opening later registers fresh and untrusted. An unknown project is `UnknownProject` (`404`), removing nothing. The UI confirm is a genuine-decision modal (DESIGN.md dialog vocabulary) naming the running-process count and the disk guarantee; the CLI requires the project name explicitly (a destructive action never defaults to the sole project). |
| HTTP `/processes/:id/output` read endpoint | Added in Phase 10 (slice 3) for the CLI's `logs`: `GET /processes/:id/output?lines=N` returns a process's recent rendered output lines (oldest first) as a JSON array, thin over the **same** `Facade::process_output` the MCP output tools use — the default count and the ceiling are enforced in the core, an unknown id reads as an empty list (like `/ports`), and it is open on loopback (a read). Completes the read surface the CLI needs; Solo lists a `solo logs` but no HTTP shape for it, so the endpoint is ours |
| `soloist` CLI → endpoint mapping (H4) | The CLI is a thin HTTP client over `ipc::http` (port **and per-launch token** via `read_runtime()` → `DEFAULT_PORT` + empty token when the file is absent; the token rides **every** request now that reads are gated too, PRD-06; a refused connection → a clear "Soloist is not running", a rejected token → "run this as the same user Soloist is running as"). `status [--status running\|crashed]` → `GET /processes`, filtered and tabulated client-side; `start\|stop\|restart <name>` → resolve the name to an id via `GET /processes` (an ambiguous `label` across projects is **refused, never guessed** — the core's never-guess scope discipline) then `POST /processes/:id/<action>`; `start\|stop\|restart all` → the project bulk endpoints (`start-all`/`stop-all`/`restart-all`), resolving the project via `GET /projects` — the sole open project, or `--project <name>` when ≥2 are open (mirrors the MCP single-project default above); `logs <name> [-n N]` → `GET /processes/:id/output`; `spawn <tool> [--project <name>] [-- args]` → resolve the project via `GET /projects` (sole open, or `--project`) then `POST /projects/:id/spawn-agent` (launches a known agent tool as a worker — the local-user-authority `launch_agent`); `focus` and `open` → `POST /focus` (Solo's `open` raise-app case is the same action, so both share one `raise` handler). Solo documents the subcommands, not the mapping — ours |
| `soloist spawn` | **Resolved (2026-07-01).** `POST /projects/:id/spawn-agent` launches a **known** agent tool as a worker via the same `Facade::launch_agent` the desktop launch picker drives — the local user's authority on the loopback socket (an ungated `Agent`, a root process). It is **not** the session-scoped MCP `spawn_agent`, which needs a bound (`SO_PEERCRED`-authenticated) session to derive scope + lead lineage and so stays MCP-only; the loopback API deliberately has no session and addresses the project by explicit id. `soloist spawn <tool> [--project <name>] [-- args]` resolves the project (sole open, or `--project`) and posts `{tool,args}`; an unknown tool/project is a `404`. Arbitrary `spawn_process` (a non-vetted command) is a separate deferral (O9), unchanged. |
| `soloist open` | **Resolved (2026-07-01).** Solo's `open` = raise the app, so the `open` subcommand shares the `raise` handler with `focus` and maps to the existing `POST /focus` — no new endpoint. The separate "open a project folder over HTTP" case (a `load_project` endpoint doing blocking fs + actor spawn) is **not built**: it is out of scope for a stack-control CLI, and the desktop app already opens folders via its own file-association / argv path. Recorded so nothing is lost. |
| MCP feature-group enablement (G10) | Solo documents that the MCP core tool groups are always on and the feature groups toggle (Scratchpads/Todos/Timers inherit; **Key-Value defaults OFF**), but not the mechanism. Ours: enablement is a durable setting (`McpToolGroups` — Scratchpads/Todos/Timers default on, Key-Value off) read in the core via `Facade::mcp_tool_groups` / `set_mcp_tool_group`. The toggle governs the **MCP surface**, not the core feature, so it gates only the `soloist-mcp` server — not the `Facade` methods or a future settings UI (which would wrongly disable a UI panel). The server reads the enablement once at startup over a global `McpToolGroups` IPC read (open on the `0700` socket, not project-scoped) and composes only the enabled feature-group sub-routers (core groups always); a disabled group's tools are therefore absent from `list_tools` and uncallable. A settings change applies on the next MCP-client reconnect (a session resolves its tool surface on connect). If the app is unreachable at startup, the server falls back to the defaults so it still lists its core tools. Ours (clean-room) |
| Settings persistence | App preferences persist in **SQLite** as a single global record (`settings` table, `id = 1` singleton CHECK, migration v9) holding the `Settings` document as JSON, so the persisted shape is the domain type and serde document defaults keep a record an older build wrote readable. The `SettingsStore` aggregate applies the documented defaults when no record exists. The first setting is the MCP feature-group enablement; theme / terminal / notification settings (I5/I7) extend the same document. Ours |
| Settings base — one store for two surfaces (I7s) | Solo has a global Settings window and a per-project settings page; the storage split is undocumented. Ours: **one generic base** `SettingsStore<K, D>` over a `SettingsRepo<K, D>` port — `get(key)` (absent → document default) plus one `update(key, mutator)` write primitive. `K = ()` keys the global singleton (`Settings`), `K = ProjectId` keys per-project local settings (`ProjectSettings`, 11a). Adding a setting is one `#[serde(default)]` field + one façade getter/setter — never a new store, table, or migration. Clean-room (`plan/06` §5.9) |
| Global Settings tabs — fields & defaults (I7f–I7k) | Source: the demo's global-settings tour (Aaron Francis, `youtube.com/watch?v=kVyFCcP6B28`, **7:16–9:36**), read frame-by-frame. The video confirms the **controls exist** and a few defaults; the discrete step-sets and the unshown defaults are **ours** (clean-room — never fabricated as Solo's). **Appearance:** theme Light/Dark/**System** (our default System); an "A·A·A" interface + terminal font-scale stepper (our 5 steps, default Medium); terminal font weight / bold weight as the CSS 100–900 steps (**Solo defaults 400 / 600**); line-height (~1.0–1.8, **Solo default ~1.1**) and letter-spacing (~0.5–1.3, **Solo default ~0.9**) as our discrete enums mapped to a CSS value in the frontend; focus-on-click toggle (our default off = double-click). **Sidebar:** show-filter-input, hide-empty-sections, three project hover-action toggles, and a settings-footer toggle (our boolean defaults: filter on, hide-empty off, hover actions on, footer on); per-header CPU/mem usage thresholds as closed enums with the demo's option sets (project CPU Always/25/50/100/200/Never, project mem Always/500MB/1/2/8GB/Never, process CPU Always/10/30/60/90/Never, process mem Always/100/500MB/1/2GB/Never; **our default Always** — the unshown picker default). **Agents:** the tool registry reuses the C4 Phase-7 registry; the new setting is the auto-summarization opt-in (tool + model), **OFF by default** (locked decision — core never hard-depends on an LLM). **Tools:** default editor + default terminal (the chosen launch name, `None` = system default; editor overridable per project, 11a). **Integrations:** master MCP toggle + master HTTP-API toggle (both our default on); the per-group MCP enablement is the existing `McpToolGroups` (D4 stdio, no TCP port — Solo's "Port 45678" is N/A; HTTP API stays `127.0.0.1:24678`, H1). The whole-tab façade setter auto-saves the sub-document. Ours (clean-room) |
| Global Hotkeys registry (I7h) | Solo's Hotkeys tab is a searchable, scoped, remappable keymap with "Reset all to defaults" and hover-press-x to disable; the defaults are the macOS `⌘`/`⌥` reference (§10). Ours: a closed `HotkeyAction` set with a code-defined default per action — the single source — each in a `HotkeyScope` (General / Sidebar / Terminal). The default keymap remaps **`⌘`→Ctrl and `⌥`(Option)→Alt** for Linux (the standard Cmd→Ctrl convention; Super is left to the window manager). The durable document stores **only deviations** — a remap (`Some(binding)`) or a disable (`None`) — so "Reset all" clears the overrides and a future default change reaches anyone who has not overridden that action. A binding is a typed chord (modifier flags + a `KeyboardEvent.key` token) so the frontend matches a real key event. **Conflicts are reported only within a scope** — a key shared across scopes (e.g. previous-project in the sidebar and previous-process in the terminal both Ctrl+ArrowUp) is allowed (Solo: "Process and project shortcuts can share the same key"). System shortcuts (copy/paste/quit) are OS/webview-level and not in our remappable set. Clean-room |
| Global Settings — Notifications & Account tabs (I7l/I7m) | **NOT SHOWN on camera** (the tabs exist in the bar but were never opened). Their fields are **undefined pending an owner decision** — stubbed in the UI with a "to be defined" state, **no invented fields persisted**. Account is largely N/A under D3 (no licensing); a proposed Soloist use (app info / data dir / reset / export-import) needs an explicit decision before building. Recorded so nothing is fabricated |
| Per-project settings — fields & defaults (I7a–I7e) | Source: the demo's project-settings tour (Aaron Francis, `youtube.com/watch?v=kVyFCcP6B28`, **4:06–4:54**). The page composes shared `solo.yml` config (C1) with app-local `ProjectSettings` (the `SettingsStore<ProjectId, ProjectSettings>` surface — the "Settings base" row above). App-local fields + our defaults: an **auto-start gate** (off by default — a project-level suppressor that, when engaged, stops the project's whole stack auto-starting on open; off preserves each command's own `auto_start`); an **editor override** (`None` → falls back to the global Tools default through one resolver, single source); **crash & exit alerts** (on); **terminal alerts** (on, with per-command overrides — an absent command follows the project default). The icon is the shared `solo.yml` `icon:` and rejects `.svg` (allow png/jpg/gif/ico/webp), per the video. The video confirms the controls and the alerts-on / gate-off defaults; the gate's polarity and the per-command override model are ours (clean-room) |
| Per-project local commands & the shared⇄local move (I7e) | A command is either **shared** (in `solo.yml`, `Visibility::Shared`) or **local** (app-state only, `Visibility::Local`, kept in `ProjectSettings.local_commands` and **never** written to `solo.yml`). "Make local" / "Save to solo.yml" **moves** a command between the two stores via one core method that adds to the destination **before** removing from the source and rolls back on failure — so a command is never lost and the two stores never both hold it after a move (no duplicate, no copy). A shared add/edit re-trusts via the C1 trust gate; a local command needs no `solo.yml` write. Ours (clean-room) |
| Programmatic `solo.yml` write (I7d/I7e) | Solo does not document how it edits `solo.yml`; ours (clean-room), chosen for stability. Shared command create/edit/rename/delete route through the **C1 config write path**: the file is edited **in place** so the user's comments, ordering, and formatting are preserved, then the result is **re-parsed and verified to equal the intended config** — if the in-place edit did not reproduce it exactly (an unusual layout, an inline mapping, or a quoted key the minimal editor cannot handle), a faithful full render is written instead, **preserving the file's leading comment block and never injecting Soloist's own header** into a file the user wrote. The write is **atomic** (temp file + rename, so a crash never leaves a half-written file) and can **never corrupt the file or write anything but the intended config**. After a write, sync state is refreshed to the written bytes so the file watcher's debounced re-read of our own write is a no-op (hash matches), and a trust-aware `ConfigChanged` is published. The one limit: inline comments **inside a single edited entry** (or a rename that also edits that entry's spec) are not preserved — that entry's fields are re-rendered — while comments on every other entry and the file header survive |
| "Automatically trust command changes" — default & scope (A8) | Solo documents the setting re-trusts a command on sync **only** when the change came from a user action that creates/saves it (§4), but documents neither its **default** nor whether it is one global switch or per-project. Ours (clean-room, decided 2026-06-29): a **per-project** setting (`ProjectSettings.auto_trust_command_changes`), **OFF by default** — trust stays explicit unless the user opts in, the safer security posture. It is honored only on the **user-initiated** C1 config write path (`Facade::write_shared_command` → auto-trust the pending variant via the same `trust + supervisor.mark_trusted` path as an explicit grant); an external `solo.yml` edit arrives via `ConfigEngine::sync`, which holds no settings and never trusts, so a change made outside Soloist still requires explicit trust. The auto-trusted variant is the existing A6/A7 variant (command+working_dir+env, D-1). Ours |
| `solo.yml` editor JSON Schema (A5) | Solo documents no schema. Ours (clean-room): a JSON Schema **generated from the `SoloYml` model** via `schemars` (off-by-default `schema` feature so the shipped binary carries no schema-gen dep), committed as `solo.schema.json` at the repo root and kept in step with the model by a drift-guard test (CI + `just lint`). A generated starter `solo.yml` carries a `# yaml-language-server: $schema=…` modeline pointing at that file's canonical raw URL so editors validate/autocomplete it; the URL is a **forward reference** that resolves once the repo (or that file) is published, harmless until then (editors skip a schema they cannot fetch). Ours |
| "Resume last session" for a stopped agent (B9) | Solo documents that a stopped **agent** offers "Resume last session" in place of plain Start (§10, v0.8.2 §11), but not the mechanism. Ours (clean-room, decided 2026-06-29 — a `later` row pulled forward at the owner's request, like A5/A8): resume **relaunches the resting process with its provider's documented resume-last invocation** instead of the fresh command, so the agent CLI reopens the most recent conversation in the project directory. This works **without Soloist tracking any session id** — every supported CLI resumes by "most recent conversation in the current working directory", and the agent process already pins its working dir to the project root; we relay the CLI's own semantics rather than reimplement session storage. The per-provider invocation is a **Strategy** (`core::agents::resume`, one cited arm per provider, mirroring the idle FSM) — the single place that knows how each provider resumes: **Claude** `--continue` ([code.claude.com/docs/en/cli-reference](https://code.claude.com/docs/en/cli-reference)), **Codex** `resume --last` ([developers.openai.com/codex/cli/reference](https://developers.openai.com/codex/cli/reference)), **Gemini** `--resume` ([github.com/google-gemini/gemini-cli](https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/session-management.md)), **OpenCode** `--continue` ([opencode.ai/docs/cli](https://opencode.ai/docs/cli/)), **Copilot** `--continue` ([docs.github.com/copilot](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/use-copilot-cli)), **Kimi** `--continue` ([moonshotai.github.io/kimi-cli](https://moonshotai.github.io/kimi-cli/)). **Gaps (no fabricated flag):** **Amp** resumes only by an explicit thread id (`amp threads continue <id>`, [ampcode.com/manual](https://ampcode.com/manual)) which Soloist does not capture, and **Generic** is user-configured — neither offers resume. The resume command is composed **once at launch** (same extra args as the fresh launch) and stored on the registry entry as an opaque alternate command; the supervisor replays it via `Supervisor::resume`, leaving the fresh command intact so **Start (fresh) and Resume (continue) are both offered** for a stopped resumable agent (a faithful superset of Solo's "Start *or* Resume"). Resume scope is **within the app run** (processes are in-memory; closing Soloist stops all — §10); the agent CLI's own on-disk history is what `--continue` reopens. Surfaced on `ProcessView.resumable`; offering a Resume control beside Start is recorded in `KNOWN-DIVERGENCES.md` D-9. Ours (clean-room) |
| Storage layer | **SQLite** (todos/scratchpads/KV/locks/trust/settings/project-settings); runtime process state in memory |
| Terminfo / `TERM` | `TERM=xterm-256color` (no custom terminfo) |
| Linux packaging floor (J1/J2) | Solo ships a macOS `.dmg` (§11); our Linux packaging is ours. Both the `.deb` and the `.AppImage` are **x86_64, Ubuntu 22.04+**. D2's 20.04 floor proved infeasible (Tauri v2 needs WebKitGTK 4.1, absent on 20.04; a 22.04 build's glibc-2.33+ libs do not run on 20.04). `KNOWN-DIVERGENCES` D-11 |
| Desktop entry & `solo.yml` association (J3) | Our (clean-room): the `.deb` ships a `.desktop` (`Categories=Development;Utility;`, `StartupWMClass=soloist`), hicolor icons, and a custom MIME type **`application/vnd.soloist.project+yaml`** matched to the glob `solo.yml` (a shared-mime-info package + a postinst/postrm refreshing the desktop/MIME/icon caches). Opening a `solo.yml` (or a folder argument) launches Soloist and opens that project via the one core `load_project`, behind single-instance (a second launch focuses the running app). Solo documents `solo://` deep links (§10), not an OS file association — the MIME type, glob, and argv-open are ours |
| System tray & launch-on-login (Phase 12) | Our: a status-tray icon (Solo has a tray bell, §10) with Show / **Start on login** (opt-in, off by default) / **Check for Updates…** / Quit. Quit routes through the deterministic shutdown (reaps every process group). All app-shell, no domain logic; autostart via the OS `~/.config/autostart` entry |
| In-app updater (J4) | Solo documents an in-app "Check for updates" (backend unnamed, §11). Ours (clean-room): **disabled by default** — never auto-checks; the tray's manual check is the only trigger. Updates are **minisign-signed** (public key in config, private key a release secret) and verified before install; the feed is a static `latest.json` on GitHub Releases; on Linux the updater replaces the **AppImage** (`.deb` updates via apt). On a private repo the feed needs auth until the repo/releases are public |
| Checksums (J5) | Our: the tag-driven release pipeline emits **SHA-256** sums (`SHA256SUMS`) for each artifact, attached to the GitHub Release |
| `quick_actions` palette behavior (I6) | Solo lists `Cmd+P` as "Quick actions" in §10 (keyboard-shortcuts page) but **never describes what it contains** — the behavior is not shown on camera and not in the docs. Our explicit decision: **process control palette for the active project** — all processes in whichever project currently has a terminal, settings pane, or orchestration view open, filtered by name, with status-aware actions (Start / Stop / Restart / Resume / Trust). This is intentionally distinct from the command palette (Ctrl+K, I2), which handles app-wide actions. Ours (clean-room) |
| `quick_jump` scope (I3) | Solo's `Cmd+E` is described as a "jump to any destination" covering processes, projects, todos, and scratchpads. Our v1 scope: **processes + projects only** — todos and scratchpads require a per-project `orchestration_snapshot` call that is not pre-loaded at the App shell level, so fetching them on palette open would add async cost. The limitation is recorded in `KNOWN-DIVERGENCES.md` and can be lifted once those data sources are promoted to the App-level store. Ours (partial parity, scoped) |
| MCP Setup/Support tools (F12) | Solo documents the tool names (§7): `help`, `submit_solo_feedback`, `setup_agent_integration` (writes Solo MCP docs into `AGENTS.md`/`CLAUDE.md`); the schemas and semantics are ours (clean-room). **`help`** answers straight from the core's embedded guide in the `soloist-mcp` binary — **no app round-trip** — so it works even while the app is down (exactly when an agent most needs it). It is **topic-structured** (a compact overview with no argument; one topic by key or alias with `help(topic=…)`) — see the "MCP `help` topics" row below — and single-source holds because the mcp crate and the `setup_agent_integration` file section both render the one `core::support` guide. **`submit_solo_feedback`** keeps Solo's tool name for interop but **stores the message locally** in the app's SQLite (`feedback` table; trimmed, non-empty, capped at 4,000 chars, wall-clock stamped) and is never transmitted anywhere — recorded as D-13 in `KNOWN-DIVERGENCES.md`. **`setup_agent_integration`** writes the same guide into the **effective project root** as a **marker-delimited managed section** (`<!-- soloist:integration-guide:begin/end -->`): the file is created if missing, appended if unmarked, and the marked span is replaced in place on re-run (idempotent; the write is temp-file + rename so the user's file is never left truncated); the `file` arg chooses `agents_md` (the default) or `claude_md`, and only those two fixed names in the project root are ever writable. Setup/Support is a **core group, always served** (§7) |
| MCP setup snippets (F2) | Solo generates setup snippets for Claude Code, Cursor, Windsurf, Cline, Claude Desktop, with a non-default data dir adding `SOLOTERM_APP_DATA_DIR` (§7); the snippet shapes and client set are ours (clean-room). Ours: the Settings → Integrations panel generates a per-client snippet from a data-driven client table (one row per client: label, config-file location, shape renderer), for **Claude Code, Codex, Amp, OpenCode, Cursor, Windsurf, Cline** — each shape verified against the client's official docs (see `docs/mcp-setup.md` for sources). **Claude Desktop is deliberately absent**: it ships for macOS/Windows only and a stdio server runs beside the client, so no working configuration exists against a Linux-only Soloist (D2). The snippet's command is resolved app-side (`mcp_setup_info`): the absolute path of the `soloist-mcp` **sibling of the app binary** when present, else the bare name (PATH lookup) until packaging bundles the helper. An `env` entry carrying **`SOLOIST_APP_DATA_DIR`** (single-sourced as `soloist_ipc::DATA_DIR_ENV`) is emitted **only when the variable is set** for the app — required, since the MCP host launches the helper with its own environment and the helper would otherwise resolve a different socket. JSON snippets are serialized (never string-built), so paths always escape validly |
| Prompt-template MCP tools (F14) | Solo's changelog documents the feature (v0.8.2: a prompt-templates view + optional MCP tools, §10) but not the tool names, schemas, or semantics — all ours (clean-room). Ours: six tools, `prompt_template_list`/`_read`/`_create`/`_update`/`_delete`/`_export`, served as a **feature group that defaults OFF** (`McpToolGroups.prompt_templates`, toggled in Settings → Integrations like Key-Value). A template = name (the addressing handle, **unique per scope**) + optional description + body, capped at **64 KiB**; **placeholders are derived from the body, never stored** — `{{name}}` markers, scanned left-to-right (first `}}` closes), inner text trimmed, a candidate that is empty or still contains a brace/newline is plain text (its span consumed). **Scope**: `project` (default — the session's effective project) or `global` (cross-project; the one C6 state with no project key); the same name may exist in both; an unscoped `list` merges global + effective-project rows and never fails on scope. **Updates are revision-guarded** (scratchpad-style optimistic concurrency; create = expected-absent). `_export` returns a portable envelope `{format: "soloist.prompt-template/v1", name, description, body}` that re-creates via `_create` (no file write). Storage: `prompt_templates` table, nullable `project_id` (cascade), name-per-scope uniqueness via a **`COALESCE(project_id, 0)` unique expression index** — NULLs are distinct inside a SQLite UNIQUE constraint, so a plain `UNIQUE(project_id, name)` would allow duplicate global names. The I13 prompt-templates *view* stays `later`; these tools and the storage are the base it will reuse |
| MCP `help` topics & the guide's auto-bind fix (F12) | **Source: Aaron Francis, `x.com/aarondfrancis/status/2075571055041675691` (2026-07-10) — Solo's progressive-disclosure MCP write-up, post-v0.8.2 primary evidence.** Solo's `help` returns a compact capability overview with no argument and topic guidance for `help(topic=…)`, with aliases (`ports`, `services`, `status`, "how do I", `yaml`) routing to the right topic; the mechanics are ours (clean-room). Ours: the one `core::support` guide is a **topic set** — `help_overview()` (the compact menu + first-run path), `help_topic(query)` (one topic resolved by key or alias, normalized so `how do I`/`how-do-i`/`How Do I` all match), and `agent_guide()` (every topic concatenated — still the single source the `setup_agent_integration` file section renders). The `help` tool takes an optional `topic`; an unknown topic falls back to the overview with the query echoed, never an error. **Bug fix (same change):** the identity topic now teaches that a managed session **binds automatically on connect** (the `soloist-mcp` client sends the bind) and names `register_agent` for external callers — the earlier guide told agents to *call* `bind_session_process`, which is **not** an MCP tool (Soloist exposes no manual bind), so an agent following it would have called a nonexistent tool. Ours (clean-room) |
| MCP initialization instructions & server identity | **Source: same tweet (2026-07-10).** Solo reinforces the first-run path in the MCP server's **initialization instructions** (1. `whoami` 2. `help` 3. `help` on a topic). Ours: `SoloistMcp::get_info` returns a `ServerInfo` carrying those instructions (single-sourced as `core::support::onboarding_hint()`), advertises the **tools capability** (`enable_tools`), and identifies the server as **`soloist-mcp`** + its version — the rmcp default reports the `rmcp` crate. Ours (clean-room) |
| MCP `whoami` payload | **Source: the tweet's screenshot (2026-07-10).** Solo's `whoami` reports the process id/name/kind/status, the actor, the project id/name, the detected vs effective project, the session id, and an `mcp_tools` block (enabled + server-side enabled-tool count + a visibility note). Ours (clean-room): the core `Whoami` is enriched from bare ids — `bound_process`/`selected_process` are the canonical `ProcessView` projection (id, label, kind, status, …) and `effective_project` is a lean `ProjectRef { id, name }` resolved by the same display-name rule the UI uses; `origin` conveys the actor (bound process / external label / unbound). The `soloist-mcp` `whoami` tool attaches the **`mcp_tools`** block (`enabled: true`, `server_enabled_tool_count` = the server's own composed router size, a `visibility_note`) since the count is a fact of the MCP surface, not the core. **Deliberate divergences:** Soloist does not surface the **OS pid** (the agent knows its own; `ProcessView` does not carry it), and it does not duplicate the `project_id`/`detected_process_id` fields that equal `effective_project`/`bound_process.id` for an authenticated caller — the enriched fields carry the same information without the redundancy. Ours (clean-room) |
| MCP `mcp_tools_summary` | **Source: same tweet (2026-07-10).** Solo's `mcp_tools_summary` returns a compact, categorized list of the currently-enabled tools without dumping their input schemas. Ours (clean-room): a Setup/Support tool returning `{ tool_count, categories: [{ category, tools: [{ name, summary }] }] }`, where `summary` is the first sentence of the tool's description (no input schema). The categories are built from the **same per-category sub-routers the server composes**, then filtered to the tools actually served, so a disabled feature group's tools drop out and the summary can never name a tool the server does not define (single source, no hand-kept parallel list). Added to the `EXPECTED_TOOL_SURFACE` guard. Ours (clean-room) |
| MCP `tools/list` ordering (featured tools) | **Source: same tweet (2026-07-10).** Solo relies on `tools/list` being ordered and, **when it detects the caller runs inside Solo**, reorders so `whoami` and `help` come first, followed by a small starter pack. rmcp lists tools **alphabetically** (`ToolRouter::list_all` sorts by name), burying `whoami`. Ours (clean-room): `SoloistMcp::list_tools` surfaces a featured starter pack first — `whoami`, `help`, `mcp_tools_summary`, then the common read/act tools (`list_processes`, `get_process_status`, `get_process_output`, `start_process`, `restart_process`, `send_input`) — then every remaining tool in the default order; the full surface is unchanged, only its order (a test guards that reordering neither drops nor duplicates a tool, and that every featured name is served). **Divergence:** we feature the pack **unconditionally** (Solo reorders only when it detects it launched the caller) — featuring is harmless for an external agent and still guides it, and a client that ignores server order is unaffected. Ours (clean-room) |
| MCP next-tool suggestions (decaying) | **Source: same tweet (2026-07-10).** Some Solo tool results include a contextual suggestion for the logical next tool ("you spawned a process — monitor it with a timer"), and the suggestions **decay after being shown** so they stop spending tokens. Ours (clean-room): `SoloistMcp::call_tool` appends a next-tool hint (an extra text content block, leaving the structured data intact) to a **successful** result when a suggestion applies, drawn from a single `hint_for(tool)` catalog (spawn→timer, start/restart→wait-for-port, send_input→read-output/timer, lock_acquire→release, todo_create→lock, scratchpad_write→lease+revision, register_agent→whoami/select). A per-session ledger caps each suggestion at **2 shows** then falls silent (one MCP connection is one session, so the ledger lives with the handler). Ours (clean-room) |
| Per-individual-tool MCP disable | **Source: same tweet (2026-07-10) — "In Solo you can turn off specific MCP tools. Disabled features are removed from tool discovery entirely."** Soloist already gates the MCP surface at the **feature-group** level (G10 above): a disabled group's tools are absent from `list_tools` and uncallable — which delivers the observable behavior ("removed from tool discovery entirely"). **Decision (owner-confirmed 2026-07-12): keep group-level; per-individual-tool disable is not built** — it would add an individual-tool set to settings (schema + migration + UI) beyond the matrix's v1 scope. Tracked as a *later* refinement. Ours (clean-room) |

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
