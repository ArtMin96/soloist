# e2e-01 — Screens, Flows & the First Journey

**Goal:** Turn the proven harness into a **reusable architecture**, and prove it carries real behavior by
landing the first catalog walk — **Dashboard core**. This is the phase that decides what every later spec
looks like, so its output is a pattern as much as a test. Read [`README.md`](README.md) §2–§3 (scope +
architecture) first; [e2e-00](e2e-00-harness-and-ci.md) must be green before starting.

**Delivers:** `src/screens/`, `src/flows/`, `src/harness/`, the domain `specs/` tree, and the Dashboard-core
journey. **No product code** (a missing accessible name is the one exception — see Task 2).

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

## Tasks

1. **`src/harness/`:**
   - `appDataDir.ts` — a fresh `SOLOIST_APP_DATA_DIR` per session; torn down after.
   - `fixtureProject.ts` — materialize `fixtures/projects/<name>` into a temp dir and hand back its path,
     so a spec names a fixture and never a path.
   - `waits.ts` — the shared event-based waits. **No `sleep`, ever.** Prefer waiting on the concrete
     rendered state the user would look at (a status glyph's accessible name), not a timer.

2. **`src/screens/`** — one per surface the first journey touches: `Sidebar`, `ProcessControls`,
   `TrustDialog`. Each exposes intent-shaped methods (`sidebar.selectProcess(name)`,
   `controls.start()`, `trust.approve()`) and **queries by accessible name** — `$('aria/Start')` — per
   charter §3.2.
   **The one product-code exception:** if a surface the journey needs has no stable accessible name, add
   one **to the component** via `/impeccable` (CLAUDE.md §5) — an `aria-label` that improves the real
   app's accessibility, or a `data-testid` only where no semantic handle fits. Never a test-only hack,
   never a brittle CSS selector as a workaround.

3. **`specs/` domain tree:** create the directories from charter §3 (`projects/`, `supervision/`,
   `terminal/`, `monitoring/`, `agents/`, `coordination/`, `orchestration/`, `shell/`, `cross-surface/`).
   Empty directories are not committed — they arrive as their first spec lands. **Name for what they
   are**: no parity letters, no phase numbers, in directories, filenames, or test titles (CLAUDE.md §8).

4. **The Dashboard-core journey (`specs/supervision/`):** the charter §4 row —
   - the tree groups by project and kind;
   - selecting a process shows it;
   - Start / Stop / Restart reach the **real core** and the status glyph updates;
   - the trust dialog gates an untrusted command until approved.

   Drive the real core against the hermetic fixture. Assert on `ProcStatus` imported from the UI's
   `domain.ts` (charter §3.1) — never a literal `"Running"`.

5. **Traceability:** flip the Dashboard-core row in charter §4 to covered, and update the corresponding
   `PROGRESS.md` line for the Phase-5 deferred walk. Traceability lives in those two places — **not** in a
   code comment (CLAUDE.md §8).

## Interfaces

```
e2e/
├── src/
│   ├── harness/
│   │   ├── appDataDir.ts     # fresh SOLOIST_APP_DATA_DIR per session
│   │   ├── fixtureProject.ts # fixture name → materialized temp path
│   │   └── waits.ts          # event-based waits; no sleeps
│   ├── screens/
│   │   ├── Sidebar.ts        # selectProcess, groups, row status
│   │   ├── ProcessControls.ts# start/stop/restart/resume
│   │   └── TrustDialog.ts    # approve/reject; is-blocking
│   └── flows/
│       ├── trustProject.ts   # open fixture → trust it
│       └── startProcess.ts   # select → start → await Running
└── specs/
    └── supervision/          # the Dashboard-core journey
```

Later phases add screens as their walk needs them (`TerminalPane`, `OrphanDialog`, `CommandPalette`,
`SettingsOverlay`, `ProjectSettingsPane`, `OrchestrationPane`) — each when its trigger fires, never
speculatively (CLAUDE.md §16, YAGNI).

## Acceptance criteria

- The Dashboard-core journey passes locally (`just e2e`) and headless under `xvfb-run`.
- **No selector appears in a spec** — grep the `specs/` tree for `aria/`, `$(`, `data-testid` and find
  nothing.
- **No `sleep`** anywhere in `e2e/`.
- No status/kind string literal in a spec — they come from `domain.ts`.
- The run is hermetic: it passes twice in a row, and passes with the developer's real Soloist state
  present and untouched.
- Charter §4 Dashboard-core row and the matching `PROGRESS.md` line both flipped to covered.
- `just lint` / `just test` unaffected.

## Test plan

The journey **is** the test. Its value is that it fails for a real reason: break `proc_start` in the core
and this spec must go red. Confirm that once, deliberately, before calling the phase done — a spec that
cannot fail is a pretend test (CLAUDE.md §15).

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
