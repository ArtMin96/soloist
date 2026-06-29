# End-to-End Testing Track — Charter & Index

> A **standalone, cross-cutting track** for the real-window end-to-end (e2e) verification that several
> features defer as a **user-only walk**. It builds one reusable harness — **WebdriverIO + `tauri-driver`**
> driving the actual Soloist window — and then turns each owed manual walk into an automated `spec`.
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

## 1. Approach & the WebKitGTK finding (clean-room-irrelevant; tooling only)

**Decision: WebdriverIO + `tauri-driver` — not Playwright.** Tauri's Linux webview is **WebKitGTK**, which
exposes **no Chrome DevTools Protocol**, so Playwright/CDP-based drivers cannot attach. The supported path
is the **WebDriver** protocol via `tauri-driver` (a thin proxy over `WebKitWebDriver`), driven by
WebdriverIO. This was the Phase-5 finding and is now the canonical e2e approach for the project; any phase
file that still says "Playwright" defers to this track (reconcile the wording when that phase is next
touched). Source: the `tauri-testing` skill + [v2.tauri.app/develop/tests](https://v2.tauri.app/develop/tests/).

**Platform note (consistent with D2 — Linux x86_64 only):** WebDriver for Tauri is supported on **Linux**
(`WebKitWebDriver`) and Windows (Edge driver); **macOS has none** (WKWebView lacks WebDriver tooling). Our
single target is Linux, so this is a non-issue — but it is *why* there was never a macOS or CDP option.

**Cost the owner must accept once:** the harness needs **system packages installed via `sudo`** and a
display (real or virtual). It cannot be set up from the headless agent environment — hence this is owner-
driven setup, with the agent authoring the harness + specs.

## 2. The harness (what e2e-00 builds)

Grounded in the `tauri-testing` skill (no fabrication); confirm each command against it at build time.

- **System deps (Linux, one-time, `sudo`):** `sudo apt install webkit2gtk-driver xvfb` (plus the existing
  Tauri build deps from `CONTRIBUTING.md`). Verify with `which WebKitWebDriver`.
- **Driver:** `cargo install tauri-driver --locked`.
- **Layout:** a new top-level `e2e/` workspace (its own `package.json`, **not** mixed into the UI package)
  with `wdio.conf.ts` + `specs/`. Mirrors the skill's reference structure.
- **Target binary:** the built desktop app **`target/debug/soloist`** (`mainBinaryName: "soloist"`,
  `crates/app`); `wdio.conf` builds it in `onPrepare` (`cargo build -p soloist-app`) and points
  `tauri:options.application` at it (`browserName: "wry"`, port 4444).
- **Lifecycle:** `beforeSession` spawns `tauri-driver` (resolve once it logs `listening`); `afterSession`
  kills it. Each spec drives the real window and asserts on the DOM the components render.
- **Runner:** a `just e2e` recipe (`xvfb-run` on a headless box; a real display locally) and a CI job on
  `ubuntu-latest` (the skill's workflow: install the webkit/xvfb deps, `cargo install tauri-driver`,
  build UI + app, `xvfb-run npm test`). E2e is a **separate, slower gate** from `just lint` / `just test`
  (it builds and launches the app), run on PRs touching UI or on a label, not on every push.
- **Determinism:** drive a controlled fixture project (a `solo.yml` + stub agents/commands the spec
  controls) so a walk is repeatable and hermetic — never the developer's live stack.

A spec asserts on **stable selectors** (prefer `aria-label` / `role` / `data-testid` the components
already expose — e.g. the process-control buttons are labelled `Start` / `Resume last session` /
`Restart` / `Stop`, the terminal host is `data-testid="terminal-host"`), never on brittle CSS.

## 3. Owed real-window walks — the catalog (the living to-do for this track)

Each row is a deferred user-only walk recorded in `PROGRESS.md` / a phase file. Once e2e-00 lands, each
becomes one `e2e/specs/*.spec.ts`. Implement highest-value first; this table is the source of truth for
what's covered.

| Walk | Feature(s) | What the spec asserts | Recorded in | Status |
|------|-----------|-----------------------|-------------|--------|
| Dashboard core | Phase 5 (C-UI) | Tree groups by project/kind; select a process; Start/Stop/Restart reach the core and the status glyph updates; trust dialog gates an untrusted command | `phase-05` | ⬜ not built |
| **Resume last session** | **B9** | A stopped resumable agent shows **Resume last session** beside Start (sidebar row + terminal header); click → it relaunches continuing the prior session; a non-resumable target (Amp, Generic, command, terminal) shows **only** Start; the resumed terminal fits the pane (no right/bottom gaps) | `plan/05 §12`, `KNOWN-DIVERGENCES.md` D-9, `PROGRESS.md` | ⬜ not built |
| Agent lineage tree | orch-01 (O3/O4) | A bound lead's spawned worker nests under it; a manual launch is a root; a worker's glyph flips on an activity event; a closed lead re-roots its workers | `plan/orchestrator/orch-01` | ⬜ not built |
| Scratchpad & to-do panels | orch-02 (O5/O6/O12/O14) | Edit + save a scratchpad and force a revision conflict; a blocker chain refuses complete then allows it; a comment renders its author; "Copy link" puts the `solo://` URL on the clipboard | `plan/orchestrator/orch-02` | ⬜ not built |
| Timers & wake-cycle | orch-03 (O7/O8) | A `timer_fire_when_idle` arm shows a countdown + waiting-on chips; driving workers idle removes the timer and delivers its body (with the wake-reason prefix) to the lead's terminal | `plan/orchestrator/orch-03` | ⬜ not built |
| Settings surfaces | Phase 11a/11b | Each shown tab renders and auto-saves; a per-project override resolves through to behavior | `phase-11a`, `phase-11b` | ⬜ not built |
| Desktop notification | Phase 6 (D8) | A crash raises a libnotify desktop toast; clicking it focuses the terminal (needs a real notification daemon) | `phase-06` | ⬜ not built |

`later` / out of scope for this track: load/soak testing (that is the Phase-13 nightly soak, a different
gate), and any cross-platform matrix (D2 — Linux x86_64 only).

## 4. Phase index

| Phase | Title | Delivers | Touches |
|-------|-------|----------|---------|
| [e2e-00](e2e-00-harness-and-ci.md) | Harness, fixture project & CI | The reusable `e2e/` harness + `just e2e` + a CI job + **one smoke spec** (app launches, the window renders) | `e2e/`, `justfile`, CI; no product code |

Subsequent phases (`e2e-01`+) each implement one catalog row as a spec; they are independent once e2e-00
lands and can be done in any order, highest-value first. Add a phase file (or just a spec) per walk as you
go — the catalog in §3 is the backlog.

**Build order:** e2e-00 first (nothing else runs without the harness). After it, the catalog walks are
independent slices.

## 5. Per-phase definition of done (inherits CLAUDE.md §7)

1. **e2e-00:** the harness builds and launches `target/debug/soloist`, a smoke spec passes locally
   (`just e2e`) and in CI (`xvfb-run`), and the steps are documented in `CONTRIBUTING.md` (the sudo deps)
   so a fresh machine can run them.
2. **Each walk spec:** drives the real window against the controlled fixture, asserts the behavior its
   catalog row names, is independent/hermetic, and flips the corresponding feature's deferred walk to
   covered — update that feature's `PROGRESS.md` line and this README's §3 status.
3. CI gates still pass (`clippy -D warnings`, `rustfmt`, `tsc --noEmit`, ESLint, dep-direction); e2e is an
   additional gate, never a replacement for the headless suites.
4. No flakiness: explicit waits (`waitForExist`/`waitUntil`), generous timeouts for app init, stable
   selectors. A spec that needs a `sleep` to pass is wrong — fix the wait.
5. `../../PROGRESS.md` updated (CLAUDE.md §10/§11).
