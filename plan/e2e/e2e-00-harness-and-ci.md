# e2e-00 — Harness, Plugin Wiring, Fixture & CI

**Status: built and green (2026-07-15).** `just e2e` compiles the app and drives the real window;
the smoke spec passes on a live WebKitGTK 605.1.15 session. What follows is what exists, not a plan.

**Goal:** Stand up the one reusable real-window e2e harness — **WebdriverIO + `@wdio/tauri-service`**
(embedded provider) driving the built `soloist` app — plus the feature-gated server the embedded
provider needs, a hermetic fixture, a `just e2e` runner, and a CI job. Read [`README.md`](README.md)
first — especially §1.1 (gating), §1.2 (no mocking), §1.3 (environment constraints).

## What exists

```
e2e/
├── package.json            # WebdriverIO + tauri-service; engines: node >=20 <26
├── pnpm-workspace.yaml     # minimumReleaseAge, denied install scripts, the native-utils pin
├── .nvmrc                  # 24 — the Node pin (§1.3); CI reads this file
├── tsconfig.json           # types + the @domain path alias onto the UI's domain.ts
├── wdio.conf.ts            # the only file that knows the service exists; builds the app in onPrepare
├── fixtures/
│   ├── projects/basic/     # solo.yml + stub processes under bin/ (self-contained)
│   └── bin/                # stub agent CLIs + the stand-in $SHELL, first on PATH
└── specs/
    └── smoke.spec.ts       # app launches + shell renders
```

Plus, outside `e2e/`: the `wdio` cargo feature and its gated plugin registration in `crates/app`,
`crates/app/tauri.e2e.conf.json`, the `VITE_E2E`-gated plugin injection in the UI's `vite.config.ts`
(§1.1), the `just e2e` recipe, `.github/workflows/e2e.yml`, the `CONTRIBUTING.md` section, and
`/e2e/.tmp/` + `/e2e/logs/` in `.gitignore`.

## How it works

- **Provider:** embedded, pinned explicitly in `wdio.conf.ts`. No `tauri-driver`, no
  `webkit2gtk-driver`, no `sudo`. `browserName: "tauri"` (not `"wry"` — that is the tauri-driver
  convention this track does not use).
- **Build:** `onPrepare` runs `cargo tauri build --debug --no-bundle --features wdio --config
  tauri.e2e.conf.json` from `crates/app`, into its **own `target/e2e/`** (`CARGO_TARGET_DIR`) — the
  `wdio` feature links a WebDriver server into the binary, and it must never land where `just dev`
  puts the ordinary one (nor force a feature-flip rebuild between dev and e2e). `-c/--config` merges
  the overlay over `tauri.conf.json`; no other build path sets any of these, so no ordinary build
  can produce this binary by accident.
- **Hermetic, fresh per session:** each session's app gets its **own** directories under
  `e2e/.tmp/` — its Rust state (`SOLOIST_APP_DATA_DIR` → `app-data/<worker>/`) and its webview
  storage (`XDG_DATA_HOME` → `xdg-data/<worker>/`) — never the developer's real
  `~/.local/share/`. Both are assigned in **`onWorkerStart`**, the launcher-side hook that runs
  before the worker's app is spawned; a module-load or lifecycle hook is too late (the app is
  already up) and the app inherits the *launcher's* environment, not its worker's. They are also set
  once at `wdio.conf.ts` module load, to a dead `unassigned/` subdir, only so the variables are
  never unset — an unset override falls through to the real path. The single wipe is `onPrepare`'s,
  before any app exists. Two earlier accounts this supersedes — a whole-tree wipe at each worker's
  load, and a per-instance `XDG_DATA_HOME` the service was believed to set — were both wrong; see
  the two postmortems below.
- **Stub agents and a stub shell:** `fixtures/bin/` is prepended to `PATH`, so agent CLIs resolve to
  deterministic stand-ins; and `SHELL` points at a profile-free stand-in shell, because the app
  captures a launch environment from `$SHELL -ilc env` and that capture outranks the app's own env —
  a real login shell would put a real `claude` straight back ahead of the stubs (observed: the
  first harness revision really launched the developer's Claude, session and all).
- **Every spec file leaves nothing running.** An agent or command that outlives its app session is
  a leftover the *next* session's app rightly raises its orphan dialog over — modal, so it blocks
  every click-driven spec after it (observed). Spec files stop what they started in `after`
  (`sidebar.stopIfRunning`).
- **Failure evidence:** `afterTest` saves a screenshot + page source per failed test into
  `e2e/logs/`, which the CI job uploads as an artifact — a red run shows the actual window.
- **Wayland:** the config sets `GDK_BACKEND=x11`; the developer does not have to know.
- **Display:** a desktop session works as-is; CI wraps in `xvfb-run`.

## Acceptance criteria — all met

- ✅ `just e2e` builds and launches the app; the smoke spec passes on a real desktop (3 passing).
- ✅ **The `wdio` feature is absent from `default` and a release build links nothing** — verified by
  `cargo tree -p soloist-app -e normal` (no `wdio`), with the `--features wdio` tree as the control.
  The frontend carries no wdio reference because it was never modified.
- ✅ `just lint` exit 0; `just test` exit 0 (315 UI tests + the Rust suite); `cargo check -p
  soloist-app` (default features) builds.
- ✅ `CONTRIBUTING.md` documents the steps; the one-time cost is `pnpm -C e2e install` and a Node LTS.
- ✅ **The CI job runs headless under `xvfb-run`** — proven by PR #74, whose `e2e` job passed on HEAD
  `f149cc2` alongside `check`, `bundle`, and `smoke`.

## Findings worth keeping

Three things cost real time and are recorded so they cost nobody else any:

1. **Node 26 breaks WebdriverIO** (§1.3). The failure is an opaque `UND_ERR_INVALID_ARG` on
   `POST /session`, which looks like a config error and is not. `just e2e` now guards it.
2. **`@wdio/tauri-service@1.2.0` cannot initialise on a clean install** (§1.3) — upstream release
   drift, fixed by the forward pin.
3. **"It passes without the wdio plugins" is true and misleading.** They are documented as required
   for `execute`/mocking/log-capture — none of which this track uses — so dropping them looks like
   sound YAGNI, and the spec stays green. It also goes **434 ms → 45.7 s**, because the service's
   eval bridge polls for a global they install and times out five seconds on every command (§1.1).
   Correctness was verified and cost was not; both are needed before calling something unnecessary.

## Risks & mitigations

- **A WebDriver server leaking into a shipped build** → the `wdio` cargo feature, absent by default,
  plus an acceptance check that is run rather than assumed.
- **The embedded provider is young** (`tauri-plugin-wdio-webdriver` first published 2026-05-03) →
  proven working here before any journey depends on it. If it regresses, revisit charter §1 as a
  decision — do not quietly add a provider fallback.
- **Flaky app-init timing** → wait on a concrete rendered element, never a fixed delay;
  `maxInstances: 1` (the app is single-instance — a second launch forwards to the first).
- **State bleed** → a fresh `SOLOIST_APP_DATA_DIR` (and `XDG_DATA_HOME`) per session; never the developer's projects or UI state.
- **Slow CI** → e2e is a separate, path/dispatch-gated job, not part of the per-push gate.

## The wipe raced the app it isolated — found and fixed

**Found 2026-07-17 by the cross-surface walk, which it blocked; fixed the same session.** The
per-worker `app-data` wipe ran *after* the app it was meant to isolate had already booted. Measured
by instrumenting both wipes and polling the data dir once a second through a run:

```
10:19:24.848  MODULE WIPE worker=launcher            (the launcher's own config load)
10:19:24.962  ONPREPARE WIPE
10:19:54      app boots → bin/ http-api.json soloist-ipc.sock soloist.db soloist.db-{shm,wal}
10:19:57.061  MODULE WIPE worker=0-0                 ← ~3 s AFTER the app booted
10:19:57      data dir empty — the running app's files are gone
10:19:59      runtime-state.json  (the app rewrites only this, during the spec)
```

**Two corrections to the older account this replaces.** The app inherits the **launcher's**
environment — it is spawned before its worker ever loads the config, so this is not "an environment
the service captured before `WDIO_WORKER_ID` was set". And each worker's wipe deleted **its own
app's** files, not a previous spec's.

**Why every walk stayed green anyway.** An open SQLite handle keeps working on an unlinked inode, so
the app ran on a database that no longer existed on disk. Isolation was achieved by destroying each
app's durable state mid-run rather than by starting clean: durable state was silently non-durable,
and every on-disk artifact (`http-api.json`, the IPC socket, the exported companion binaries) was
invisible to anything that looked for one. Nothing asserted any of that until a walk needed a real
file on disk, which is why it survived three walks and a CI run.

**The fix:** a data dir per session, assigned in **`onWorkerStart`** — the launcher-side hook that
runs before the worker, and so before its app, is spawned. The only wipe left is `onPrepare`'s,
before any app exists, so nothing is ever deleted from under a running app. Sessions are independent
by construction rather than by a wipe that had to be timed correctly.

## The webview's storage escaped isolation too — found and fixed

**Found 2026-07-17 alongside the raced wipe; fixed the same session.** `SOLOIST_APP_DATA_DIR`
isolates the Rust side only. WebKitGTK keeps the webview's own state — including everything the UI
puts in `localStorage` — under the app identifier in `XDG_DATA_HOME`, which the harness did not set,
so it fell through to `~/.local/share/dev.soloist.app/localstorage/`: **the developer's real, shared
location**, which no wipe here touched.

This was not theoretical. A run left `soloist.sidebar.collapsed = {"project:1":true}` there
(`useCollapseState.ts` writes it), and every subsequent run booted with the project node collapsed,
so no process rows rendered and three spec files went red — with the harness and the product both
innocent. The `e2e/logs/` screenshot is what showed it; the page source alone did not. This also
disproves an earlier charter claim that the service set a per-instance `XDG_DATA_HOME`: the env the
service spawns the app with shows `XDG_DATA_DIRS` but **no `XDG_DATA_HOME`**.

**The fix:** `XDG_DATA_HOME` is now assigned per session in `onWorkerStart`, the same seam and for
the same reason as `SOLOIST_APP_DATA_DIR` — so the webview starts at its defaults each session and
the real location is never read or written. Verified by freezing the real dir's mtimes across a full
run while the per-session webview storage materialised under `e2e/.tmp/xdg-data/<worker>/`.

One habit the episode still earns, now that a red run can no longer be *this* particular leak: when
rows or panels are mysteriously missing, a screenshot from `e2e/logs/` distinguishes empty state
from a real regression faster than the page source does.

## A benign warning you will see

`WARN tauri-service:service: Failed to clear mock store: A sessionId is required` on teardown. It is
the service's own mock-store cleanup running after the session closes; we never register mocks
(§1.2), and the run exits 0. Not worth chasing.
