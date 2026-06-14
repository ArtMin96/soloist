# Contributing to Soloist

Soloist is a native-Linux (Ubuntu, x86_64) desktop app built with Tauri v2, a Rust
core, and a React/TypeScript UI.

## Prerequisites

### System libraries

Tauri v2 requires **WebKitGTK 4.1**, which ships on Ubuntu 22.04 and newer. Build on
22.04+; Ubuntu 20.04 is supported only as a *runtime* target via the AppImage, which
bundles its own WebKit.

**Ubuntu 22.04+ (build host):**

```bash
sudo apt update && sudo apt install -y \
  libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev libgtk-3-dev
```

Ubuntu 20.04 ships only `libwebkit2gtk-4.0-dev`, which Tauri v2 does not support; build
the AppImage on 22.04 and run it on 20.04+.

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
just bundle   # build the .deb (and .AppImage) bundles
```

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
