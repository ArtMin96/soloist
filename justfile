ui := "crates/app/ui"
app := "crates/app"

# List recipes.
default:
    @just --list

# Run the desktop app in dev mode (Vite + Tauri).
dev:
    cd {{app}} && cargo tauri dev

# Run the production renderer in a browser against bounded development fixtures.
browser:
    pnpm -C {{ui}} dev:browser

# Run dev mode while an *installed* Soloist keeps running, untouched. Both overrides are
# load-bearing. The data dir, because the IPC server unlinks and rebinds the socket on start:
# sharing one dir silently steals the installed app's MCP clients. The identifier, because
# single-instance keys its DBus name on it: matching, the dev build hands its arguments to the
# installed app, focuses *that* window, and exits 0 — so no dev window ever opens.
[doc("Run dev mode alongside an installed Soloist, leaving it untouched.")]
dev-alongside:
    cd {{app}} && SOLOIST_APP_DATA_DIR="$HOME/.local/share/soloist-dev" cargo tauri dev --config '{"identifier":"dev.soloist.devmode"}'

# Run dev with CrabNebula DevTools — a viewer opens showing IPC command timings, events, and
# spans. Dev-only; the `devtools` feature is never in a release build.
devtools:
    cd {{app}} && cargo tauri dev --features devtools

# Run dev with tokio-console instrumentation, then attach the `tokio-console` CLI in another
# shell (install once: `cargo install --locked tokio-console`). Surfaces live task states,
# poll times, and lock contention. Dev-only; needs the tokio_unstable cfg, set here.
tokio-console:
    cd {{app}} && RUSTFLAGS="--cfg tokio_unstable" cargo tauri dev --features tokio-console

# Run dev with the MCP bridge so an AI agent (via @hypothesi/tauri-mcp-server, registered in the
# Claude Code MCP config) can inspect IPC calls and drive the webview on ws://localhost:9223.
# Dev-only: the feature plus the withGlobalTauri/capability override in tauri.dev.conf.json never
# enter a release build. Grants the agent broad webview access — run only in a trusted session.
agent-bridge:
    cd {{app}} && cargo tauri dev --features agent-bridge --config tauri.dev.conf.json

# Build only the .deb bundle (mirrors CI; faster than the full release set).
deb:
    cd {{app}} && cargo tauri build --bundles deb

# Build the full release set (.deb + .AppImage). AppImage is finalized in Phase 12.
bundle:
    cd {{app}} && cargo tauri build

# Run Rust and UI tests.
test:
    cargo test --workspace
    pnpm -C {{ui}} test

# Real-window end-to-end tests: builds the app with the `wdio` feature (an in-app WebDriver server,
# never in a release build) and drives the actual window through WebdriverIO. A separate, slower gate
# than `just test` — it compiles and launches the app. One-time setup: `pnpm -C e2e install`.
#
# On a display of its own, which is also how CI runs it. The window has to end up focused: the core
# routes an alert to the desktop rather than to an in-app toast for a user who is not looking, so on
# a shared desktop the notification walks assert against a window nobody is at. Measured on a
# GNOME/Wayland session, the app is refused focus outright — through Tauri's own `set_focus` as
# readily as through xdotool or wmctrl — because Mutter owns focus for XWayland clients. A display
# with nothing else on it always grants it.
e2e:
    #!/usr/bin/env bash
    set -euo pipefail
    # The supported Node range lives in e2e/package.json (`engines.node`); this only reads its
    # ceiling out, so tightening or lifting the range is a one-file change.
    ceiling=$(node -p 'require("./e2e/package.json").engines.node.match(/<\s*(\d+)/)[1]')
    major=$(node -p 'process.versions.node.split(".")[0]')
    if [ "$major" -ge "$ceiling" ]; then
      echo "error: e2e needs Node < ${ceiling} (found ${major})." >&2
      echo "WebdriverIO 9.29.1 sets Content-Length/Connection headers that Node ${ceiling}'s undici rejects," >&2
      echo "so no WebDriver session can start (webdriverio/webdriverio#15265 — fixed upstream, not" >&2
      echo "yet released). Switch to the pinned LTS, which e2e/.nvmrc records:  fnm use  (in e2e/)" >&2
      exit 1
    fi
    if ! command -v xvfb-run >/dev/null 2>&1; then
      echo "error: e2e runs on a display of its own (see the comment above this recipe)." >&2
      echo "Install it:  sudo apt install xvfb" >&2
      exit 1
    fi
    pnpm -C e2e typecheck
    xvfb-run -a pnpm -C e2e test

# Regenerate solo.schema.json (the editor JSON Schema for solo.yml) from the SoloYml model.
# Run after changing the config model; the drift guard in `just lint` fails if it is stale.
schema:
    cargo run -q -p soloist-core --features schema --example gen_solo_schema > solo.schema.json

# Run the longevity soak — the leak gate. These tests are #[ignore]d (the regular `test`
# recipe and per-change CI skip them) and run nightly in CI. Serialized because each test
# measures the whole process's file-descriptor, thread, and task counts.
soak:
    cargo test -p soloist-pty --test soak -- --ignored --nocapture --test-threads=1

# Run every lint, format, type, and architecture gate.
lint:
    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    pnpm -C {{ui}} typecheck
    pnpm -C {{ui}} lint
    pnpm -C {{ui}} run format:check
    node scripts/check-theme-colors.mjs
    ./scripts/check-core-deps.sh
    ./scripts/check-core-cycles.sh
    ./scripts/check-file-size.sh
    cargo test -q -p soloist-core --features schema config::schema

# Audit the Rust dependency tree against RustSec advisories, the license allow-list, and
# source provenance. The whole policy lives in deny.toml — including which target, features,
# and crates make up the audited graph — so this recipe and the CI action check the same tree
# and cannot report different results. Needs `cargo install --locked cargo-deny`.
#
# Split in two so an `[advisories] ignore` entry stays visible in the transcript: cargo-deny
# only prints why an advisory was ignored at its `info` log level, and raising that for the
# whole check buries the note under thousands of lines of `[bans] multiple-versions`
# duplicate-crate graphs. Scoping `info` to the advisories check alone keeps everything else at
# the default level and shows exactly what risk is being knowingly carried, and why.
audit:
    cargo deny check bans licenses sources
    cargo deny --log-level info check advisories

# Auto-format Rust and UI sources.
fmt:
    cargo fmt
    pnpm -C {{ui}} format

# Install UI dependencies.
setup:
    pnpm -C {{ui}} install

# Report what takes space in the release app binary — the biggest crates/functions first.
# Measure before optimizing size. Needs `cargo install cargo-bloat`. Pass extra flags, e.g.
# `just bloat --crates` or `just bloat -n 50`.
bloat *args:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! cargo bloat --version >/dev/null 2>&1; then
        echo "cargo-bloat not installed — run: cargo install cargo-bloat" >&2
        exit 1
    fi
    cargo bloat --release -p soloist-app {{args}}

# Report the shipped artifact and frontend bundle sizes — the real numbers to track and
# record. Reads whatever is already built; run `just bundle` (or `just deb`) and
# `pnpm -C {{ui}} build` first.
bundle-size:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "== Packaged artifacts =="
    artifacts=$(find target -path '*/release/bundle/*' \( -name '*.deb' -o -name '*.AppImage' \) 2>/dev/null || true)
    if [ -n "$artifacts" ]; then echo "$artifacts" | xargs du -h; else echo "  (none built — run 'just bundle')"; fi
    echo
    echo "== Frontend bundle ({{ui}}/dist) =="
    if [ -d {{ui}}/dist ]; then
        du -sh {{ui}}/dist
        du -h {{ui}}/dist/assets/* 2>/dev/null | sort -h || true
    else
        echo "  (not built — run 'pnpm -C {{ui}} build')"
    fi

# Build the frontend with a bundle treemap — writes dist/bundle-stats.html (open it to see
# what fills the bundle). A normal `just bundle` build is unaffected.
ui-analyze:
    ANALYZE=1 pnpm -C {{ui}} build

# Report where the codebase repeats itself: cloned files first (whole-file similarity, measured
# with the one noun that distinguishes a pair's file names masked, so a clone renamed as it was
# pasted still shows), then blocks repeated verbatim. Covers both halves of the tree — the Rust
# workspace and the TypeScript frontend and e2e sources.
#
# On demand only. It always exits 0, and it is deliberately absent from `just lint` and from every
# CI workflow: a duplication gate fires on code that is legitimately similar but must stay separate,
# and a gate that cries wolf gets switched off, which is worse than no signal at all. Every other
# CLAUDE.md §15 rule has a signal; DRY had none, which is how it decayed unnoticed. This is it.
#
# Some pairs it lists are considered separations that must never be merged — the header of
# scripts/report-duplication.mjs names them. Read that before "fixing" anything here.
[doc("Report cloned files and repeated blocks across Rust and TypeScript (never fails).")]
dupes:
    @node scripts/report-duplication.mjs
