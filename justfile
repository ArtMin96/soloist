ui := "crates/app/ui"
app := "crates/app"

# List recipes.
default:
    @just --list

# Run the desktop app in dev mode (Vite + Tauri).
dev:
    cd {{app}} && cargo tauri dev

# Build the .deb and .AppImage bundles.
bundle:
    cd {{app}} && cargo tauri build

# Run Rust and UI tests.
test:
    cargo test --workspace
    pnpm -C {{ui}} test

# Run every lint, format, type, and architecture gate.
lint:
    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    pnpm -C {{ui}} typecheck
    pnpm -C {{ui}} lint
    pnpm -C {{ui}} run format:check
    ./scripts/check-core-deps.sh

# Auto-format Rust and UI sources.
fmt:
    cargo fmt
    pnpm -C {{ui}} format

# Install UI dependencies.
setup:
    pnpm -C {{ui}} install
