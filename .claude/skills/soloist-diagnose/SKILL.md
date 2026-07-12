---
name: soloist-diagnose
description: Detect, fix, and re-verify performance, quality, supply-chain, and runtime issues across the whole Soloist app — the Rust core + Tauri adapters and the React/TS frontend — using the project's own detection tools. Use when the user wants to optimize, speed up, audit, health-check, or "find and fix anything broken or slow" in Soloist. Runs the real gates (cargo-deny, clippy, the soak leak-gate, bundle/bloat measurement) and delegates React component issues to the react-doctor skill, then applies fixes and re-runs each gate to confirm green. It changes code, but stays inside the architecture and the locked non-changes — it measures before optimizing, never weakens a test to pass, accepts an unfixable advisory only with a written reason, and never touches PROGRESS.md unless asked.
version: 1.1.0
argument-hint: "[scope — 'deps', 'lints', 'perf', 'leaks', 'frontend', or blank for a full sweep]"
allowed-tools: Bash(just *) Bash(cargo *)
---

# Soloist diagnose

You are **finding and fixing** real issues in Soloist — a clean-room native-Linux rebuild of Solo
(`soloterm.com`). You run the project's detection tools, interpret their output honestly, fix what is
fixable, and **re-run each gate to prove the fix**. You report what you changed with evidence.

This skill **changes code** (unlike `soloist-review`, which is read-only). That makes the guardrails below
non-negotiable.

## Hard rules (read first)

- **Stay inside the architecture.** Fixes live where the code lives — pure logic in `crates/core`, OS/UI/
  transport in adapters, React renders projections. Never add a domain `if` to an adapter or `use tauri`
  in `core` to "fix" something. If a real fix would break the hexagonal layering or a bounded context,
  stop and surface it (CLAUDE.md §12) — do not force it.
- **Measure before optimizing (CLAUDE.md §6).** No speculative micro-optimization. Profile the proven hot
  spot, record the before number, change one thing, record the after number. A change with no before/after
  evidence is rejected — revert it.
- **Locked non-changes are off-limits (§6).** Do not touch `panic = "unwind"`, `freezePrototype = false`,
  the `Cargo.lock` brotli pins, the release `opt-level`, or `removeUnusedCommands`. If a fix seems to need
  one of these, stop and ask.
- **Never weaken a test to pass a gate.** Fix the cause, or report it red with the output. Never `#[ignore]`
  or delete a test to dodge a failure (§12, §15).
- **No fabrication.** Confirm any Tauri/MCP/library API against official docs or `context7` before applying
  a fix that uses it (§4). Invoke the matching `tauri-*` skill for Tauri surfaces.
- **Leave the ledger alone.** Do not edit `PROGRESS.md` (or any `plan/` doc) unless the user explicitly
  asks. Report your changes in chat instead.
- **Consult the references first for perf/rendering work.** `references/webkitgtk-perf.md` records the
  WebKitGTK gotchas this project already hit and the perf issues already fixed (theme lag, terminal
  switching, metrics re-render storm). Read it before diagnosing any slow / janky / laggy / rendering
  issue so you apply the known fix and never re-derive or re-fix — it points to the full report,
  `plan/performance-native-feel.md`.

## The layers and their gates

Scope: `$ARGUMENTS` (blank = full sweep). Run only the layers in scope. For each: **detect → fix → re-verify**.

### 1. Supply chain — `just audit` (cargo-deny)

Detect: `just audit` (advisories, licenses, sources; policy in `deny.toml`).
Fix:
- A **vulnerability with a fixed version** → bump the crate (a normal dependency change, not a gratuitous
  `cargo update`); re-run.
- A **new disallowed license** → review it; if genuinely permissive and acceptable, add its SPDX id to
  `[licenses] allow` in `deny.toml`; otherwise drop/replace the dependency.
- An **unfixable vulnerability** (no patched release) → add it to `[advisories] ignore` with a written
  `reason`. The reason is the audit trail; never silence a finding without one.
Re-verify: `just audit` is green.

### 2. Rust lints & perf — clippy

Detect: `cargo clippy --workspace --all-targets -- -D warnings` (includes the `clippy::perf` group).
Fix: address each warning at the cause — needless clones in hot paths (the PTY read loop, event fan-out),
inefficient patterns, dead code. Keep changes minimal and idiomatic to the surrounding code.
Re-verify: clippy exits 0; `cargo fmt --check` is clean; the relevant `cargo test` passes.
Note: UI/rendering perf (WebKitGTK jank, React re-renders, theme/terminal lag) is **not** clippy — see
`references/webkitgtk-perf.md` for the known traps and already-applied fixes, and delegate React
component specifics to §5.

### 3. Size — `just bloat` / `just bundle-size`

Detect: `just bloat` (biggest crates/functions in the release binary), `just bundle-size`, `just ui-analyze`
(frontend treemap). Record the numbers.
Fix only a **measured** regression: lazy-load a heavy dep, code-split, drop an unjustified dependency
(§6 size budget). Re-measure and record before/after. Do not chase micro-savings.

### 4. Runtime leaks — `just soak`

Detect: `just soak` (the leak gate; serialized FD/thread/task counts across start/stop loops). This is the
agent-readable proxy for what tokio-console shows a human.
Fix: a leak means a resource isn't reclaimed deterministically (a PTY/FD not closed in Drop/cancel, a task
not ended, a process group not reaped). Fix the reclamation; the invariant is "N start/stops end at the same
PID/FD/task count" (§8). Re-verify: `just soak` is green.

### 5. React frontend — delegate to `/doctor`

Do **not** hand-roll React analysis. Invoke the **react-doctor** skill for component-level lint,
accessibility, bundle, and architecture findings, and let its `--fix` loop apply fixes. If no react-doctor
skill is listed, it is not installed — tell the user and offer to run
`npx -y skills add millionco/react-doctor` first. Never substitute the built-in `/doctor` command, which
diagnoses the Claude Code installation, not React code. For deeper runtime render issues, point the user
to `react-scan` / the webview DevTools. Re-verify: react-doctor re-scans clean.

## Tools you do NOT drive (tell the user)

`CrabNebula DevTools` (`just devtools`) and `tokio-console` (`just tokio-console`) are **live visual** tools
for a human's eyes — you cannot read a GUI or TUI. If a finding needs them (a specific slow IPC command, a
runtime stall), say so, tell the user exactly what to run and what to look for, and act on what they report.
If the `tauri-bridge` MCP (the `@hypothesi/tauri-mcp-server` package) is connected and the app is running
with `just agent-bridge`, you *can* inspect IPC calls and drive the webview through its `tauri-bridge:*`
tools instead — prefer it when available.

## Report

End with a short, factual summary: which gates you ran, what each said before and after, what you changed
(by file), what you accepted-with-reason, and anything left red with its output. No green you did not see.
