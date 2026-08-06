# e2e-01 — Screens, Flows & the First Journey

**Status: built and green (2026-07-16).** The three-layer architecture exists and carries its first
journey: opening a project, launching Claude into it, and asserting the app really starts it and
renders it. That journey is 4 specs; the suite it seeded is **18 specs / 6 files / ~57 s** as of
2026-07-17. What follows describes what exists; the remaining catalog walks (charter §4) are e2e-02+.

**Goal:** Turn the proven harness into a **reusable architecture**, and prove it carries real behavior by
landing a real journey. This is the phase that decides what every later spec looks like, so its output is
a pattern as much as a test. Read [`README.md`](README.md) §2–§3 (scope + architecture) first.

**Delivers:** `src/screens/`, `src/flows/`, `src/harness/`, the domain `specs/` tree, and the
launch-an-agent journey. **No product code.**

## Scope

**In:** the three-layer harness architecture; the screen objects the first journey needs; the
Dashboard-core walk (charter §4 row 1).

**Out:** the remaining catalog walks (e2e-02+, one per row, independent once this lands); backend command
mocking (charter §1.2); re-asserting logic the headless suites already own (charter §2).

## The layer rule (the point of this phase)

```
specs → flows → screens → harness
```

- **`specs/`** — *what the user does*. Reads as a user journey. **Never** contains a selector, a wait, a
  path, or an `import` from `harness/`.
- **`flows/`** — reusable journeys spanning more than one screen (`trustProject`, `startProcess`,
  `launchAgent`). A flow is extracted when a second spec needs the same sequence — not before.
- **`screens/`** — the **only** place selectors live. One object per UI surface, mirroring
  `crates/app/ui/src/components/`. A selector for a surface exists exactly once (CLAUDE.md §15).
- **`harness/`** — app lifecycle, data dir, fixture materialization, waits. The only layer that knows
  about the filesystem or the process.

If a spec reads like a script of clicks and selectors, the phase failed. It should read like the catalog
row it implements.

## What each layer holds

1. **`src/harness/`** — `fixtureProject.ts` copies `fixtures/projects/<name>` into a scratch dir and
   returns its path, so a spec names a fixture and never a path (and never dirties the checked-in
   fixture, which opening a project would write to). `waits.ts` holds the two named timeouts — a local
   render, and a round trip through the real core — so no spec carries a bare number. `tauri.ts` is the
   app's own IPC, for **arrange only**.

2. **`src/screens/`** — `Sidebar`, `AgentPicker`, `Titlebar`, `TerminalPane`. Each exposes
   intent-shaped methods and **queries by accessible name** where one exists (`$('aria/Launch agent')`),
   per charter §3.2. Where one does not, prefer a **structural** handle over a styling one: the sidebar
   reads a row's label as the direct child span carrying none of the indicator's markers, which
   survives a restyle and breaks only if the row stops rendering a label.
   **The product-code exception, unused so far:** if a surface genuinely has no handle, add an
   `aria-label` **to the component** via `/impeccable` (CLAUDE.md §5) — improving the real app's
   accessibility. Never a test-only hack, never a brittle CSS selector as a workaround.

3. **`specs/` domain tree** — the directories from charter §3, each arriving with its first spec.
   **Named for what they are**: no parity letters or phase numbers in directories, filenames, or test
   titles (CLAUDE.md §8).

4. **The launch-an-agent journey (`specs/agents/`)** — open a project, and:
   - the picker targets that project and offers Claude with the command it would spawn;
   - launching renders the agent in the sidebar, labelled and selected, under Agents;
   - the app **actually starts it** — the status settles `Running`;
   - a terminal opens for it, mounted and measured non-zero.

   Assert on `ProcStatus` imported from the UI's `domain.ts` via the `@domain` alias — never a literal.

5. **Traceability** lives in charter §4 and `PROGRESS.md` — **not** in a code comment (CLAUDE.md §8).

## The journey never runs a real agent

What launches is a **stub `claude`** (`fixtures/bin/claude`) the harness prepends to `PATH` in
`wdio.conf.ts`: it answers the `--version` detection probe and otherwise stays alive like a real
agent. That is charter §3.1's hermeticity lever applied to agents — the journey behaves identically
on a developer's box (where a real Claude would otherwise launch with a real session) and in CI
(where none exists), which is also what lets the spec assert the launch **settles `Running`**
rather than the weaker "left `Stopped`" an environment-dependent agent forced.

One thing the journey still deliberately does not assert: **the terminal header's exact text**. The
header shows the process's label until the process retitles itself over OSC; the assertion is
containment, identifying the process without pinning the header's surrounding layout.

## Interfaces

```
e2e/
├── src/
│   ├── harness/
│   │   ├── fixtureProject.ts # fixture name → a clean scratch copy's path
│   │   ├── tauri.ts          # the app's own IPC — arrange steps only
│   │   └── waits.ts          # the named timeouts; no sleeps
│   ├── screens/              # the only place selectors live
│   │   ├── Sidebar.ts        # rows: label, status, selection, discovered port;
│   │   │                     #   select/trust/start/stop/restart, stopIfRunning cleanup
│   │   ├── AgentPicker.ts    # target project, tools, choose
│   │   ├── Titlebar.ts       # launch agent, open-project affordance
│   │   └── TerminalPane.ts   # title, mounted, measured size
│   └── flows/
│       ├── openProject.ts    # materialize fixture → load → shown
│       └── launchAgent.ts    # picker → choose → row appears
├── fixtures/bin/             # stub agent CLIs, shadowing real ones on PATH
└── specs/
    ├── agents/               # the launch-an-agent journey
    └── supervision/          # trust → start → crash/stop, via the row's own controls
```

Later phases add screens as their walk needs them (`ProcessControls`, `TrustDialog`, `OrphanDialog`,
`CommandPalette`, `SettingsOverlay`, `ProjectSettingsPane`, `OrchestrationPane`) — each when its
trigger fires, never speculatively (CLAUDE.md §16, YAGNI).

**One arrange step is not a click, and cannot be.** Opening a project goes through the OS folder
dialog, which a WebDriver session cannot drive. `harness/tauri.ts` calls the same core command that
dialog's handler calls; nothing else uses it, and every assertion stays on what the window renders.
Reaching for it to *act* rather than to arrange is the line not to cross.

**Reads are atomic.** The sidebar snapshots its rows in one pass rather than walking them element by
element: a live agent re-renders its row as its activity changes, and a row-at-a-time walk races that
re-render and dies on a stale element reference — a flake for a reason unrelated to the assertion.

## Acceptance criteria

- ✅ The journey passes locally (`just e2e`) and headless in CI (PR #74's `e2e` job under `xvfb-run`).
- ✅ **No selector appears in a spec** — they live only in `src/screens/`.
- ✅ **No `sleep`** anywhere in `e2e/`.
- ✅ No status literal in a spec: `ProcStatus` is imported from the UI's `domain.ts` via the `@domain`
  alias, so a renamed variant is a type error rather than a silently-passing string.
- ✅ Hermetic: each run wipes its data dir and copies the fixture to scratch; the developer's real
  Soloist state is never read or written.
- ✅ `just lint` / `just test` unaffected.

## Test plan — the journey must fail for a real reason

A spec that cannot fail is a pretend test (CLAUDE.md §15), and a green e2e is exactly where that hides.
Both assertions were confirmed by mutating the **product** and watching the right test go red:

| Mutation | Expected | Observed |
|----------|----------|----------|
| Drop `supervisor.start(id)` in `facade.rs` — register the agent, never run it | only "actually starts the agent's process" fails | exactly that; the other three still passed, because the row *is* still drawn |
| Render `{process.label + "X"}` in `ProcessRow` | the label assertions fail | "renders the agent…" and "actually starts…" failed, naming the rendered rows |

Repeat this whenever a walk lands. The first mutation is the one that matters: without it, "renders the
agent in the sidebar" passes against a row that was merely painted.

The supervision walk's product-mutation pass is done. It caught real defects the harder way at
landing too (`ProjectService::open` duplicated command registrations on re-open — fixed with a unit
test — and a real product gap: no `solo.yml` watcher, charter §4). Choosing the mutation matters
here: every spec's cleanup after-hook stops its process (`sidebar.stopIfRunning` waits for
`Stopped`) and three of the four tests start one, so breaking `Supervisor::start` or
`Supervisor::stop` is *not* surgical — it cascades into other walks' cleanup and no single walk
fails alone. The restart signal is surgical: it drives exactly one assertion and no cleanup, and
that assertion is the walk's most deliberately-robust one — the reborn process's *changed* ephemeral
port, which no repaint can fake.

| Mutation | Expected | Observed |
|----------|----------|----------|
| Comment out the `ActorMsg::Restart` signal in `Supervisor::restart` — a running restart no-ops | only "restart replaces the process, not just the row" fails | exactly that: `Listener never showed a port other than :41723`; the walk's other three (start→Running, stop→Stopped, crash→Crashed) and all three other spec files passed |

(start→Running needs no separate supervision mutation — the agents walk's `supervisor.start(id)`
mutation above already proves it load-bearing.)

The cross-surface walk (`specs/cross-surface/cli-restart.spec.ts`) is proven the same way, with the
mutation chosen so that a *successful* CLI call is not enough to pass:

| Mutation | Expected | Observed |
|----------|----------|----------|
| Make the HTTP `restart` handler in `crates/httpapi/src/mutations.rs` return `200 OK` without calling the core | only the CLI-restart walk fails; the CLI still reports success | exactly that: `sidebar row "Listener" never showed a port other than :37583`, and the other four spec files passed. Surgical because no other spec drives HTTP and no cleanup path uses it — unlike `Supervisor::restart`, which the supervision walk shares |

It earned its keep beyond the assertion: building it exposed a harness defect that had been deleting
every app's database, socket, and HTTP runtime file ~3 s after boot, in every run since the wipe
landed (e2e-00). Three walks and a CI run had stayed green over it, because an open SQLite handle
survives an unlinked inode — nothing had needed a real file on disk until a second binary had to
find the app.

The trust-review walk (`specs/projects/config-trust.spec.ts`) earned its keep the same way: building it caught
a second real defect. At the time, every e2e session shared one durable data dir (the app inherits the
*launcher's* environment, whose `SOLOIST_APP_DATA_DIR` was the module-load default, so every session
resolved to that one path for the run), so `basic` was already registered — and its root already watched — when
`materializeProject` deleted and recreated the fixture directory for this spec, giving it a new inode. The
config watcher held its now-dead watch on the vanished inode and never saw the edit. The fix is a genuine
robustness improvement, not a test accommodation: **`ConfigWatchReactor` now re-establishes a project's
watch on every `ProjectOpened`**, since re-opening a path is exactly when its directory may have been
replaced (unit test `reopening_a_project_re_establishes_its_watch`). When that defect was found the
harness still shared one durable data dir, so the walk re-opened an already-watched `basic` and exercised
the re-watch end to end. That is no longer how the suite runs: `wdio.conf.ts` now gives each session its
own data dir (`onWorkerStart`), so each spec boots a clean app and config-trust opens `basic` for the first
time — it never re-opens a swapped-inode project. So the re-watch's load-bearing proof today is its *unit
test*, not the e2e (the second mutation below shows exactly this).

Its product-mutation pass is done:

| Mutation | Expected | Observed |
|----------|----------|----------|
| Comment out the `config_watch_loop()` spawn in `crates/app/src/lib.rs` — the watcher never runs | the walk's assertions fail; the other walks hold | exactly that: "the trust review dialog never opened" (test 1) and Trust-Echo-not-clickable (test 2, its consequence); agents, smoke, and supervision all passed — nothing else raises that dialog. Proves the whole watcher → debounce → reload → `ConfigChanged{requires_trust}` → dialog chain load-bearing |
| Drop the `ProjectOpened` re-watch (`watches.remove(&id)`) in `ConfigWatchReactor` | (at landing) the walk regresses; (now) the unit test regresses | the e2e stayed green — the per-session data dir isolates each spec's app, so nothing re-opens a watched project — while the unit test `reopening_a_project_re_establishes_its_watch` deadlocks (the re-open never re-establishes the watch), passing again once restored. The proof moved from the e2e to the unit test; the fix stayed load-bearing |

The agent-lineage walk (`specs/orchestration/agent-lineage.spec.ts`) drives the orchestration tree against a
genuine lead→worker lineage. A new fixture is the foundation: a stand-in **lead** agent (`fixtures/lead-agent/`,
its own workspace, built by `onPrepare`) that, launched by the app as an agent, reaches the app's real IPC
socket, binds its session to its own process (authenticated by its process group, exactly as `soloist-mcp`
does), and `spawn_agent`s a worker over the same wire — so lineage is recorded by the real core, not staged. A
stub **worker** (`fixtures/bin/opencode`, OutputDelta idle heuristic) cycles output then quiet so the idle
sampler drives a deterministic Working→Idle flip. Single-agent removal is reachable only through a bound
MCP/IPC session (never the local UI, HTTP, or CLI — a product finding, below), so the re-root is triggered
cross-surface: the lead stub closes its own bound process on a trigger file, and the window reflects the
re-root. Its product-mutation pass is done:

| Mutation | Expected | Observed |
|----------|----------|----------|
| Comment out `self.inner.lineage.record(worker, lead)` in `crates/core/src/facade/scoped_process.rs` — a bound-lead spawn records no parent | only the walk's nesting assertions fail; the other walks hold | exactly that: "nests a spawned worker under the lead" failed (`worker.parent` expected `1`, got `null` — the worker read back as a root) and "re-roots a closed lead's workers" failed at its pre-close nesting guard; the walk's lineage-independent assertions (manual-root, the Working→Idle glyph flip) held, and all five other spec files (agents, cross-surface, config-trust, smoke, supervision) stayed green. Surgical: no other spec depends on lineage recording |

**Product finding (recorded, not fixed here):** removing a *single* agent from the registry — the action that
re-roots its workers — is reachable only via a bound MCP/IPC session's `close_process`. The local Tauri UI has
no per-agent close/remove affordance (stopping leaves the agent resting in the registry, so its workers stay
nested), and HTTP/CLI expose only start/stop/restart + whole-project removal. The walk therefore drives the
close cross-surface (the lead stub closes itself), mirroring the CLI-restart precedent, rather than inventing a
UI affordance the e2e track does not build.

The coordination-panels walk (`specs/coordination/coordination-panels.spec.ts`) drives the scratchpad and to-do
panels against a bound lead that writes the shared documents over the real MCP/IPC wire. It **reuses the lead
fixture** (`fixtures/lead-agent/`) via a second arm the spec selects with a dropped plan file: the lead seeds a
scratchpad, a blocker chain, and a comment as a genuinely bound agent, then re-writes the scratchpad on a
trigger to bump its revision under the window's stale editor — the concurrent writer the conflict needs. New
screens `ScratchpadPanel` and `TodoBoard` mirror the panels; `OrchestrationPane` gained a `showView` for the
pane's segmented view switch. The `solo://` copy-link is covered **partially** — reading the system clipboard
under WebKitGTK/WebDriver would need a test-only hack (no first-class WebdriverIO clipboard API; clipboard-read
denied under automation), so the URL construction stays headless (the core `link` tests plus new Vitest for the
UI `copyLink → writeText` wiring, `useTodoActions.test.ts` / `useScratchpadEditor.test.ts`) and only the
OS-clipboard hop is left unproven. Its product-mutation pass is done, each mutation reverted byte-clean:

| Mutation | Expected | Observed |
|----------|----------|----------|
| Drop the `revision == expected` guard in `SqliteStore::write` (`crates/store/src/scratchpads.rs`) — a stale scratchpad write is applied instead of refused | only "refuses a stale scratchpad save…" fails | exactly that: the conflict banner never appeared (`[role="alert"]` not displayed after 30 s); the walk's other two assertions and all six other spec files passed |
| Remove `self.guard_blockers(project, id)?` from `Todos::complete` (`crates/core/src/coordination/todo.rs`) — a blocked todo completes | only "refuses to complete a blocked todo…" fails | exactly that: the refusal alert never appeared; the other two and all six other files passed |
| Force the comment author to `None` in `todo_comment_create` (`crates/core/src/facade/todo.rs`) — the bound author is not stamped | only "shows the bound author of a comment" fails | exactly that: the board read "unattributed" where "Codex" was expected (the item's full text carried no "Codex" at all); the other two and all six other files passed |

Surgical because no other spec writes a scratchpad, completes a todo, or creates a comment — the mutated code
paths are coordination-only. Each mutation reddened exactly one assertion; `git diff --stat crates/` showed
none of the three files after restore.

The timers-and-wake-cycle walk (`specs/orchestration/timers-wake-cycle.spec.ts`) drives orch-03's headline
behavior — token-free waiting — against the real scheduler. It **reuses the lead fixture** via a third arm
(a dropped timer plan): the bound lead spawns a worker and arms a `fire_when_idle_all` timer over it across the
real IPC wire, then echoes its own PTY stdin so the delivered wake turn shows in its terminal (as the headless
`crates/pty/tests/orchestration.rs` `cat` lead does). The `opencode` worker gains a file-gated hold mode so the
walk drives its idle transition on cue rather than racing the natural cycle: while a hold file exists it stays
Working (the timer holds its waiting state for the panel assertion); deleting it settles the worker Idle,
firing the timer. A new `TimersPanel` screen reads the panel's accessible names (waiting-on chips, "Time
remaining" countdown); `TerminalPane` gained a viewport text read. **A product change unblocked the terminal
read:** the WebKitGTK webview runs xterm's WebGL renderer, which draws to a canvas the DOM cannot read, so the
e2e build turns on xterm's screen-reader mode (`terminalOptions`, gated on `VITE_E2E`), mirroring the live
viewport into the accessibility DOM — the real renderer stays, the shipped default stays off (guarded by
`appearance.test.ts`). Its product-mutation pass is done, both mutations in `crates/core/src/coordination/scheduler.rs`,
each reverted byte-clean:

| Mutation | Expected | Observed |
|----------|----------|----------|
| Drop the PTY delivery write in `deliver` (`try_write_stdin`) — a fired timer's body is not delivered | only "…delivers its body with the wake-reason prefix…" fails | exactly that: the timer still fired and left the panel (`TimerFired` still published), then the terminal read failed (`never showed "wake up: resume the release cut"`); the walk's waiting assertion and all seven other spec files passed |
| Force the idle quorum unmet in `is_due` (`Some(_) => false`) — a fire-when-idle timer fires only on its backstop | only the panel-clear fails, under its bounded wait | exactly that: "an armed timer never left the panel — it did not fire" (the 10-min backstop is beyond the 30 s wait), never reaching the delivery read; the waiting assertion and all seven other spec files passed |

Surgical because no other spec arms a timer or observes a wake delivery — the mutated paths are scheduler-only.

The prompt-template walk (`specs/coordination/prompt-templates.spec.ts`) drives the Templates manager's
preview against the real renderer. Its reason to exist is narrow and worth stating, because a thorough
headless test of the same behavior already exists: `TemplatePreview.test.tsx` asserts the unanswered
placeholder in jsdom, but against `coreRender`, a **hand-written stand-in for the core's contract** in the
test file. It proves the window's half given a render that behaves as we believe the core's does. The seam
it cannot reach is the one the design rides on — the window drops an emptied field from the values it sends
(`useTemplateRender`), and the core must read an absent key as unanswered rather than as an empty answer.
Only the real renderer under the real window puts both halves on the screen a user reads. Two new screens
(`SettingsOverlay`, `TemplatesPanel`) and one arrange flow; the body is authored through the core command
the create form posts, because a template body is a ProseMirror contenteditable and WebKitGTK under
WebDriver delivers none of the events it needs to accept typed characters (the same limitation
`ScratchpadPanel` works around by clicking a toolbar control). Its product-mutation pass is done, reverted
byte-clean:

| Mutation | Expected | Observed |
|----------|----------|----------|
| Drop the unanswered-placeholder `AdvisoryNotice` in `TemplatePreview.tsx` — the gap stays literal in the prompt but is never named | only "leaves an unanswered placeholder in the prompt and names it" fails | exactly that: `Expected length: 1, Received length: 0, Received array: []`. The same test's *prompt text* assertion still passed, so the mutation is pinned to the notice and the walk's two independent reports of the gap fail independently; the walk's other four assertions and all eight other spec files passed |

Surgical because no other spec opens a template — and the notice is the one thing in the preview no other
assertion reads.

The notification walks (`specs/notifications/`, three files) cover what only the window can say about an
alert: that it reaches the screen, that the app really tells the core where the user is looking, that the
unread indicators render and clear together, and that a level chosen in the settings pane is what the core
stored. Two cue-driven stubs join the `basic` fixture — a crasher and a signaller that act on a file the
spec drops — because starting a process from its row selects it, and the core suppresses an alert about the
process being watched: a signal that has to reach the user must land *after* the spec has looked elsewhere.

**Window focus is the precondition, and it is asserted rather than arranged.** `route()` sends an alert to
the desktop instead of a toast whenever the window is unfocused, so "no toast appeared" is exactly what a
lost focus produces — the one shape of green a mutation pass cannot catch, on the walk that matters most.
`harness/windowFocus.ts` therefore reads the real window and fails with an explanation. Nothing tries to
take focus, because nothing can: measured on a GNOME/Wayland session, the app is refused focus through
Tauri's own `set_focus` as readily as through `xdotool` or `wmctrl`, since Mutter owns focus for XWayland
clients. A display with nothing else on it always grants it, which is why `just e2e` now runs under
`xvfb-run` — the way CI already ran it. The suppression walk carries a second discriminator for the same
reason: it asserts the row is **not** marked unread, which separates real suppression (the reactor returns
before raising) from an alert that merely went to the desktop (which still marks it).

Its product-mutation pass is done — eight mutations, each reverted byte-clean, `git diff --stat crates/`
empty after each:

| Mutation | Expected | Observed |
|----------|----------|----------|
| Drop the `presence.viewing == Some(process)` branch in `routing.rs` — a watched process's alert is no longer suppressed | only "says nothing about a crash the user watched happen" fails | exactly that: `Received array: ["Build", "Faulty crashed"]` where the crash must not appear. All 13 other spec files passed — the other walks watch a different process, so nothing else depends on the branch |
| Drop the `!level.admits(kind.severity())` guard in `routing.rs` — the level gates nothing | only "drops a crash while the project is set to None…" fails | exactly that: `Expected: null, Received: 1` — the refused crash had left something waiting. Every other file passed; the other walks run at `All`, where the guard admits anyway |
| Remove the `b"777"` arm from `terminal/parser.rs` — a process's own notification is not recognised | the script-alert walk fails, and with it the three assertions that use that alert | `no alert titled "Build" appeared; on screen: ["Faulty crashed"]` — the crash alert still rendered, so the mutation is pinned to the OSC arm rather than to alerting. The cascade is owned: the unread-count walk and both negative walks use that alert as their sentinel, by design |
| Remove the unread dot from `ProjectGroup.tsx` — a project no longer shows its processes need attention | only "marks the crashed process's row and its project as unread" fails, at its project half | exactly that (`Expected: true, Received: false` at the project assertion); the row half and every other file passed |
| Invert the row marker in `ProcessRow.tsx` (`!unread`) — every row is marked except the ones that should be | the row-marker assertions fail | the marker walk failed at its row half, the clear-all walk found every row marked, and the suppression walk's "nothing else is marked" failed (37 marked rows against an expected none). Cascades across every assertion that reads a row marker, which is the honest scope of one marker |
| `AttentionRegistry::clear_all` returns `false` — clearing announces nothing, so no surface re-reads | only "clears every indicator at once" fails | exactly that: `the title bar still showed 2 unread`. Surgical — nothing else clears |
| Drop `select.current(process)` in `NotificationToasts.tsx` — acting on a toast no longer goes anywhere | only "takes the user to the process when its toast is acted on" fails | exactly that: `sidebar row "Faulty" never became the selected one; selected: ["Echo"]` |
| Store `NotificationLevel::None` where `set_command_notification_level` removes the entry — inheriting becomes silence | only the level-persistence walk fails | exactly that: `choosing "Same as project" left "None" chosen`. The two states that read most alike on screen are the two most easily confused in storage, and this is the assertion that tells them apart |

**Harness findings (fixed here, none a product defect).** Four, each of which had been quietly costing runs:

- **The unread marker had broken the sidebar's row read.** The marker is the row's first child span and
  carries no text, so `ROW_TEXT` matched it first and every marked row's label read back empty — the row
  then vanished from every lookup by name. `indicatorRow.ts` now excludes any span carrying a role.
- **A failed `onPrepare` build ran the suite against the previous run's binary.** WebdriverIO logs a
  rejected hook and carries on, so a broken build reports on an app that no longer exists in the tree —
  observed when a mutation left a type error and a full suite still ran, green except where the *stale*
  binary differed. `buildBinary` now ends the process instead of throwing.
- **A single click is an attempt, not an intent.** WebKitGTK under WebDriver drops a click outright when
  the app is busy; observed on a row select after a project's ••• menu, and on Start. `sidebar.select` and
  `sidebar.start` now repeat until the row reports the intended state, both bounded, and neither can mask a
  refusal — a start the core refuses is unmoved however often it is asked.
- **A failed screenshot took the page source down with it.** On a virtual display WebKitGTK can refuse a
  snapshot, and the throwing `afterTest` replaced the real failure with a hook error. Each capture is now
  independent and reports rather than throws.

**Not covered here, and not coverable:** the native desktop notification, whether a chosen bell is audible,
and the app-icon badge. All three are window-system or audio surfaces outside the page, so no WebdriverIO
assertion can reach them; a green `just e2e` is not full notification coverage.

**Harness finding (fixed here, not a product defect):** opening the agent picker was a single click and a
single 10 s wait, and it intermittently never opened — observed on `launch-agent` in one run and on
`timers-wake-cycle` in another, each time as `element ("[cmdk-root]") still not displayed`. WebKitGTK under
WebDriver drops a click outright when the app is busy, and the picker is lazy-loaded behind a deferred
overlay, so the open also waits on a chunk fetch. With no retries configured, one dropped click takes a
whole spec file with it. `flows/launchAgent.ts` now re-clicks until the picker is actually up — safe
because the titlebar action *sets* it open rather than toggling, and the re-click is skipped once it is —
the same remedy `Sidebar.openOrchestration` and `ScratchpadPanel.reload` already use. The picker-opening
step is now exported and shared, since `launch-agent.spec.ts` was carrying its own copy of the sequence
that flaked.

## Risks & mitigations

- **Screens drifting into logic** → a screen returns state and performs intent; it never asserts and never
  branches on domain rules. Assertions live in specs.
- **Premature abstraction** → extract a flow on the *second* use, not the first. Three screens is the
  right size for this phase; the rest arrive with their walks.
- **Selector churn as the UI evolves** → accessible-name selectors track what the user perceives, so they
  break when the *user-visible* thing breaks, which is the signal we want.
- **The journey re-testing headless-covered logic** → if an assertion could pass in jsdom, it belongs in
  Vitest, not here (charter §2).

## Effort

~1 day for the architecture + the first journey; each subsequent catalog walk is ~½–1 day.

## Reading a change — the mutation pass

The walk exists because the diff surface is the one place where what the *bundler* resolved decides
whether the window survives, and no headless suite can see that: Vitest resolves the viewer's
dependencies for itself, so the app's `lowlight` alias never applies under test and 19 green component
tests ran against a stand-in that could not load in a real build.

| Mutation | Expected | Observed |
|----------|----------|----------|
| Drop `register` from the `lowlight` stand-in (`lib/diff/lowlight.ts`) — the shape it shipped in | only the version-control walk fails, and it fails as an *empty window* rather than a missing diff | exactly that: both of `specs/version-control/open-diff.spec.ts` red (`element ("section[aria-label="Diff"]") still not displayed`, then the sidebar read `false`), the captured page source showing `<div id="root"></div>` with nothing in it; the other 11 non-notification spec files passed unchanged |

Surgical because the stand-in is reachable from exactly one chunk, which exactly one spec loads: no
other walk opens a diff, and nothing in any cleanup path touches it.

The second assertion is the one that earns its keep. Asserting only that the diff renders reports "no
diff" for a failure whose real shape is "the app is gone" — the app mounts one React root, and a module
that throws while a lazily-loaded pane is being brought in takes the root with it.
