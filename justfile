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
