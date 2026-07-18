# End-to-End Testing Track — Charter & Index

> A **standalone, cross-cutting track** for the real-window end-to-end (e2e) verification that several
> features defer as a **user-only walk**. It builds one reusable harness — **WebdriverIO + the official
> `@wdio/tauri-service`** driving the actual Soloist window — and then turns each owed manual walk into an
> automated `spec`.
>
> This track owns *how Soloist is verified in a real window*; it builds **no product features**. It is
> subordinate to the canonical contract: where it disagrees with
> `../04-engineering-architecture-and-patterns.md` / `../05-solo-reference-and-sources.md` /
> `../06-codebase-blueprint-and-cleanup.md`, the higher doc wins (CLAUDE.md §2).

---

## 0. Why this track exists

Soloist's logic is verified headlessly today — `cargo test --workspace`, Vitest reducers/components, the
real-PTY integration tests, and the `mockIPC` frontend layer all run with no display. But several UI
behaviors can only be proven by **driving the real Tauri window** (a click reaches the core; a status
flip re-renders the right glyph; the terminal fits the pane). Every one of those has been recorded as a
**"user-only real-window walk"** deferral rather than being faked headlessly:

- The headless frontend layer is **`mockIPC`** (it stubs the Rust side — it never exercises the real
  backend, the real window, or layout/measurement).
- jsdom has **no real layout**, so anything depending on measured size (terminal fit, scroll, focus
  rings, responsive behavior) cannot be asserted in Vitest.

This track removes the deferral: one harness, then one spec per owed walk, runnable locally and in CI.

## 1. Approach — WebdriverIO via the official service

**Decision: WebdriverIO — not Playwright.** Tauri's Linux webview is **WebKitGTK**, which exposes **no
Chrome DevTools Protocol**, so Playwright/CDP-based drivers cannot attach. The supported path is the
**WebDriver** protocol. This was the Phase-5 finding and remains the canonical e2e approach for the
project; any phase file that still says "Playwright" defers to this track (reconcile the wording when
that phase is next touched).

**Decision: `@wdio/tauri-service` with the embedded WebDriver provider.** This is the path the official
Tauri docs prescribe — [v2.tauri.app/develop/tests/webdriver](https://v2.tauri.app/develop/tests/webdriver/)
presents the service as the recommended setup and the manual `tauri-driver` route as the legacy
alternative ("most projects should use `@wdio/tauri-service` instead"). The service's own
[platform-support doc](https://github.com/webdriverio/desktop-mobile/blob/main/packages/tauri-service/docs/platform-support.md)
states that **`'embedded'` is the default on every platform when `driverProvider` is unset**.

There is **one path and no fallback**: embedded. We name `driverProvider: 'embedded'` explicitly, as
the official sample does — it is also the default when unset, so this states the choice rather than
leaving it to be inferred — and we do not install `tauri-driver` or carry a provider knob. If the
embedded provider ever fails us, that is a decision to revisit here — not a branch to pre-build
(CLAUDE.md §15, YAGNI).

**What this replaces.** An earlier revision of this charter specified a hand-rolled harness that spawned
`tauri-driver` from `beforeSession` and required `sudo apt install webkit2gtk-driver`. The service
supersedes it: it owns driver lifecycle, port allocation, Xvfb detection, and per-instance data-dir
isolation that the hand-rolled config would otherwise reinvent. The embedded server runs **inside the
app**, so **no external driver and no `sudo` system package is needed** — `webkit2gtk-driver` is
required *only* by the `'external'` provider we are not using.

**Platform note (consistent with D2 — Linux x86_64 only):** the service supports Linux, Windows, and
macOS. Our single target is Linux. `xvfb` remains the way to run headless in CI (WebdriverIO ≥9.19.1
auto-detects it); CI already installs it for the packaging smoke.

**Wayland note:** on a Wayland desktop the service's docs prescribe `GDK_BACKEND=x11`. This matches the
project's recorded Wayland focus gotcha — the runner sets it rather than leaving it to the developer.

### 1.1 What the app under test must carry

The embedded provider works by linking a WebDriver server into the app. The whole cost, as built and
verified:

| Piece | Kind | Why |
|-------|------|-----|
| `tauri-plugin-wdio-webdriver` | Cargo (optional) | The in-app WebDriver server the harness attaches to. |
| `tauri-plugin-wdio` | Cargo (optional) | Backs the service's window/eval bridge. |
| `@wdio/tauri-plugin` | npm (UI dev dep) | Its frontend half; installs the globals that bridge looks for. |
| `wdio:default`, `wdio-webdriver:default` | capability permissions | Their ACL entries, in the e2e config overlay only. |
| `withGlobalTauri: true` | config | Required by the plugin setup; e2e overlay only. |

**The docs call the last three "required", and they are — for a reason the docs do not give.** Their
stated purpose is `browser.tauri.execute()`, mocking, and log capture, none of which this track uses
(§1.2). A spec does pass without them. But the service's eval bridge polls for
`window.__wdio_original_core__` — a global `@wdio/tauri-plugin` installs — and **every driver command
then waits five seconds and gives up**: measured here, the same smoke spec runs in **434 ms with them
and 45.7 s without**. Correct either way; unusable one way. This is recorded because "it passes
without them" is true, tempting, and wrong.

The npm half is a **dev dependency of the UI package**, which is the one place Vite can resolve it
from, and it never reaches a bundle: `vite.config.ts` prepends the import only under `VITE_E2E`, so a
normal build never resolves the dependency at all. That gate is checked in both directions (§6) —
a production build must not contain it, and the e2e build must.

**Gating — the load-bearing rule.** None of this may reach a shipped build (CLAUDE.md §6 size budget,
and a WebDriver server is an open door). The plugins' own setup doc shows `#[cfg(debug_assertions)]`,
but that is too coarse here: `just dev` is a debug build, and it would stand up a WebDriver server on
every ordinary dev session. This repo already has the right pattern for *exactly* this shape of thing
— a dev-only plugin that drives the webview — in the **`agent-bridge`** feature
(`tauri-plugin-mcp-bridge`, whose own
comment reads "Grants the agent broad webview access — run only in a trusted session"). Same threat,
same gate:

- A cargo feature **`wdio`** in `crates/app`, absent from `default`, mirroring `agent-bridge` /
  `devtools` / `tokio-console`. Release builds never link the plugins.
- The frontend import gated behind `VITE_E2E` in `vite.config.ts`, mirroring the `ANALYZE` switch
  beside it, so a normal build is byte-identical and never resolves the dependency.
- A **`tauri.e2e.conf.json`** overlay mirroring `tauri.dev.conf.json`, carrying `withGlobalTauri` and the
  wdio capability. It must be its own file, **not** the dev config: `tauri.dev.conf.json` declares the
  `mcp-bridge` capability, whose permissions only resolve when `agent-bridge` links that plugin.

This is not a divergence from the docs — it is one of the two shapes they document, and it is worth
being precise about *which* docs, because Tauri's are not the ones that say it. Tauri's WebDriver page
delegates all setup detail to the service and is **silent on gating** (it even points `appBinaryPath` at
a release binary). The requirement lives in
[webdriver.io's Tauri plugin setup](https://webdriver.io/docs/desktop-testing/tauri/plugin-setup), which
answers "Should I Include the Plugin in Production?" with *"No, the plugin is test-only"* and then shows
**both** `#[cfg(debug_assertions)]` **and** a cargo feature (`wdio = ["dep:tauri-plugin-wdio"]`) as ways
to meet it. We take the feature — the more precise of the two here, for the `just dev` reason above.
"Release links nothing" is an acceptance check that is actually run, not an assumption.

### 1.2 Mocking is out of scope — deliberately

The service also offers `browser.tauri.mock()` (backend command mocking). **We do not use it**, and the
plugins that provide it are not installed (§1.1). Using it would reintroduce precisely what this track
exists to remove: assertions against a stubbed Rust side. Specs drive the **real core** against a
controlled fixture.

### 1.3 Two environment constraints the harness cannot paper over

- **Node must be older than 26.** WebdriverIO 9.29.1 sets `Content-Length`/`Connection` headers that
  Node 26's undici rejects, so no WebDriver session can start
  ([webdriverio#15265](https://github.com/webdriverio/webdriverio/issues/15265) — fixed upstream,
  unreleased as of 2026-07-15). `e2e/.nvmrc` pins the LTS and `just e2e` checks it explicitly rather
  than failing obscurely.
- **`@wdio/native-utils` is pinned forward to 2.5.0.** `@wdio/tauri-service@1.2.0` imports
  `installMockSyncOverride` from it but pins it to 2.4.0, which does not export it — the service cannot
  initialise on a clean install. This is upstream release drift the maintainer fixed for the sibling
  electron service ([desktop-mobile#506](https://github.com/webdriverio/desktop-mobile/issues/506));
  the pin in `e2e/pnpm-workspace.yaml` applies the same remedy and comes out when a tauri-service
  release corrects its own pin.

Both are recorded because they are the two things most likely to waste a future session's afternoon.

## 2. Scope — user journeys, not a second copy of the pyramid

Soloist already has **998 Rust + 315 UI tests** covering logic. This track does **not** re-assert that
logic through a window. It owns the **user journeys** per domain — what an end user actually does — and
the headless suites keep owning pure logic. e2e is an **additional gate, never a replacement**.

A behavior belongs in e2e when it needs the real window: a real click reaching the real core, a real
status flip re-rendering, real layout/measurement, real cross-surface propagation. A behavior belongs in
the headless suites when it is a reducer, a projection, a pure function, or an FSM transition.

## 3. Architecture — how the specs are structured

The harness mirrors the app's own hexagonal discipline: each layer knows only the one below it, and each
fact lives in exactly one place (CLAUDE.md §15/§16).

```
e2e/                              # top-level workspace, its own package.json
├── package.json
├── tsconfig.json                 # path alias → the UI's domain.ts
├── wdio.conf.ts                  # the single source; the only file that knows the service exists
│
├── specs/                        # WHAT the user does. No selectors, no waits, no paths.
│   ├── projects/                 #   open, trust gate, config sync, command CRUD
│   ├── supervision/              #   start/stop/restart, auto-start, resume, restart-exhausted
│   ├── terminal/                 #   echo, fit/resize, find bar, scrollback
│   ├── monitoring/               #   crash restart, file-watch restart, notifications, ports
│   ├── agents/                   #   detect, launch, idle-FSM badges
│   ├── coordination/             #   todos, scratchpad conflict, timers, locks
│   ├── orchestration/            #   lineage tree, wake-cycle
│   ├── shell/                    #   palettes, hotkeys, settings, theme, window controls
│   └── cross-surface/            #   CLI/MCP mutates → the window reflects it
│
├── src/
│   ├── screens/                  # the ONLY place selectors live. One per UI surface.
│   ├── flows/                    # reusable journeys spanning screens
│   └── harness/                  # app lifecycle, data dir, fixtures, waits
│
└── fixtures/
    ├── projects/<name>/          # hermetic projects the specs control (solo.yml + stub processes)
    └── bin/                      # stub agent CLIs, prepended to PATH to shadow real ones
```

**The layer rule:** `specs → flows → screens → harness`. A spec never contains a selector. A screen never
contains a fixture path. Only `wdio.conf.ts` knows which service or provider is in play. Swapping any of
that is a one-file change.

**Screens mirror `components/`.** One screen object per UI surface —`Sidebar`, `ProcessControls`,
`TerminalPane`, `TrustDialog`, `OrphanDialog`, `CommandPalette`, `SettingsOverlay`, `ProjectSettingsPane`,
`OrchestrationPane` — so a selector for a surface exists exactly once (§15 DRY, single source).

**Domain partition.** The `specs/` directories are the parity-matrix domains (projects `A`, supervision
`B`, terminal `C`, monitoring `D`, agents `E`, coordination `G`, shell `I`, orchestrator `O`) — but named
for **what they are**, never for their letter or phase. CLAUDE.md §8 bans plan tags in identifiers ("no
`phase5_test`), and that applies to spec files and test titles too. Traceability from a spec back to its
parity row lives in the §4 catalog below and in `PROGRESS.md` — not in code.

**Two domains are absent, one is added.** `F` (MCP) and `H` (HTTP/CLI) have no window to drive and are
already genuinely end-to-end headlessly. In their place, **`cross-surface/`** drives the CLI or MCP and
asserts the *window* updates — the one thing no current test proves, and a direct test of the "one
behavior, many frontends" invariant (CLAUDE.md §8).

### 3.1 Determinism — the three levers

1. **A fresh data dir per session, for both sides**, assigned in `onWorkerStart` — the app inherits
   the launcher's environment, so that is the only place that reaches it. `SOLOIST_APP_DATA_DIR`
   (the documented data-dir override) covers the **Rust** side: the database, the IPC socket, the
   HTTP runtime file. `XDG_DATA_HOME` covers the **webview** side: WebKitGTK keys the webview's
   `localStorage` and caches under the app identifier there, and the service does **not** set it on
   its own (an earlier revision claimed a per-instance `XDG_DATA_HOME` — dumping the app's env as the
   service spawns it shows `XDG_DATA_DIRS` and no `XDG_DATA_HOME`), so without this the webview fell
   through to the developer's real `~/.local/share/dev.soloist.app/` and bit once. Both are now set
   the same way; see e2e-00's two postmortems.
2. **Hermetic fixture projects and stub agents** — a `solo.yml` plus deterministic stub binaries the
   spec controls (a `crasher` that exits on cue is how the 10/60s restart cap is provable), and stub
   agent CLIs in `fixtures/bin/` that the runner prepends to `PATH`, shadowing any real ones — a
   launch never opens a real agent session, and detection behaves identically on a developer's box
   and in CI. Never the developer's live stack.
3. **No magic strings.** `e2e/tsconfig.json` aliases the UI's `domain.ts`, so a spec imports `ProcStatus`
   rather than typing `"Running"`. The Rust enum stays the single source across Rust → TS → e2e (§15).

### 3.2 Selectors — accessible names first

The app has exactly **one** `data-testid` (`terminal-host`) and queries overwhelmingly by role and label
(57 `getByRole`, 82 `getByText`, 26 `getByLabelText`). WebdriverIO natively supports the accessible-name
strategy — `$('aria/Start')` — and its docs rate accessibility-first selectors as best practice. The e2e
layer **inherits the app's existing convention** rather than forcing a `data-testid` rollout across ~80
components, and every selector doubles as an accessibility assertion.

Add a `data-testid` only where no stable accessible name exists (containers, the terminal host) — in the
component, via `/impeccable`, never as a test-only hack.

## 4. Owed real-window walks — the catalog (the living to-do for this track)

Each row is a deferred user-only walk recorded in `PROGRESS.md` / a phase file. Once e2e-00 lands, each
becomes specs under its domain directory. This table is the source of truth for what's covered and is
where spec → parity-row traceability lives.

| Walk | Domain | Feature(s) | What the spec asserts | Recorded in | Status |
|------|--------|-----------|-----------------------|-------------|--------|
| Launch an agent | `agents` | Phase 7 (E-UI) | The picker targets the open project and offers Claude with the command it spawns; launching renders it in the sidebar, labelled and selected under Agents; the app really starts it (the status settles `Running` against the stub agent); a terminal opens for it, mounted and measured non-zero | e2e-01 | ✅ **covered** |
| Dashboard core | `supervision` | Phase 5 (C-UI) | Tree groups by project/kind; select a process; Start/Stop/Restart reach the core and the status glyph updates; trust gates an untrusted command | `phase-05` | ✅ **covered** — `specs/supervision/` drives select → trust (the row control) → start → `Running`, stop → `Stopped`, a nonzero exit → `Crashed`, and restart proven by the reborn process's *changed* discovered port. Grouping is asserted incidentally (the Agents group on launch); a per-kind grouping pin is not owed |
| Trust review on `solo.yml` change | `projects` | A9 / CLAUDE.md §3 ("synced via hash-diff + debounce") | Editing the open project's `solo.yml` externally raises the review dialog; trusting from it is what lets the changed command start | this charter | ✅ **covered** — the blocking product gap (nothing watched `solo.yml`; `reload_project` had no caller for external edits) was closed by the `ConfigWatchReactor` (2026-07-16, `KNOWN-DIVERGENCES.md` D-2). `specs/projects/config-trust.spec.ts` writes the open project's `solo.yml` from outside the app, waits for the review dialog (the watcher → debounce → reload → `ConfigChanged{requires_trust}` chain, end to end), reads the row's **disabled** Start control while untrusted, trusts from the dialog, and proves the same command then starts to `Running` |
| **Resume last session** | `supervision` | **B9** | A stopped resumable agent shows **Resume last session** beside Start (sidebar row + terminal header); click → it relaunches continuing the prior session; a non-resumable target (Amp, Generic, command, terminal) shows **only** Start; the resumed terminal fits the pane (no right/bottom gaps) | `plan/05 §12`, `KNOWN-DIVERGENCES.md` D-9, `PROGRESS.md` | ⬜ not built |
| Agent lineage tree | `orchestration` | orch-01 (O3/O4) | A bound lead's spawned worker nests under it; a manual launch is a root; a worker's glyph flips on an activity event; a closed lead re-roots its workers | `plan/orchestrator/orch-01` | ✅ **covered** — `specs/orchestration/agent-lineage.spec.ts` launches a lead whose stub binds its own MCP session (over the real IPC socket, authenticated by its process group) and `spawn_agent`s a worker; the tree nests that worker under the lead (`aria-level` + the `role="group"` it sits in resolve `parent` to the lead's `data-process-id`), while a second manual agent renders as a root — the contrast that proves nesting reflects a real recorded lineage edge, not order. The worker's `data-activity` flips Working→Idle off the real idle sampler reading its PTY output. Closing the lead from outside the window (single-agent removal is reachable only via a bound MCP/IPC session — see below) re-roots the worker to level 1, the lead's node gone. Mutation-verified: disabling `lineage.record` reddens only the walk's nesting assertions, every other spec file holding |
| Scratchpad & to-do panels | `coordination` | orch-02 (O5/O6/O12/O14) | Edit + save a scratchpad and force a revision conflict; a blocker chain refuses complete then allows it; a comment renders its author; "Copy link" puts the `solo://` URL on the clipboard | `plan/orchestrator/orch-02` | ✅ **covered** (copy-link partial) — `specs/coordination/coordination-panels.spec.ts` drives the three window-dependent assertions against a bound lead writing the shared documents over the **real MCP/IPC wire** (the lead fixture's second arm, selected by a dropped plan file). A stale scratchpad save is refused by the **real** optimistic-concurrency guard: the conflict banner names the revision only the concurrent lead bumped, and reloading restores the lead's content with the window's rejected edit gone. A blocked todo's Complete is refused by the **real** gate (`TodoBlocked` surfaced) until its blocker completes, whose live `TodoChanged` clears the gate and lets it complete. A comment renders the author the core stamped from the lead's **bound session** ("Codex", never "unattributed"). **Copy-link is partial:** reading the system clipboard under WebKitGTK/WebDriver needs a test-only hack (no first-class WebdriverIO clipboard API; clipboard-read is denied under automation — webdriver.io docs verified 2026-07-18), so the `solo://` URL construction stays headless — the core `link` round-trip + scope-resolver tests (`crates/core/src/coordination/link_tests.rs`, `facade/link_tests.rs`) plus new Vitest for the UI `copyLink → writeText` wiring — leaving only the OS-clipboard hop honestly not e2e-proven. Mutation-verified: dropping the SQLite revision guard, the todo blocker gate, and the comment-author stamp each reddens exactly its one assertion, all six other spec files holding |
| Timers & wake-cycle | `orchestration` | orch-03 (O7/O8) | A `timer_fire_when_idle` arm shows a countdown + waiting-on chips; driving workers idle removes the timer and delivers its body (with the wake-reason prefix) to the lead's terminal | `plan/orchestrator/orch-03` | ✅ **covered** — `specs/orchestration/timers-wake-cycle.spec.ts` launches a bound lead that spawns a worker and arms a `fire_when_idle_all` timer over it across the **real MCP/IPC wire**; the Timers panel shows it waiting on that worker (the real core's `waiting_on`) with a live countdown. Deleting a hold file drives the worker idle: the **real idle sampler + scheduler** fire the timer, it leaves the panel, and its body — prefixed with the core's wake-reason header (`Soloist timer #… all … watched agents are idle`, so the woken agent knows peers-idle vs backstop) — arrives over a **real PTY** into the lead's terminal, read there through xterm's accessibility DOM. Reuses the `lead-agent` fixture (new timers arm) + a file-gated `opencode` worker; adds a `TimersPanel` screen + `TerminalPane` text read. **Product change (e2e-only):** the e2e build turns on xterm screen-reader mode so the WebGL-rendered terminal is DOM-readable (`appearance.ts`, gated on `VITE_E2E`; shipped default stays off, guarded by a Vitest). Mutation-verified: dropping the scheduler's PTY delivery write reddens **only** the delivery assertion (the timer still fires and leaves the panel); forcing the idle quorum unmet reddens **only** the panel-clear (under a bounded wait); every other spec file held |
| Settings surfaces | `shell` | Phase 11a/11b | Each shown tab renders and auto-saves; a per-project override resolves through to behavior | `phase-11a`, `phase-11b` | ⬜ not built |
| Desktop notification | `monitoring` | Phase 6 (D8) | A crash raises a libnotify desktop toast; clicking it focuses the terminal (needs a real notification daemon) | `phase-06` | ⬜ not built |
| Cross-surface parity | `cross-surface` | H1–H4 | A CLI/MCP `restart` moves the window's status glyph — one core command, many frontends | `phase-10`, CLAUDE.md §8 | ✅ **covered** (CLI) — `specs/cross-surface/cli-restart.spec.ts` starts a trusted command through the window, restarts it with the **real `soloist` binary** as a separate process, and proves the window shows the reborn process's *changed* ephemeral port. Covers the assembly no headless test reaches: `serve()`'s runtime-file write (its own tests drive `serve_on` on a hand-made listener), the CLI's discovery + token handshake (its tests are in-crate), and the composition root's HTTP wiring. Building it exposed a harness defect that had been deleting every app's on-disk state mid-run (e2e-00). The MCP half of the row is not owed by this spec |

`later` / out of scope for this track: load/soak testing (that is the Phase-13 nightly soak, a different
gate), backend command mocking (§1.2), and any cross-platform matrix (D2 — Linux x86_64 only).

## 5. Phase index

| Phase | Title | Delivers | Status |
|-------|-------|----------|--------|
| [e2e-00](e2e-00-harness-and-ci.md) | Harness, plugin wiring, fixture & CI | The `e2e/` workspace, the feature-gated in-app WebDriver server, `just e2e`, a CI job, and **one smoke spec** (app launches, window renders) | ✅ **Built & green** (2026-07-15); the CI job's headless `xvfb-run` run passed on PR #74 |
| [e2e-01](e2e-01-screens-and-flows.md) | Screens, flows & the first journey | The `screens/` + `flows/` layer and the **launch an agent** journey — the first real walk, proving the architecture carries behavior | ✅ **Built & green** (2026-07-16) |

Subsequent phases (`e2e-02`+) each implement one catalog row as specs under its domain directory; they
are independent once e2e-01 lands and can be done in any order, highest-value first. The catalog in §4 is
the backlog.

**Build order:** e2e-00 first (nothing runs without the harness), then e2e-01 (nothing is reusable without
the screens layer). After that, the catalog walks are independent slices.

## 6. Per-phase definition of done (inherits CLAUDE.md §7)

1. **e2e-00:** the harness builds and launches the app, a smoke spec passes locally (`just e2e`) and
   headless under `xvfb-run` (the CI path), and `CONTRIBUTING.md` documents the steps so a fresh machine
   can run them. The `wdio` feature is absent from `default` and **verified absent from a release build**.
2. **e2e-01:** the screens/flows layer exists, the Dashboard-core walk passes, and no selector appears in
   a spec.
3. **Each walk spec:** drives the real window against the controlled fixture, asserts the behavior its
   catalog row names, is independent/hermetic, and flips the corresponding feature's deferred walk to
   covered — update that feature's `PROGRESS.md` line and this README's §4 status.
4. CI gates still pass (`clippy -D warnings`, `rustfmt`, `tsc --noEmit`, ESLint, dep-direction); e2e is an
   additional gate, never a replacement for the headless suites.
5. No flakiness: explicit waits (`waitForExist`/`waitUntil`), generous timeouts for app init, stable
   selectors. A spec that needs a `sleep` to pass is wrong — fix the wait.
6. `../../PROGRESS.md` updated (CLAUDE.md §10/§11).
