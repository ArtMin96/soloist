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
│   └── projects/basic/     # solo.yml + bin/echo-loop.sh + bin/crasher.sh (self-contained)
└── specs/
    └── smoke.spec.ts       # app launches + shell renders
```

Plus, outside `e2e/`: the `wdio` cargo feature and one gated plugin registration in `crates/app`,
`crates/app/tauri.e2e.conf.json`, the `just e2e` recipe, `.github/workflows/e2e.yml`, the
`CONTRIBUTING.md` section, and `/e2e/.tmp/` + `/e2e/logs/` in `.gitignore`. **The frontend is
untouched** (§1.1).

## How it works

- **Provider:** embedded, pinned explicitly in `wdio.conf.ts`. No `tauri-driver`, no
  `webkit2gtk-driver`, no `sudo`. `browserName: "tauri"` (not `"wry"` — that is the tauri-driver
  convention this track does not use).
- **Build:** `onPrepare` runs `cargo tauri build --debug --no-bundle --features wdio --config
  tauri.e2e.conf.json` from `crates/app`. `-c/--config` merges the overlay over `tauri.conf.json`;
  no other build path sets either flag, so no ordinary build can produce this binary by accident.
- **Hermetic:** `wdio.conf.ts` wipes `e2e/.tmp/app-data` and points `SOLOIST_APP_DATA_DIR` at it.
  The env is set on `process.env` rather than as a capability because the published Tauri capability
  type has no `env` field even though the launcher honours one — setting it directly avoids depending
  on an untyped field. Verified: a run writes `soloist.db` there and leaves
  `~/.local/share/soloist/` untouched.
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
3. **The npm `@wdio/tauri-plugin` is not needed and is not benign.** Installing it into the UI package
   pulled `esbuild` with an unapproved install script into the product's dependency tree and broke
   `pnpm build`. The smoke spec passes without it and without its Rust sibling (§1.1).

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
