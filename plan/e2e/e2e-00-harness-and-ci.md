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
│   ├── configs/            # prepared solo.yml variants, written over an open project
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
- **Hermetic, per session:** each spec file gets its own app instance *and its own data dir* —
  `beforeSession` points `SOLOIST_APP_DATA_DIR` at `e2e/.tmp/app-data/<cid>` — so no session boots
  into a previous session's durable state (restored projects, orphan bookkeeping, trust). A
  persistence walk will share a dir deliberately; nothing inherits one by accident. The env is set
  on `process.env` rather than as a capability because the published Tauri capability type has no
  `env` field even though the launcher honours one. Verified: a run writes `soloist.db` there and
  leaves `~/.local/share/soloist/` untouched.
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
- ⬜ **Owed:** the CI job has not run yet — it lands with the first PR that touches `crates/app/**` or
  `e2e/**`. The headless `xvfb-run` path is therefore unproven; treat the first CI run as the
  acceptance check for it.

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
- **State bleed** → a wiped `SOLOIST_APP_DATA_DIR` per run; never the developer's projects.
- **Slow CI** → e2e is a separate, path/dispatch-gated job, not part of the per-push gate.

## A benign warning you will see

`WARN tauri-service:service: Failed to clear mock store: A sessionId is required` on teardown. It is
the service's own mock-store cleanup running after the session closes; we never register mocks
(§1.2), and the run exits 0. Not worth chasing.
