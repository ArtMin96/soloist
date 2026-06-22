ui := "crates/app/ui"
app := "crates/app"

# List recipes.
default:
    @just --list

# Run the desktop app in dev mode (Vite + Tauri).
dev:
    cd {{app}} && cargo tauri dev

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
    ./scripts/check-file-size.sh

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
