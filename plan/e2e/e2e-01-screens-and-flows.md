# e2e-01 — Screens, Flows & the First Journey

**Status: built and green (2026-07-16).** The three-layer architecture exists and carries its first
journey: opening a project, launching Claude into it, and asserting the app really starts it and
renders it. 4 specs, ~6 s. What follows describes what exists; the remaining catalog walks (charter
§4) are e2e-02+.

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

- ✅ The journey passes locally (`just e2e`); the headless `xvfb-run` path is owed with e2e-00's CI job.
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

The supervision walk's product-mutation pass is **owed** — its landing session verified the specs can
fail for real reasons the harder way instead: they caught a real defect (`ProjectService::open`
duplicated command registrations on re-open; fixed with a unit test) and a real product gap (no
`solo.yml` watcher — charter §4). The restart spec is structurally mutation-resistant: it asserts the
reborn process's *changed* ephemeral port, which no repaint can fake.

The trust-review walk (`specs/projects/config-trust.ts`) earned its keep the same way: building it caught
a second real defect. Every e2e session shares one durable data dir (the embedded provider spawns the app
with an environment captured before the per-worker `WDIO_WORKER_ID` is known, so `SOLOIST_APP_DATA_DIR`
resolves to one path for the run), so `basic` was already registered — and its root already watched — when
`materializeProject` deleted and recreated the fixture directory for this spec, giving it a new inode. The
config watcher held its now-dead watch on the vanished inode and never saw the edit. The fix is a genuine
robustness improvement, not a test accommodation: **`ConfigWatchReactor` now re-establishes a project's
watch on every `ProjectOpened`**, since re-opening a path is exactly when its directory may have been
replaced (unit test `reopening_a_project_re_establishes_its_watch`). The spec proved it end to end only in
the real window — the headless suites never re-open a project whose directory was swapped underneath it.

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
