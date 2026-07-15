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

There is **one path and no fallback**: embedded. We do not configure `driverProvider`, do not install
`tauri-driver`, and do not carry a provider knob. If the embedded provider ever fails us, that is a
decision to revisit here — not a branch to pre-build (CLAUDE.md §15, YAGNI).

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
| `wdio-webdriver:default` | capability permission | Its ACL entry, in the e2e config overlay only. |
| `withGlobalTauri: true` | config | Required by the plugin setup; e2e overlay only. |

**The frontend is untouched — zero changes.** The docs also list `tauri-plugin-wdio` + the npm
`@wdio/tauri-plugin`, described as required. They are required only for `browser.tauri.execute()`,
mocking, and log capture; the docs' own "what works without the plugin" list — element interactions,
navigation, basic WebDriver commands — *is* the whole vocabulary of a user journey. Verified on this
repo: the smoke spec passes with neither installed. They are deliberately absent, and adding the npm
half proved actively harmful (it pulled `esbuild` with an unapproved install script into the **product**
UI package, breaking `pnpm build`). If a future spec genuinely needs `execute` for setup, add them then
— not before (CLAUDE.md §15, YAGNI).

**Gating — the load-bearing rule.** None of this may reach a shipped build (CLAUDE.md §6 size budget,
and a WebDriver server is an open door). The official docs show `#[cfg(debug_assertions)]`, but that is
too coarse here: `just dev` is a debug build, and it would stand up a WebDriver server on every ordinary
dev session. This repo already has the right pattern for *exactly* this shape of thing — a dev-only
plugin that drives the webview — in the **`agent-bridge`** feature (`tauri-plugin-mcp-bridge`, whose own
comment reads "Grants the agent broad webview access — run only in a trusted session"). Same threat,
same gate:

- A cargo feature **`wdio`** in `crates/app`, absent from `default`, mirroring `agent-bridge` /
  `devtools` / `tokio-console`. Release builds never link the plugin.
- A **`tauri.e2e.conf.json`** overlay mirroring `tauri.dev.conf.json`, carrying `withGlobalTauri` and the
  wdio capability. It must be its own file, **not** the dev config: `tauri.dev.conf.json` declares the
  `mcp-bridge` capability, whose permissions only resolve when `agent-bridge` links that plugin.

This is not a divergence from the docs. The docs' requirement is "don't ship it";
`#[cfg(debug_assertions)]` is one example of meeting it, and a cargo feature meets it more precisely.
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

Soloist already has **974 Rust + 315 UI tests** covering logic. This track does **not** re-assert that
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
    ├── projects/<name>/solo.yml  # hermetic projects the specs control
    └── bin/                      # deterministic stub processes
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

1. **A fresh `SOLOIST_APP_DATA_DIR` per session** (the documented data-dir override). The service also
   sets a per-instance `XDG_DATA_HOME` on Linux, and Soloist's default data dir is XDG-based, so
   isolation is available from either side. Never the developer's real state.
2. **Hermetic fixture projects** — a `solo.yml` plus deterministic stub binaries the spec controls (a
   `crasher` that exits on cue is how the 10/60s restart cap is provable). Never the developer's live
   stack.
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
| Dashboard core | `supervision` | Phase 5 (C-UI) | Tree groups by project/kind; select a process; Start/Stop/Restart reach the core and the status glyph updates; trust dialog gates an untrusted command | `phase-05` | ⬜ not built |
| **Resume last session** | `supervision` | **B9** | A stopped resumable agent shows **Resume last session** beside Start (sidebar row + terminal header); click → it relaunches continuing the prior session; a non-resumable target (Amp, Generic, command, terminal) shows **only** Start; the resumed terminal fits the pane (no right/bottom gaps) | `plan/05 §12`, `KNOWN-DIVERGENCES.md` D-9, `PROGRESS.md` | ⬜ not built |
| Agent lineage tree | `orchestration` | orch-01 (O3/O4) | A bound lead's spawned worker nests under it; a manual launch is a root; a worker's glyph flips on an activity event; a closed lead re-roots its workers | `plan/orchestrator/orch-01` | ⬜ not built |
| Scratchpad & to-do panels | `coordination` | orch-02 (O5/O6/O12/O14) | Edit + save a scratchpad and force a revision conflict; a blocker chain refuses complete then allows it; a comment renders its author; "Copy link" puts the `solo://` URL on the clipboard | `plan/orchestrator/orch-02` | ⬜ not built |
| Timers & wake-cycle | `orchestration` | orch-03 (O7/O8) | A `timer_fire_when_idle` arm shows a countdown + waiting-on chips; driving workers idle removes the timer and delivers its body (with the wake-reason prefix) to the lead's terminal | `plan/orchestrator/orch-03` | ⬜ not built |
| Settings surfaces | `shell` | Phase 11a/11b | Each shown tab renders and auto-saves; a per-project override resolves through to behavior | `phase-11a`, `phase-11b` | ⬜ not built |
| Desktop notification | `monitoring` | Phase 6 (D8) | A crash raises a libnotify desktop toast; clicking it focuses the terminal (needs a real notification daemon) | `phase-06` | ⬜ not built |
| Cross-surface parity | `cross-surface` | H1–H4 | A CLI/MCP `restart` moves the window's status glyph — one core command, many frontends | `phase-10`, CLAUDE.md §8 | ⬜ not built |

`later` / out of scope for this track: load/soak testing (that is the Phase-13 nightly soak, a different
gate), backend command mocking (§1.2), and any cross-platform matrix (D2 — Linux x86_64 only).

## 5. Phase index

| Phase | Title | Delivers | Status |
|-------|-------|----------|--------|
| [e2e-00](e2e-00-harness-and-ci.md) | Harness, plugin wiring, fixture & CI | The `e2e/` workspace, the feature-gated in-app WebDriver server, `just e2e`, a CI job, and **one smoke spec** (app launches, window renders) | ✅ **Built & green** (2026-07-15); the CI job's headless run is owed |
| [e2e-01](e2e-01-screens-and-flows.md) | Screens, flows & the first journey | The `screens/` + `flows/` layer and the **Dashboard core** walk — the first real journey, proving the architecture carries behavior | ⬜ next |

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
