# Contributing to Soloist

Soloist is a native-Linux (Ubuntu, x86_64) desktop app built with Tauri v2, a Rust
core, and a React/TypeScript UI.

## Prerequisites

### System libraries

Tauri v2 requires **WebKitGTK 4.1**, which ships on Ubuntu 22.04 and newer. Build on
22.04+. The supported floor for the shipped artifacts is **Ubuntu 22.04+** — Ubuntu 20.04
is **not** supported (its glibc 2.31 is too old for a 22.04 build; see
[`KNOWN-DIVERGENCES.md`](KNOWN-DIVERGENCES.md) D-11).

**Ubuntu 22.04+ (build host):**

```bash
sudo apt update && sudo apt install -y \
  libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev libgtk-3-dev
```

Ubuntu 20.04 ships only `libwebkit2gtk-4.0-dev`, which Tauri v2 does not support; build on
22.04+ and ship to 22.04+ (the `.AppImage` bundles WebKit but its glibc floor is still 22.04).

> **Build distributable artifacts on Ubuntu 22.04 — not on a newer host.** A Rust binary
> links against its build host's glibc, so a `.deb` built on, say, 24.04 (glibc 2.39+)
> fails to start on 22.04 with `version 'GLIBC_2.xx' not found`. The CI `bundle` job builds
> on `ubuntu-22.04` for this reason, and the `smoke` job installs that artifact in a
> 22.04 environment to catch ABI drift. Building locally on a newer host is fine for
> running on that same host, but the resulting `.deb` is **not** 22.04-compatible.

### Toolchains

```bash
# Rust (stable)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Node 20+ and pnpm
#   (install Node via your version manager, then:)
npm install -g pnpm

# Tauri CLI and the task runner
cargo install tauri-cli --locked
cargo install just --locked
```

## Setup

```bash
just setup   # install UI dependencies (pnpm)
```

## Common tasks

```bash
just dev      # run the desktop app with hot reload
just test     # Rust workspace tests + UI unit tests (vitest)
just lint     # rustfmt, clippy, eslint, prettier, tsc, and the dependency-direction guard
just fmt      # auto-format Rust and UI sources
just deb      # build only the .deb bundle (fastest; mirrors the per-PR CI gate)
just bundle   # build the .deb + .AppImage bundles
just e2e      # real-window end-to-end tests (see below; slower — builds and launches the app)
```

## Running e2e tests

`just e2e` drives the **actual Soloist window** through WebdriverIO, using the Tauri service's
embedded WebDriver provider. There is deliberately **no `sudo` driver install**: the WebDriver server
is compiled into the app under the opt-in `wdio` cargo feature, so there is no `tauri-driver` and no
`webkit2gtk-driver` to set up. Release builds link neither plugin.

One-time setup:

```bash
pnpm -C e2e install
```

Two requirements the harness cannot paper over:

- **Node must be older than 26.** `e2e/.nvmrc` pins the LTS; run `fnm use` (or `nvm use`) inside
  `e2e/`. WebdriverIO 9.29.1 sets `Content-Length`/`Connection` headers that Node 26's undici
  rejects, so no WebDriver session can start ([webdriverio#15265] — fixed upstream, unreleased).
  `just e2e` checks this and tells you rather than failing obscurely.
- **A display.** A normal desktop session works as-is. On a headless box, install `xvfb` and run
  under `xvfb-run -a` (what CI does). Under Wayland the harness sets `GDK_BACKEND=x11` for you.

Each run is hermetic: it builds the app, points `SOLOIST_APP_DATA_DIR` at a scratch directory it
wipes first, and drives a fixture project — it never reads or writes your real Soloist state.

The track's charter and phase plan live in [`plan/e2e/`](plan/e2e/README.md).

[webdriverio#15265]: https://github.com/webdriverio/webdriverio/issues/15265

## Packaging & releases

Soloist ships a `.deb` (Ubuntu 22.04+) and a portable `.AppImage`, both x86_64.

- **[`docs/packaging.md`](docs/packaging.md)** — what the bundles contain: the desktop
  entry, icons, the `solo.yml` MIME association, the system tray, opt-in launch-on-login,
  and the opt-in in-app updater.
- **[`docs/releasing.md`](docs/releasing.md)** — the step-by-step runbook for cutting a
  versioned GitHub release (the tag-driven `release.yml` pipeline signs the artifacts,
  publishes a draft release with `SHA256SUMS`, and smoke-tests the AppImage on a clean
  Ubuntu 22.04 with no WebKit installed).

Per-PR CI (`.github/workflows/ci.yml`) builds **both** bundles on `ubuntu-22.04` and
installs the `.deb` in a clean 22.04 container to catch ABI drift; the distributable,
signed artifacts come from the release pipeline.

## Layout

```
crates/
  core/      pure domain core — no framework dependencies
  store/     SQLite storage adapter
  pty/       process + PTY spawner
  app/       Tauri desktop binary + the React/TS UI in app/ui/
  mcp/       stdio MCP server
  httpapi/   loopback HTTP API adapter
  cli/       command-line client
  ipc/       shared local transport + message types
```

## Architecture rule

The domain core (`crates/core`) is pure: it must not depend on `tauri`, `rmcp`, `axum`,
`rusqlite`, or `notify-rust`. Everything OS/UI/transport/storage is an adapter behind a
port. `scripts/check-core-deps.sh` (run by `just lint` and CI) enforces this.
