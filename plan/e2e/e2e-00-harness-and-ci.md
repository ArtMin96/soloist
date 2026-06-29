# e2e-00 — Harness, Fixture Project & CI

**Goal:** Stand up the one reusable real-window e2e harness — **WebdriverIO + `tauri-driver`** driving the
built `soloist` desktop app — plus a hermetic fixture project, a `just e2e` runner, and a CI job. Land a
single **smoke spec** so the path is proven end to end before any feature walk is written. Read
[`README.md`](README.md) (the track charter) first.

**Delivers:** the harness every catalog walk (charter §3) builds on. **No product code.**

## Scope

**In:** a top-level `e2e/` workspace; `tauri-driver` + WebKitWebDriver setup (documented in
`CONTRIBUTING.md`); `wdio.conf.ts`; a controlled fixture (a `solo.yml` + stub scripts the specs drive,
not the developer's live stack); a `just e2e` recipe (`xvfb-run`-aware); a GitHub Actions job on
`ubuntu-latest`; one smoke spec.
**Out:** any feature walk spec (those are e2e-01+); changing product code; cross-platform CI (D2 — Linux
x86_64 only).

## Tasks

1. **System deps + driver (owner-run once; document, don't automate behind `sudo`):** `sudo apt install
   webkit2gtk-driver xvfb`; `cargo install tauri-driver --locked`; verify `which WebKitWebDriver`. Add
   these to `CONTRIBUTING.md` under a new "Running e2e tests" section.
2. **`e2e/` workspace:** its own `package.json` (`@wdio/cli`, `@wdio/local-runner`, `@wdio/mocha-framework`,
   `@wdio/spec-reporter`; TS via `tsx`/`ts-node` per the project's TS setup) — kept **separate** from the
   UI package so the heavy WebDriver deps never enter the app bundle's tree.
3. **`wdio.conf.ts`:** `browserName: "wry"`, `tauri:options.application = ../target/debug/soloist`, port
   4444, `maxInstances: 1`. `onPrepare` builds the UI + app (`pnpm -C crates/app/ui build` then
   `cargo build -p soloist-app`). `beforeSession` spawns `tauri-driver` and resolves once it logs
   `listening`; `afterSession` kills it. Generous `mochaOpts.timeout` for app init.
4. **Hermetic fixture:** an `e2e/fixtures/` project — a `solo.yml` with a trivial command and a **stub
   agent** script (a tiny shell script that prints a marker and stays alive, reused from the pattern in
   `crates/pty/tests/integration.rs`) — and a fresh `SOLOIST_APP_DATA_DIR` per run (the documented data-dir
   override, §3 invariants) so a spec never touches real state and always starts clean.
5. **`just e2e` recipe:** builds, then runs WebdriverIO; wraps in `xvfb-run` when no `$DISPLAY` is present
   so it works headless (CI) and on a real desktop alike.
6. **CI job (`.github/workflows/e2e.yml`):** `ubuntu-latest`; install the Tauri build deps +
   `webkit2gtk-driver xvfb`; `cargo install tauri-driver --locked`; build; `xvfb-run` the e2e run.
   Triggered on PRs labelled `e2e` or touching `crates/app/ui/**` / `crates/app/**` — **not** every push
   (it builds and launches the app, so it is the slow gate).
7. **Smoke spec (`e2e/specs/smoke.spec.ts`):** launch the app against the fixture and assert the window
   renders its shell (e.g. the sidebar `nav[aria-label="Projects"]` exists and the app chrome is present).
   No feature behavior — just proof the harness drives the real window.

## Interfaces

```
e2e/
├── package.json          # WebdriverIO deps (isolated from the UI package)
├── wdio.conf.ts          # wry capability → target/debug/soloist; tauri-driver lifecycle
├── fixtures/
│   ├── solo.yml          # a trivial command + a stub agent the specs control
│   └── stub-agent.sh     # prints a marker, stays alive (reused integration-test pattern)
└── specs/
    └── smoke.spec.ts     # app launches + shell renders
```

Specs target the stable handles components already expose: `aria-label` on the process-control buttons
(`Start` / `Resume last session` / `Restart` / `Stop`), `role="option"` rows, `data-testid="terminal-host"`,
`nav[aria-label="Projects"]`. Add a `data-testid` to a component only when no semantic handle fits — in the
component, via `/impeccable`, not as a test-only hack.

## Acceptance criteria

- `just e2e` builds and launches `target/debug/soloist` and the smoke spec passes on a real desktop.
- The same run passes headless under `xvfb-run` (the CI path).
- The CI job is green on a PR that touches the UI.
- `CONTRIBUTING.md` documents the one-time `sudo` deps + `cargo install tauri-driver` so a fresh machine
  can run e2e from the docs alone.
- No product code changed; `just lint` / `just test` unaffected.

## Test plan

The smoke spec **is** the test for this phase (it proves the harness). Keep it minimal and stable
(explicit `waitForExist`, no `sleep`). Feature behavior is verified by e2e-01+ specs, each per a charter
§3 walk.

## Risks & mitigations

- **WebKitGTK has no CDP** → use WebDriver via `tauri-driver` (the whole reason for this track); never
  reach for a CDP/Playwright driver.
- **Flaky app-init timing** → wait on a concrete rendered element, not a fixed delay; generous init
  timeout; `maxInstances: 1` (one window at a time).
- **State bleed between runs** → a fresh `SOLOIST_APP_DATA_DIR` + the hermetic fixture per run; never the
  developer's projects.
- **Slow CI** → e2e is a separate, label/path-gated job, not part of the per-push `lint`/`test` gate.
- **`sudo` deps can't be installed from the agent sandbox** → this phase is **owner-driven setup**; the
  agent authors `e2e/` + the spec, the owner runs the one-time install and the first `just e2e`.

## Effort

~1 day for the harness + smoke spec + CI (owner runs the one-time deps); each subsequent catalog walk is
~½–1 day of spec.
