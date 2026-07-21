# Soloist Comprehensive Codebase Audit Report

## 1. Executive Summary

This audit represents an all-round review of the "Soloist" codebase—a native Linux desktop app clone of Solo written in Rust, Tauri v2, and React/TypeScript. The architecture is engineered around Hexagonal (Ports & Adapters) principles with explicit bounded contexts, deterministic mock testing, robust actor-based supervision, and optimized rendering behaviors.

Our deep-dive audit was conducted with a focus on **Security, Performance (CPU & Memory), Reliability**, and **Code Maintainability**, specifically assessing WebKitGTK and Unix/Linux integration boundaries.

Overall, the codebase exhibits exceptionally high standards of structural discipline. However, several critical optimization areas, edge-case safety vulnerabilities, and minor security hardening opportunities were uncovered. This report details those findings alongside concrete, root-cause recommendations.

---

## 2. Security Audit & Hardening

### S-1: Tauri v2 Capabilities & Command ACL Validation
- **Location:** `crates/app/capabilities/` and `crates/app/src/lib.rs`
- **Assessment:** Tauri v2 introduces a robust fine-grained Access Control List (ACL) system. Currently, `capabilities/default.json` does not strictly enforce command-level permissions. While app-defined commands registered via `tauri::generate_handler!` are not gated by plugin ACLs by default, Tauri v2 *does* support restricting custom IPC commands using custom capability definitions to prevent compromised third-party webview code or XSS from calling sensitive local actions.
- **Impact:** Low. The front-end has no external web routing or dynamic script injection, which naturally mitigates XSS. However, for a process supervisor, enforcing a strict capability configuration is a valuable defense-in-depth practice.
- **Recommendation:** Implement explicit custom command permissions in `capabilities/default.json` restricting custom invoke handlers to absolute minimum privileges.

### S-2: Peer Credential Verification & Race Conditions (SO_PEERCRED)
- **Location:** `crates/app/src/peer_cred.rs`
- **Assessment:** `peer_pgid` retrieves credentials via `stream.peer_cred()`, unforgeably reported by the Linux kernel. It validates that the peer's UID matches Soloist's running UID.
- **Vulnerability:** Under high local process churn, a PID returned from `peer_cred()` could theoretically be recycled by the kernel before `getpgid()` is invoked, leading to a stale group association.
- **Mitigation:** The risk is already mitigated by the design of "fail-closed" process-group binding in the core: if a process tries to bind to a recycled PGID, it will simply fail to find a matching supervised process in the project, resulting in a refused bind rather than an unauthorized scope grant.
- **Harden Opportunity:** To ensure absolute safety on Unix sockets, you can use `SO_PEERGROUPS` or verify if the peer still resides in the exact expected path/namespace of the supervisor.

### S-3: Environment Variable Sanitization on Process Spawning (Implemented)
- **Location:** `crates/pty/src/lib.rs`
- **Assessment:** Spawning processes inherits the host shell's environment (`std::env::vars()`) and overlays `spec.env`.
- **Vulnerability:** Unsanitized environments can expose sensitive host keys (like `TAURI_PLATFORM`, `TAURI_ENV_DEBUG`, custom session tokens, or internal socket paths) to spawned processes or untrusted agent tools.
- **Fix Applied:** Implemented active environment sanitization in `PtyProcessSpawner` by iterating through `std::env::vars()` and explicitly invoking `builder.env_remove(key)` for any environment variables prefixed with `TAURI_`. This guarantees that internal Tauri application state never leaks into child process groups.

---

## 3. Performance, CPU & Memory Audit

### P-1: Connection Pooling in SQLite Durability
- **Location:** `crates/store/src/lib.rs` (SqliteStore connection lock)
- **Assessment:** `SqliteStore` operates via a single `Mutex<Connection>`.
- **Performance Trade-off:** While a single-writer lock is highly reliable and prevents SQLITE_BUSY errors in WAL mode, it forces all reads and writes to queue sequentially on `spawn_blocking` pools.
- **Optimization:** For read-heavy operations (e.g. project and process snapshot listings), a connection pool (using `r2d2` or `sqlx` read-pools) would allow parallel read executions while keeping a single dedicated writer connection, significantly reducing latency when concurrent agents query states.

### P-2: Large Scrollback Allocations in Hot-Path Terminal PTY Buffers
- **Location:** `crates/core/src/terminal/buffers.rs`
- **Assessment:** `ScrollbackBudget` enforces a global byte budget (default 16MB) by shrinking individual process buffers. However, the vector/string allocations within raw scrollback buffers can cause fragmentation or high heap allocation overhead under extreme terminal throughput.
- **Optimization:**
  - Utilize `bytes::Bytes` or raw pre-allocated chunk pools to store raw terminal streams, eliminating continuous `to_vec()` allocations in the hot PTY read loop.
  - Coalesce small PTY read chunks dynamically before committing them to the ring buffer.

### P-3: Idle CPU Sampler and Port Scanner Overhead
- **Location:** `crates/sys/src/metrics.rs` and `crates/sys/src/portscan.rs`
- **Assessment:** Background samplers run on a periodic tick (~1s) reading `/proc/<pid>/stat` and `/proc/net/tcp`.
- **Optimization:**
  - If a process group has been resting (Stopped / Crashed) for several intervals, dynamically suspend its CPU/memory sampler loop or down-sample the rate to 5s/10s, reducing context switches and battery drain.
  - Port scanning via `/proc/net/tcp` parsing can be optimized by using incremental file-watchers or only scanning when new sockets are bound (triggered by kernel/eBPF or netlink), though basic proc-reading is standard.

---

## 4. React Performance Audit & Refinements

### PR-1: Avoid Redundant Sidebar Grouping & Filtering (Implemented)
- **Location:** `crates/app/ui/src/components/sidebar/Sidebar.tsx`
- **Problem:** Every time the `Sidebar` component re-rendered (including on 1 Hz metrics-tick updates, or when any other slice of parent/context state changed), the `filterSidebar` and `groupByProject` computations were executed from scratch. On a system with multiple projects and dozens of processes, this could consume a noticeable amount of CPU and cause periodic layout lag in WebKitGTK.
- **Fix Applied:** Wrapped both `filterSidebar` and `groupByProject` calls in `useMemo` hooks. They now only execute when their exact dependencies (`processes`, `projects`, `sidebar.show_filter_input`, `filter`, `sidebar.hide_empty_sections`, or `lineage`) actually mutate. This reduces rendering overhead on metrics ticks to a lightweight O(1) virtual DOM comparison for unchanged rows, making the UI extremely snappy.

### PR-2: Memoized Global Context Providers
- **Location:** `crates/app/ui/src/store/SignalsProvider.tsx` and `SignalsContext`
- **Assessment:** The codebase includes a custom-engineered `signalStore.ts` utilizing `useSyncExternalStore` with custom slice-level selectors. This is a best-practice pattern that avoids the classic React "re-render storm" where every metric update triggers a re-render of the entire tree.
- **Optimization Benefit:** Cell-level subscribers are completely isolated, ensuring that only the specific terminal header or sidebar row receiving a new CPU/RSS metric is re-rendered.

### PR-3: Component Virtualization for Heavy Projects
- **Location:** `TodoBoard.tsx` and `ScratchpadPanel.tsx`
- **Assessment:** Initial loading of projects with dozens of scratchpads or hundreds of to-do items can occasionally stall the main thread due to large DOM footprints.
- **Recommendation:** Implement list virtualization (e.g., using `@tanstack/react-virtual` or standard CSS content-visibility properties) in these views to render only the elements currently on screen, lowering memory footprint and layout times.

---

## 5. xterm.js Terminal Integration Audit & Optimizations

### XT-1: Animation-Frame Byte Coalescing
- **Location:** `crates/app/ui/src/components/terminal/useTerminal.ts`
- **Assessment:** The terminal implementation includes a robust custom-engineered coalescing loop. It queues incoming PTY chunks and flushes them to `xterm.js` inside a `requestAnimationFrame` callback.
- **Benefit:** This prevents high-frequency stdout streams from thrashing the browser thread. Instead of triggering a paint cycle for every incoming TCP/PTY chunk, updates are coalesced and drawn at the system's screen refresh rate (60Hz+), dramatically reducing CPU usage under heavy logging.

### XT-2: Bounded Keep-Alive WebGL Contexts (LRU Pool)
- **Location:** `crates/app/ui/src/store/useTerminalPool.ts`
- **Assessment:** WebKitGTK enforces a hard limit of 16 active WebGL contexts across the webview. Soloist brilliantly manages this by implementing a bounded Least-Recently-Used (LRU) keep-alive pool capped at 6.
- **Benefit:** When switching to a hidden terminal, its tab is marked `display:none`. This tells `xterm.js` to automatically pause its WebGL renderer and event loop. When brought back to focus, `useTerminal` triggers `fit()` and `focus()` seamlessly, keeping background resource consumption near zero.

### XT-3: Memory Backlog Overflow & Coherent Re-Attaching
- **Location:** `crates/app/ui/src/components/terminal/useTerminal.ts`
- **Assessment:** Background terminal panes keep receiving output but cap their memory backlog at `PENDING_CAP_BYTES` (512 KiB) to avoid out-of-memory states. If an overflow is detected, the tab avoids slicing partial/gappy output by setting `desyncedRef.current = true`. On next focus, it automatically requests a clean, coherent raw-scrollback replay from the core.
- **Benefit:** Zero gap glitches, clean terminal outputs, and robust memory containment.

---

## 6. Project Architecture & Code Quality Audit

### AQ-1: Strict Hexagonal Separation (Verified)
- **Crates:** `core`, `store`, `pty`, `sys`, `ipc`, `mcp`, `httpapi`
- **Assessment:** The separation of boundaries is strictly verified. The `crates/core` crate is entirely free of any Tauri, HTTP (Axum), database (rusqlite), or UI frameworks. All interactions with local OS, SQLite, and the webview go through traits (Ports) defined in `soloist_core` and implemented in lightweight adapters.
- **Refactoring:** This architecture is exceptionally clean and maintainable. No leaks or cycle dependencies exist, as proven by the `check-core-deps.sh` and `check-core-cycles.sh` scripts.

### AQ-2: Single Trusted Source of Truth across Boundaries
- **Assessment:**
  - Shared domain enums (like `ProcessKind`, `ProcStatus`, `Readiness`) are defined strictly in `crates/core/src/process.rs` and mirrored once in `crates/app/ui/src/domain.ts`. This ensures no drift.
  - In SQLite migrations, older tables are generalized cleanly (such as prompt templates being migrated to the unified `templates` table in v14).

### AQ-3: Code Self-Documenting & Clean Structure (Early Returns)
- **Assessment:** The entire Rust core and adapters follow a clean layout with small files (typically under 300 lines) and explicit module boundaries. Conditional flows pervasively use early returns rather than deeply nested statements, maintaining high readability.

---

## 7. Reliability & UNIX Signal Robustness

### R-1: UNIX Signal Handling and Grandchild Orphaning
- **Location:** `crates/pty/src/lib.rs` and `crates/core/src/supervisor/actor.rs`
- **Assessment:** Soloist spawns each child into a fresh process group (`portable-pty` creates a session leader), and stop signals target the group using `killpg(pgid, signal)`. This is highly robust.
- **Safety Hardening:** To fully guard against rogue orphan grandchildren that escape the process group (e.g., by creating a new session or process group), consider using Linux **control groups (cgroups v2)** to sandbox each command's process tree natively, guaranteeing 100% containment.

### R-2: SQLite Migration Transaction Atomicity (Fixed)
- **Location:** `crates/store/src/migrate.rs` and `projects_rebuild.rs`
- **Assessment:** Previously, intermediate migration steps were not wrapped inside a unified transaction, leaving the DB vulnerable to partial migration corruption on power/disk failures.
- **Fix Applied:** Refactored `migrate` to safely wrap steps 1 to 16 inside an atomic `BEGIN TRANSACTION` / `COMMIT` block. Step 17 (which rebuilds the `projects` table) remains in its own transaction block to safely toggle `PRAGMA foreign_keys = false` without conflicting with an active outer transaction. This resolves both atomic migration safety and SQLite's architectural constraints.
