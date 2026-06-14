# Phase 5 — Dashboard UI

**Goal:** The visible app. Wire React to the core via Tauri commands/events: the **sidebar process tree**
(grouped Agents/Terminals/Commands), status indicators, the **terminal pane**, the **trust review**
dialog, and the **orphan** dialog. After this phase Soloist is usable by a human end-to-end.

**Delivers:** I1, B-/C-/A-series UI (B2,B3,B4 controls; C1–C7 terminal; A6/A9 trust dialog; B8 orphan
dialog). **Architecture:** Tauri adapter only — no business logic in React (`04` §2/§10).

## Scope
**In:** the Tauri command/event surface; sidebar tree; status dots; per-process + bulk controls;
terminal pane (xterm.js); trust-review + sync-diff dialog; orphan dialog; basic project load/switch.
**Out:** command palette/themes/settings/execution profiles (Phase 11); metrics display (Phase 6 adds
data); coordination panels (Phase 9).

## Tauri surface (adapter over the core)
```
commands: stack_start/stack_stop, proc_start/stop/restart(id), proc_list,
          logs_get(id,limit), pty_write(id,bytes), pty_resize(id,c,r),
          project_load(path), project_switch(id), config_trust(project,variant),
          orphans_resolve(decision)
events:   proc:status, proc:log, pty:<id>, term:title, term:bell,
          config:changed{diff,requires_trust}, orphans:found
```

## UI layout (ref §10)
```
┌───────────────────────────────────────────────────────────┐
│ [project ▾]  ▶ Start all  ■ Stop all  ⟳ Restart running  ⚙ │
├──────────────┬────────────────────────────────────────────┤
│ ▾ Agents     │  ┌ Terminal | Logs ───────────────────────┐ │
│   ● claude   │  │ xterm.js for the selected process       │ │
│ ▾ Terminals  │  │                                         │ │
│   ● zsh      │  └─────────────────────────────────────────┘ │
│ ▾ Commands   │   selected: web   ▶ ⟳ ■   status: ● Running  │
│   ● web ○ db │                                              │
└──────────────┴────────────────────────────────────────────┘
```

## Tasks
1. **api.ts + event store:** typed wrappers + `listen` subscriptions; a small reducer turning events
   into a read-model (TanStack Query for command results; event store for live status/logs).
2. **Sidebar tree (I1):** groups **Agents / Terminals / Commands**, collapsible (persist per project),
   drag-reorder; status dot per row (green=Running, red=Crashed, amber=Starting/Restarting,
   grey=Stopped, distinct for RestartExhausted); per-row ▶/⟳/■.
3. **Bulk controls:** Start-all / Stop-all / Restart-running in the top bar (header keys S/A/P/R in
   Phase 11), disabled appropriately during transitions.
4. **Terminal pane:** xterm.js (Phase 4 streams); on select → `pty_scrollback` replay then subscribe
   `pty:<id>`; send keystrokes via `pty_write`; `pty_resize` on mount/resize; Logs tab tails `proc:log`
   (ANSI-aware, search, pause-on-scroll).
5. **Trust review dialog (A6,A9, ref §4):** on `config:changed{requires_trust}` show the command +
   working_dir + env + the add/update/remove diff; "Trust this command" / "Trust all" →
   `config_trust`. Start controls disabled for untrusted commands.
6. **Orphan dialog (B8):** on `orphans:found` show Kill / Kill All / Leave running → `orphans_resolve`.
7. **Live status:** subscribe `proc:status`; update dots without refetching the list.
8. **Empty/error states:** no `solo.yml` → guidance; command failure → toast with the core's error.
9. **Project load/switch:** load a `solo.yml` from a folder; switch loads that project's stack (full
   switcher polish in Phase 11).

## Acceptance criteria
- Launch with a sample `solo.yml`; the sidebar shows processes grouped by subtype as `Stopped`.
- "Start all" turns dots green as commands reach Running; logs stream.
- Selecting a process shows its live terminal; typing reaches it (answer a `read`/agent prompt from the
  GUI).
- Per-row stop/restart reflect within ~1 s; RestartExhausted is visually distinct.
- Editing `solo.yml` pops the trust/sync dialog with a correct diff; Start blocked until trusted.
- Closing the app stops all processes (no orphans) — asserted via the Phase 3 pgroup test.

## Test plan
- **Playwright (webapp-testing):** fixture `solo.yml` with deterministic processes; assert grouping, dot
  colors, log text, terminal echo of typed input, trust dialog blocking Start until trusted, orphan
  dialog actions.
- **Manual:** run a real stack (Vite dev server + `claude`) and interact.

## Risks & mitigations
- **WebGL on WebKitGTK** → feature-detect; canvas fallback.
- **Event flood from chatty processes** → coalesce `pty:<id>` per animation frame; cap log event rate;
  ring buffer is source of truth (`04` §8).
- **UI/core state drift** → re-`proc_list()` on focus/reconnect; events are deltas over that snapshot.

## Effort
~5–7 days.
