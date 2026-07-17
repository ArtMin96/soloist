ui := "crates/app/ui"
app := "crates/app"

# List recipes.
default:
    @just --list

# Run the desktop app in dev mode (Vite + Tauri).
dev:
    cd {{app}} && cargo tauri dev

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
# than `just test` — it compiles and launches the app. Needs a display; on a headless box install
# xvfb and WebdriverIO uses it automatically. One-time setup: `pnpm -C e2e install`.
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
    pnpm -C e2e typecheck
    pnpm -C e2e test

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
    ./scripts/check-core-deps.sh
    ./scripts/check-core-cycles.sh
    ./scripts/check-file-size.sh
    cargo test -q -p soloist-core --features schema config::schema

# Audit the Rust dependency tree against RustSec advisories, the license allow-list, and
# source provenance (policy in deny.toml). Needs `cargo install --locked cargo-deny`.
audit:
    cargo deny check

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
