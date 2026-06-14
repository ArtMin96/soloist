# 03 — Tech Stack & Decisions

Confirmed decisions are marked ✅. Each major choice lists the options and the rationale. Behavior facts
referenced as "ref §N" point to [`05-solo-reference-and-sources.md`](05-solo-reference-and-sources.md).

## Top-level (confirmed)

| # | Decision | Choice | Rationale |
|---|----------|--------|-----------|
| D1 ✅ | Stack | Tauri v2 + Rust core + React/TS + xterm.js | Same as Solo → faithful; small native bundle; Rust ideal for process/PTY |
| D2 ✅ | Target | **Ubuntu 20.04+, x86_64 only** | Per your call; `.deb` (22.04) + `.AppImage` (20.04). No arm64. |
| D3 ✅ | Licensing | **Dropped entirely** | Personal/open clone; no tiers, server, or analytics |
| D4 ✅ | MCP | Separate `soloist-mcp` binary, **stdio** | Mirrors Solo's bundled `mcp` helper (ref §7); decouples from GUI |
| D5 ✅ | `solo.yml` | **Byte-compatible** with Solo's schema (ref §3) | The real schema is known; same file should work in both |
| D6 ✅ | Storage | **SQLite** durable; in-memory runtime | Coordination/trust/settings need durability (ref §7) |

## D1 — Application stack (options considered)

| Option | Pros | Cons | Verdict |
|--------|------|------|---------|
| **Tauri v2 + Rust + React/xterm** | identical to Solo; ~10–25 MB; native Linux bundles; Rust for PTY/processes | WebKitGTK quirks | **Chosen** |
| Electron + Node | mature `node-pty`; big ecosystem | ~150 MB Chromium — contradicts Solo's "less RAM than a Chrome tab"; not faithful | Rejected |
| Pure Rust TUI (ratatui) | tiny; SSH-able | can't match GUI (sidebar, themes, scratchpad editors) | Rejected |

## D2 — Platform & packaging
- **Ubuntu 20.04+, x86_64.** `.deb` targets 22.04 (`webkit2gtk-4.1`); `.AppImage` (self-contained
  webkit) covers 20.04 (`4.0`). No arm64, macOS, or Windows.

## D6 — Storage layer
- **SQLite** (via `rusqlite` or `sqlx`), WAL mode, versioned migrations. Holds: trust decisions,
  project registry, settings, agent-tool defs, scratchpads, todos (+comments/blockers/locks), leases,
  key-value. **Repository pattern** behind the `Store` port (ref `04` §7). Runtime process state
  (status/PIDs/metrics/PTY buffers) stays **in memory**; a small runtime-state file enables orphan
  adoption (ref §4).
- Alternative considered: flat JSON files — rejected (concurrency, transactional writes, queries,
  revision guards all want a DB).

## D7 — MCP implementation
- Use the official Rust MCP SDK (`rmcp`) for the `soloist-mcp` stdio server; hand-rolled JSON-RPC
  fallback if the SDK lags the spec. The MCP binary is a **thin adapter** to the core over the local
  IPC socket (ref `04` §1). Tool **names** mirror Solo (ref §7); **param JSON Schemas are clean-room**,
  documented per tool.

## D8 — Auto-summarization & the LLM dependency
- Solo summarizes agent output via headless models (ref §6). For us this is **optional and degradable**
  (ref `04` §8): if enabled, shell out to the user's **own** configured agent CLI in headless mode
  (e.g. `claude -p`), or disable. Idle detection's **heuristic** signal (visible-output / OSC-title)
  works without any LLM, so the core never hard-depends on a model. No keys, no cloud requirement.

## D9 — Env capture
- Match Solo (ref §5): resolve `$SHELL` → passwd → `/bin/sh`; run `-ilc env`; parse; cache ~10 min.
  Precedence (our decision): per-process `env` > captured shell env > app env. On capture failure,
  prepend `~/.local/bin`, `/usr/local/bin`, `/usr/bin`.

## D10 — Terminal stack
- Backend PTY: `portable-pty` (wezterm) + `nix` for process-group signals. Frontend: `xterm.js` +
  `@xterm/addon-fit` + `@xterm/addon-webgl` (canvas fallback). `TERM=xterm-256color`; no custom
  terminfo (ref §12).

## Library choices

**Rust core / adapters**

| Concern | Crate | Why |
|---------|-------|-----|
| Async runtime | `tokio` | actor tasks, timers, intervals |
| PTY | `portable-pty` | cross-platform PTY |
| Signals / groups | `nix` | signal the whole process group |
| Config | `serde` + `serde_yaml` + `schemars` | `solo.yml` parse + JSON Schema |
| File watching | `notify` + `globset` | debounced file-watch restarts |
| Metrics | `sysinfo` | CPU/RSS per process group |
| Ports | parse `/proc/net/tcp{,6}` + `/proc/<pid>/fd` | port discovery |
| Notifications | `notify-rust` | libnotify/D-Bus toasts |
| MCP | `rmcp` | stdio MCP server |
| HTTP API | `axum` (loopback) | `127.0.0.1:24678` |
| Storage | `rusqlite`/`sqlx` | SQLite repos + migrations |
| IPC | `interprocess` (UDS) | app ↔ mcp ↔ cli |
| Errors | `thiserror` + `anyhow` | typed boundaries |
| Tracing | `tracing` (+`-subscriber`) | structured logs/spans |

**Frontend**: React 19 + TS + Vite; **shadcn/ui (Radix primitives + Tailwind CSS) for components**;
TanStack Query; xterm.js; `markdown-it` + `mermaid` (lazy); `@tauri-apps/api` (wrapped in `api.ts`);
CSS-variable theming (shadcn design tokens, light/dark). Visual design is driven through `/impeccable`
(Phase 5); shadcn supplies the component primitives, not the visual identity.

## Resolved open questions (from v1 draft)
1. **D1/D2/D3** — confirmed by you.
2. **arm64?** — No (D2: x86_64 only).
3. **`solo.yml` compatibility?** — Yes, byte-compatible (D5); the real schema is documented (ref §3).
4. **CLI in v1?** — Yes (Phase 10, H4 is v1); it's a thin client of the loopback API, low cost.

## Confirmed scope decisions
- **Coordination layer = v1 must-have** (your call). Matrix rows **G1–G11 + E7** are v1; the full
  ~50-tool surface ships for parity. Phase 9 sequences durable store → leases/locks → timers/
  idle-watchers → scratchpads/todos → key-value so the highest-value coordination lands first.
- **Auto-summarization = off by default** (your call). Heuristic idle detection is always on;
  summarization is opt-in via your own headless agent CLI — no cloud/key requirement (D8).
- **Git: yes** (revised 2026-06-14) — under git with a private GitHub remote (`ArtMin96/soloist`);
  commit per phase. *(Earlier draft said "no git repo"; superseded.)*
