# PROGRESS.md — Soloist State Ledger

> **This file is the shared memory across sessions.** Git history complements it, but this ledger is
> where a session reads what's done and what's next. **Read it at the start of every session** (per
> `CLAUDE.md` §1) and **update it at the end of every session** (per `CLAUDE.md` §10–§11). Keep it
> factual and evidence-backed — never mark `Verified` what you didn't verify.

---

## Current state

- **Overall:** **Phase 5 (Dashboard UI) — interactive core slice — `Done — pending verify`.** The visible
  app: the Phase-1 debug harness is replaced by a real dashboard wired to the core. New **Tauri command
  surface** (`crates/app/src/{commands,demo,pty_bridge}.rs`): `proc_list`/`proc_start`/`proc_stop`/
  `proc_restart`, `stack_start`/`stack_stop`/`stack_restart_running`, `pty_write`/`pty_resize`, and
  `pty_attach`/`pty_detach` streaming raw PTY bytes over a `tauri::ipc::Channel<Vec<u8>>` (→ `Uint8Array`;
  the high-throughput IPC primitive per the `tauri-calling-frontend` skill) with the scrollback replay
  sent as the **first** channel message (preserves the core's atomic no-gap/no-dup attach). A single-slot
  `PtyBridge` aborts the prior forwarder on re-attach (no leaked streaming tasks). **Frontend**
  (`crates/app/ui/src/`): `domain.ts` re-synced to the core (ProcessView gains `project`/`exit_code`;
  DomainEvent gains the 5 missing variants); `api.ts` typed IPC + the PTY Channel; `store/` (projection
  reducer, `grouping`, `useProcesses` actions, persisted collapse); `lib/status.ts` (the single
  ProcStatus→glyph/color/label map); components — `Sidebar`/`ProcessGroup`/`ProcessRow` (I1 grouped tree,
  collapsible, keyboard-selectable), `StatusIndicator` (shape+color+label, color-blind-safe),
  `ProcessControls` (B2/B3 per-row, reused), `Toolbar` (B4 bulk), `TerminalPane`+`useTerminal` (xterm.js
  `@xterm/xterm` 6 + `@xterm/addon-fit`, scrollback replay + live, write/resize, per-animation-frame
  coalescing), `EmptyState`, `ErrorBanner`. **`DESIGN.md` seeded** via `/impeccable document` ("The
  Instrument Panel": cool-slate neutral + one azure accent; saturated color spent only on status) and
  user-approved; `index.css` implements its OKLCH tokens (azure accent replaces the shadcn neutral/purple
  primary — fixing the PRODUCT.md "no purple" tell; status palette; radius 10→6px; Geist Mono added).
  One core change: `DomainEvent::ProcessSpawned` gains `project` (single-source — the event must carry
  what `ProcessView` needs to group). **`just lint && just test` green: 107 tests** (Rust 97 / UI 10 —
  +1 from the R6 direct `store::migrate` forward-migration test). **Pending verify:** on-screen **rendering is now observed green
  (2026-06-19** via `just dev`, host `DISPLAY=:0`, screenshots — the grouped tree + statuses + empty state
  render; the `freezePrototype` blank-window bug is confirmed fixed). **Still not observed:** live terminal
  I/O (echo) + control activation — no synthetic XTEST click fired any control this session (likely an
  XWayland/WebKit quirk, unconfirmed; a **real human click** must verify start/echo **before R2**) — and the
  Playwright e2e. See the 2026-06-19 entry + open threads.
- **Active phase:** **Phase 5 (Dashboard UI)** — `Done — pending verify` (interactive core slice).
  **Deferred to a Phase-5 follow-up** (per the session-scope decision the user approved): the **trust
  dialog** (A6/A9 UI), the **orphan dialog** (B8 UI), and **project load/switch** + the
  `ConfigEngine → Supervisor::register → reconcile_orphans → start_all` wiring. A built-in **demo stack**
  (`crates/app/src/demo.rs`, app-level scaffolding, commands pre-trusted) stands in for real config
  loading so the tree is populated and interactive; it is replaced when project-load lands.

### Prior-phase carry-forward (still accurate)

- **Phase 4 (PTY & Terminal I/O, C3) — `Done — pending verify`.** Real pseudo-terminals
  replace Phase 3's null stdio: each process runs `$SHELL -lc <command>` on the **slave** side of a PTY
  (`portable-pty`), so children see a real terminal (`isatty`) and behave interactively (colours, cursor
  control, agent TUIs). New core context **C3** (`crates/core/terminal/`) maintains, from one read
  stream, a bounded **raw** byte scrollback (256 KB) **and** a bounded **rendered** line buffer
  (5,000-line `Ring<LogLine>`) via a `vte` parser — this **folds in Phase 3's deferred Task 4** (output
  capture), built once over the PTY instead of throwaway pipe capture. It surfaces OSC **title** +
  **bell** as `DomainEvent`s and streams live raw bytes over a per-process broadcast. The `Supervisor`
  gains `write_stdin` / `resize` / `attach_pty` (atomic scrollback replay + live) / `pty_scrollback` /
  `rendered`; the actor drains PTY output → buffers/events and routes input → PTY. The `pty` adapter was
  rewritten over `portable-pty` (`TokioProcessSpawner` → **`PtyProcessSpawner`**), keeping the Phase-3
  process-group reaping contract. **Phase 3's B8 (orphan adoption) also landed this session** (see below).
  **The Phase 4 PR was then reviewed and every finding fixed this session** (see "Phase 4 review fixes").
  `just lint && just test` green: **102 tests** (core 74 / pty 10 / store 12 / UI 6). All v1 rows **C1–C7,
  C9** verified headless on a real PTY (`test -t 1`, `read x`, `tput cols`, OSC title/bell, raw-vs-rendered,
  attach replay); **B8** verified via core reconcile/adopt tests + real-adapter tests.
- **Phase 4 follow-up (built this session):** the deferred piece was the **xterm.js terminal pane**
  (parity **C8** `later` + phase-04 Task 9), now built in Phase 5 via `/impeccable`. **Phase 3 is also
  `Done — pending verify`** — B8 (orphan adoption) landed earlier, so B1–B8 are complete.
- **Phase 3:** **`Done — pending verify`** — **B8 (orphan adoption) landed this session**: runtime-state
  file recording (record on Running / forget on reap) + `reconcile_orphans()` (pure adopt/surface/prune
  classification) + adoption via a *synthesized* `Spawned` over the existing pgid (liveness-poll exit +
  killpg control + closed PTY), so an adopted process runs through the **same** actor — all headless-tested
  on fakes + the mock clock. Real adapters: `FileRuntimeState` (store, atomic JSON file) + `PgidOrphanControl`
  (pty, killpg). B1–B8 + A2/A6 delivered + tested. **Pending verify:** the app's reconcile-on-launch *call*
  (wired in Phase 5 after config-registration, so matches are found) + the in-GUI bits (Phase 5 Playwright);
  B7's "clears crash tracking" half still waits on the Phase-6 restart policy.
- **Phase 2:** `Done — pending verify` — its runtime deferrals A2/A6 closed in Phase 3.
- **Phase 1:** still `Done — pending verify` — its one open step is the **manual in-GUI Start/Stop
  click** (`just dev`); the demo now runs an ungated terminal (`sleep 60`) on a **real PTY** through the
  supervisor, so the click-through path is unchanged and still valid to confirm.
- **Last session:** 2026-06-16 — built Phase 5 (Dashboard UI, interactive core slice) and fixed a
  `freezePrototype` blank-window bug (see Critical details + Decisions / changes).

---

## Critical details (carry forward — don't relearn these)

- **Build host:** Ubuntu **22.04+** only (Tauri v2 needs WebKitGTK **4.1**; 20.04 has only 4.0). Run the
  app from `crates/app` (`cargo tauri dev`) or via `just`. CI runs on `ubuntu-22.04`.
- **glibc pins the runtime floor — build distributables on 22.04, not newer.** A Rust/Tauri binary
  links its *build host's* glibc. A deb built on a newer host (this dev box is glibc **2.43**) requires
  `GLIBC_2.39+` and **won't start on 22.04** (glibc 2.35) — `version 'GLIBC_2.xx' not found`. CI builds
  on 22.04 and the new CI `smoke` job `ldd`-checks the artifact on 22.04. Local builds run only on the
  same host. Verified via a clean-container smoke 2026-06-14.
- **Toolchain:** Rust **1.96** (pinned in `rust-toolchain.toml`), pnpm **11.6**, **tauri-cli 2.11.2**,
  **just**. App crates: `tauri` 2.11.2 / `tauri-build` 2.6.2.
- **`Cargo.lock` is load-bearing — do NOT run a bare `cargo update`.** It pins `brotli-decompressor`
  **5.0.0** + `alloc-stdlib` **0.2.2** to dodge an `alloc-no-stdlib` 2↔3 conflict in the Tauri tree
  (upstream brotli 8.0.3 bug). CI uses `--locked`. Unpin only once brotli fixes it upstream.
- **PTY adapter = `portable-pty` 0.9 (blocking I/O → 2 OS threads per *running* process):** one blocking
  thread drains the master into a bounded channel (backpressure), one reaps the child + resolves the exit
  future; both are bounded by the actor's lifetime (the actor drops the output receiver on stop). Correct
  and leak-free, but a **footprint item to revisit in Phase 13** for "hundreds of processes" (could move
  reads to `tokio::AsyncFd` + `try_wait` polling to drop the threads). New deps this phase: `vte` 0.15
  (core, pure ANSI parser — dep-guard still green) + `portable-pty` 0.9 (pty adapter). `Cargo.lock` brotli
  pins unchanged.
- **Frontend gotchas:** Vite **8** (oxc bundler — use a boolean `minify`, not `"esbuild"`); React **19**;
  TS **6** (use `paths` with **no `baseUrl`**); Tailwind **v4** + shadcn (radix-nova, OKLCH tokens,
  `@/*` alias); ESLint **10** flat config (register `react-hooks`/`react-refresh` as plugin objects —
  their preset configs are still eslintrc-shaped and crash flat config).
- **`freezePrototype` MUST stay `false` — `true` breaks xterm.js (blank window).** `tauri.conf.json`
  `app.security.freezePrototype: true` (set speculatively in Phase 0) `Object.freeze`s `Object.prototype`,
  so xterm's module-init `o.toString = s` throws `Attempted to assign to readonly property` in strict mode
  → the import fails → React never mounts → blank window. Fixed to `false` (Tauri's default; the config is
  embedded via `generate_context!`, so a change needs a binary rebuild). **Do not re-enable it.**
- **Terminal/UI stack:** `@xterm/xterm` **6** + `@xterm/addon-fit` 0.11 + `@fontsource-variable/geist-mono`
  (FE deps; the legacy `xterm` package is deprecated). PTY bytes stream over a `tauri::ipc::Channel<Vec<u8>>`
  (→ `Uint8Array`), **not** events; the scrollback replay is the first channel message (atomic no-gap
  attach). `radix-ui` (unified package) supplies `Collapsible`/`Tooltip`; `lucide-react` icons; reuse the
  shadcn `Button`. The TS domain mirror is hand-kept in `crates/app/ui/src/domain.ts` (single source).
- **Tauri before-commands run from the frontend dir** (`crates/app/ui`): they are `pnpm dev` / `pnpm
  build` (NOT `pnpm -C ui …`); `frontendDist` is `ui/dist` relative to `tauri.conf.json`; dev port 1420.
- **Gates:** `just lint` (rustfmt, clippy `-D warnings`, tsc, ESLint, Prettier, dependency-direction
  guard) and `just test` (cargo + vitest). The guard is `scripts/check-core-deps.sh`.
- **Comment policy:** docblocks + important comments only — no phase numbers, `plan/§` citations, or
  changelog notes in source (CLAUDE.md §8). Use `REVIEW-PROMPT.md` to review a phase's changes.

---

## Phase status

Status vocabulary: `Not started` · `In progress` · `Done — pending verify` · `Verified`.

| Phase | Name | Status | Evidence / notes |
|------:|------|--------|------------------|
| — | Planning (foundation + 14 phase docs) | **Done** | 22 plan files in `plan/`; decisions D1–D6 locked; coordination=v1; summarization off; under git |
| 0 | Foundations (workspace, CI, `.deb` build) | **Verified** | 8-crate workspace builds; `just lint` + `just test` green (clippy -D warnings, rustfmt, ESLint, Prettier, tsc, vitest 2/2, Rust placeholder tests); dependency-direction guard passes (detection verified against `soloist-app`); `Soloist_0.1.0_amd64.deb` (2.3 MB) builds; app launches on a real desktop and renders `app_info` → "version 0.1.0" (user-confirmed). Clean-container dpkg-install smoke (Ubuntu 22.04) now run: install + `Soloist.desktop` + binary OK, and it surfaced that **host-built** debs need glibc 2.39+ (this host is 2.43) so they don't run on 22.04 — distributable debs are the CI (22.04) artifact. CI `bundle` builds the `.deb`; new CI `smoke` job installs + `ldd`-checks + Xvfb-launches it on 22.04. Container *GUI launch* on a 22.04-built artifact still to be confirmed (the host-built deb is glibc-incompatible with 22.04 by design). |
| 1 | Walking skeleton (ports/adapters + event bus) | **Done — pending verify** | Ports (`ProcessSpawner`/`Clock`/`Store`/`EventSink` + `FileWatcher`/`Notifier`/`Summarizer` stubs), `DomainEvent` broadcast bus, `Facade` (C8), supervised process actor (FSM-driven; clock-driven SIGTERM→grace→SIGKILL; panic-isolated→`Crashed`), real `TokioProcessSpawner` (fresh pgroup + `nix::killpg`) + SQLite `Store` (WAL + `user_version` migration + `meta`). Tauri command/event wiring + reusable debug panel. **Evidence:** 10 core + 2 store + 3 pty(integration) + 6 UI tests green; `just lint && just test` green; K7 guard green. **Pending:** in-GUI Start/Stop click (Playwright → Phase 5). |
| 2 | Config & projects (real `solo.yml`, trust, sync, detect) | **Done — pending verify** | Context C1 built headless on the skeleton. `crates/core/config/{model,load,diff,sync}` (serde `SoloYml`/`ProcessSpec`, `deny_unknown_fields`, `IndexMap` order, documented defaults; total `load`/`parse` w/ 1 MB cap + empty/comment-only = empty + typed `ConfigError`; `ConfigSync` add/update/remove/**rename** diff; `ConfigEngine` content-hash sync that flags `requires_trust` and emits `DomainEvent::ConfigChanged` — **owns no spawner, starts nothing**), `core/hash` (SHA-256 `Hash` + length-prefixed variant hash), `core/trust` (`TrustStore`/`Trust`), `core/projects` (`Projects`, canonical-root identity), `core/debounce` (Clock-driven). `crates/store` grown to the repository pattern (`meta`/`projects`/`trust` modules + migration **v2**: `projects`/`trust` tables, FK cascade) implementing `ProjectRepo`/`TrustRepo`. **v1 evidence:** A1/A3/A4 (`config/load` tests), A7 (`trust` + store `trust_persists_across_reopen`), A9 (`config/sync` write→mutate→`ConfigChanged{requires_trust}`, rename-preserves, no-op-on-touch), A11 (store `projects` + core `projects`). A2/A6 runtime verify → Phase 3. `later` A5/A8/A10/A12/A13 deferred. New core deps: `serde_norway` 0.9, `indexmap` 2, `sha2` 0.11 (dep-direction guard green). Divergences: `KNOWN-DIVERGENCES.md` D-1 (variant scope), D-2 (live watcher → Phase 6). |
| 3 | Process supervisor (3 subtypes, status FSM, orphans) | **Done — pending verify** | **B1–B8 + A2/A6 delivered + tested.** `Supervisor` (C2) on the Phase-1 actor: mailbox actor (`Stop`/`Restart`), status FSM, graceful SIGTERM→5s→SIGKILL on the **pgroup**, exit classification, panic isolation; **trust gate in core** (A6); login-shell `$SHELL -lc` (A2/B5); bulk ops (B4); stop→lock-release hook (B7). Task 4 (output/log ring) delivered in Phase 4. **B8 orphan adoption (this session):** runtime-state file recording + `reconcile_orphans()` (adopt/surface/prune) + adoption via a synthesized `Spawned` over the existing pgid (liveness poll + killpg), reusing the actor; real adapters `FileRuntimeState` (store) + `PgidOrphanControl` (pty). **Evidence:** core reconcile/adopt/surface/prune + record/forget tests; store `FileRuntimeState` round-trip; pty `is_alive` on a real group. **Pending verify:** the app reconcile-on-launch *call* (Phase 5, after config-registration) + in-GUI bits (Phase 5 Playwright); B7's "clears crash tracking" half (Phase-6). |
| 4 | PTY & terminal I/O (rendered+raw, input, resize, OSC) | **Done — pending verify** | **C1–C7, C9 v1 delivered (C3 context); PR reviewed + all findings fixed.** Real PTY per process via `portable-pty` (`$SHELL -lc` on the slave; child sees a tty); `pty` adapter rewritten (`PtyProcessSpawner`) keeping pgroup reaping. Core `terminal/` (`ring`/`buffers`/`parser`): bounded raw scrollback (256 KB per-process **+ a 16 MB global aggregate cap**, **C5**) + `vte`-driven rendered `Ring<LogLine>` (5,000 lines, **C4** + folded Task 4) with `\r` overwrite/tab stops; OSC **title**+**bell** → `DomainEvent`s (**C7**); live raw bytes via per-process broadcast. `Supervisor`: `write_stdin`/`resize` (**C3**/**C6**), `attach_pty` (atomic replay+live, **C9**), `pty_scrollback`/`rendered`. **Evidence:** **102 tests** (core 74 / pty 10 / store 12 / UI 6); real-OS pty suite green (`test -t 1`→tty **C1**, `read x`→input echo **C3**, `tput cols`→resize **C6**, group reap/no-survivors hardened against the async-grandchild-reap race). `just lint && just test` green. **Pending verify:** xterm.js terminal pane (**C8** `later` + phase-04 Task 9) → Phase 5 via `/impeccable`; "vim/htop visually render" is the Phase-5/manual check. |
| 5 | Dashboard UI (sidebar tree, status dots, terminal pane, trust dialog) | **Done — pending verify** | **Interactive core slice:** `DESIGN.md` seeded (`/impeccable`) + approved; full Tauri command/event/PTY-Channel adapter; TS domain mirror re-synced; sidebar tree (I1), color-blind-safe status (shape+color+label), per-row + bulk controls (B2/B3/B4), live status, xterm.js terminal pane (C1–C7 UI), empty/error states. **Demo stack** seeds the tree (real project-load deferred). `just lint && just test` green (**107** after the R6 cleanup track). **Deferred follow-up:** trust dialog (A6/A9), orphan dialog (B8 UI), project load/switch + reconcile wiring. **Pending verify:** on-screen **render now observed green (2026-06-19, screenshots)**; live terminal I/O + control activation (synthetic clicks don't fire controls — verify a real human click before R2) + Playwright e2e still pending (see 2026-06-19 entry) |
| 6 | Monitoring, restart (10/60s), file-watch, notifications | Not started | **Nightly soak test starts running from here** |
| 7 | Agents & idle detection (5-state FSM, optional summarization) | Not started | Summarization OFF by default |
| 8 | MCP server core (`soloist-mcp` stdio, scope+identity, tools) | Not started | High-risk |
| 9 | Coordination layer (scratchpads/todos/timers/leases/kv) | Not started | **v1 scope.** Sequence: durable store → leases/locks → timers/idle-watchers → scratchpads/todos → key-value. High-risk |
| 10 | HTTP API & CLI (`127.0.0.1:24678` + `soloist` CLI) | Not started | |
| 11 | UX polish & execution profiles (palettes, deep links, themes) | Not started | |
| 12 | Packaging (`.deb` + `.AppImage`, x86_64) | Not started | Add containerized 20.04 AppImage smoke (webkit 4.0 runtime) here |
| 13 | Parity QA + longevity gate | Not started | The v1 definition-of-done; runs the soak/leak gate and parity walk |

Estimated v1 critical path: **~14–18 focused weeks** (one experienced Rust+TS dev); Phases 3, 8, 9 carry
the most risk. See `plan/phases/phase-13-parity-qa-testing.md` appendix for the per-phase breakdown.

---

## Decisions / changes this session

### Cleanup R6 landed — converge docs & ledger; R-phase cleanup track COMPLETE (2026-06-19)
- **Baseline re-confirmed green first** (the start-and-end gate): `just lint && just test` → **106 tests**
  (Rust **96** / UI **10**); clippy `-D warnings`, rustfmt, tsc, ESLint, Prettier, dep-guard pass; the
  file-size guard reports **zero outliers** (`file-size OK: no source file exceeds 400 non-test lines` —
  R5 cleared the last one). R5 reviewed before proceeding (sound: commit `3f07350` is a pure structural
  move — `testing.rs` 547 → `testing/{mod,clock,spawner,lock_releaser,runtime_state,repos,fixtures}.rs`;
  `testing/mod.rs` re-exports the **same eight** public items so `crate::testing::*` /
  `soloist_core::testing::*` are byte-stable; `lib.rs` untouched).
- **R6 = docs/ledger convergence (plan/06 §7), no code-logic change.** Reconciled every plan-doc claim the
  R0–R5 refactors invalidated. Drift grepped across the whole `plan/` tree + root `*.md`, then fixed:
  - **`plan/03`** (the named drift): the Config row listed **`serde_yaml`** but we ship **`serde_norway`
    0.9** (verified in `crates/core/Cargo.toml`: `serde_norway = "0.9"`, `indexmap`, `sha2`; **no**
    `serde_yaml`). Updated the row to `serde + serde_norway + indexmap (+ schemars when A5 lands)` and added
    a note: `serde_yaml` is archived upstream → Phase 2 adopted the maintained drop-in fork; `schemars`
    (A5 JSON-Schema) and `globset` (Phase 6 glob watch) are **not yet shipped** — the rows record them as
    the intended crates for that later work.
  - **`plan/04` §15:** the file-size guard footnote said "optional, not yet built" → now wired in `just
    lint`/CI as a **warn-only** signal (R0); footnote corrected, pointing tightening-into-a-hard-gate at
    `plan/06` §9.
  - **`plan/06`:** §3.2 "`supervisor.rs` (491 code lines) is the current outlier" → split in R2 (+ testing
    in R5), **guard now zero outliers**; §4 `ports.rs` → **`ports/`** and Noop defaults in **`ports/mod.rs`**
    (R3 split `ports.rs` → `ports/{mod,bundle}.rs`); §6 "the one real DRY gap today" rewritten as
    **resolved (R1/R5)** — `core::testing` is `pub` behind the `testing` feature, reused by `store`/`pty`,
    fakes in per-concern submodules; §9 enforcement row `scripts/check-file-size.sh` `to add` → **live
    (warn-only)**; §7 gained a **completion banner** (R0–R6 done, with commit refs) and the R6 description
    was corrected (the serde swap is a stale-doc fix, **not** a Solo-behavior divergence).
  - **`ARCHITECTURE.md`:** §3 `ports.rs` → `ports/`; §5 test-fakes "the cleanup fixes the current gap" →
    **R1 closed it; R5 split the module**; §6 roadmap gained the R0–R6 completion note.
- **`KNOWN-DIVERGENCES.md` reviewed — current, no new entry.** D-1/D-2/D-3 still hold; the
  `serde_yaml → serde_norway` swap is an internal dependency choice (not observable Solo behavior), so per
  the file's own scope it does **not** warrant a divergence entry (confirmed with the user via the decision
  point).
- **Honest coverage note from R5 — folded in (user-approved "add it now").** `crates/store/src/migrate.rs`
  previously tested only the downgrade-refusal branch directly; the forward-migration steps (create
  `meta`/`projects`/`trust`, bump `user_version`) were covered only transitively via
  `store/lib.rs::open_enables_wal_and_migrates_to_the_current_version`. Added a **direct** test
  `migrates_a_fresh_database_to_the_current_schema` (commit `2dce185`, a **separate** `test(store)` commit —
  one concern per commit): opens a fresh in-memory DB, runs `migrate()`, asserts `user_version ==
  SCHEMA_VERSION`, that each of `meta`/`projects`/`trust` is created, and that a second `migrate()` is a
  no-op (idempotent). Genuinely honest (fails if any forward branch breaks), per §15. **Store suite 12 →
  13; Rust 96 → 97; total 106 → 107.**
- **Verification (honest).** `just lint && just test` green before and after; the test commit moved the
  count **106 → 107** (Rust **97** / UI **10**); file-size guard still **zero outliers**; `Cargo.lock`
  untouched. Two commits: `2dce185` `test(store): cover the forward-migration path directly` + the docs
  commit carrying this entry. The stray root `package-lock.json` was **not staged** (user decision: leave
  it). **R6 is the LAST R-phase — the R0–R6 cleanup track is now COMPLETE.** Next is real feature work (the
  deferred Phase-5 follow-up), to begin only after the user signs off the cleanup.

### Cleanup R5 landed — split `core::testing` + honest-test audit (zero deletions) (2026-06-19)
- **Baseline re-confirmed green first** (the start-and-end gate): `just lint && just test` → **106 tests**
  (Rust **96** / UI **10**); clippy `-D warnings`, rustfmt, tsc, ESLint, Prettier, dep-guard pass; the
  file-size guard warned (non-gating) on the **one** outlier `core/testing.rs` (547 — R5's split target).
  R4 reviewed before proceeding (sound: demo seam purged from the pure core; `core::testing::terminal_registration`
  single-sources the launched-terminal fixture; public surface byte-stable).
- **R5 executed (commit `3f07350`, one reviewable commit per the per-R-phase rule). PART 1 — split the lone
  outlier `core/testing.rs` (547).** The shared test-fakes module was one flat file consumed cross-crate via
  the `testing` feature (`#[cfg(any(test, feature = "testing"))] pub mod testing;`), so the guard counted all
  547 lines as code. `git mv testing.rs → testing/mod.rs` anchored the rename; carved cohesive concerns into
  `crates/core/src/testing/` submodules (same approach as R2's `supervisor.rs` split):
  - **`clock.rs`** — `MockClock` (+ `Sleeper`/`MockState`).
  - **`spawner.rs`** — `FakeSpawner` + `Behavior`/`DiesOn` + the in-memory `OneshotControl`/`NoopControl`/
    `NoopPtyIo` + the `SIGKILL`/`SIGTERM`/`killed_by` helpers (private to the spawner).
  - **`lock_releaser.rs`** — `RecordingLockReleaser`.
  - **`runtime_state.rs`** — `FakeRuntimeState` + `FakeOrphanControl` (orphan-reconcile fakes).
  - **`repos.rs`** — `FakeTrustRepo` + `FakeProjectRepo` (+ private `FakeProjects`).
  - **`fixtures.rs`** — `terminal_registration` (the R4 cross-crate fixture).
  - **`mod.rs`** — thin root: private `mod` declarations + `pub use` re-exporting the **same eight** public
    items, so **every consumer path is byte-identical** — `crate::testing::*` (10 in-core consumers) and the
    cross-crate `soloist_core::testing::terminal_registration` (pty integration test + the `store`/`pty`
    dev-dep feature). `lib.rs` is **untouched** (`pub mod testing;` unchanged). Names are permanent/descriptive
    — no R-phase/phase number or plan citation in any file/type (§8). Largest new file `spawner.rs` = **232**
    lines; **file-size guard now reports ZERO outliers**.
- **PART 2 — honest-test audit across the whole suite (26 Rust test-bearing files + 3 vitest), zero
  deletions.** Walked every `#[test]`/`#[tokio::test]` and every vitest `it(...)`; delegated the first-pass
  triage to a read-only pass, then **personally verified** the called-out small/likely-vanity ones
  (`ui/src/lib/utils.test.ts`, `core/ids.rs`, `core/events.rs`). **Result: every test exercises real behaviour
  and can fail for a real reason — no tautological/pretend/empty test found, nothing deleted** (so the count
  holds at 106). Spot-check evidence: `utils.test.ts` `cn("p-2","p-4") → "p-4"` proves `twMerge` actually runs
  (a plain join would give `"p-2 p-4"`); `ids.rs` exercises the atomic counter, the hand-written `Display`
  path, and the `from_raw` wire round-trip; `events.rs` round-trips a `DomainEvent` through the real broadcast
  bus.
- **Two defensible SUSPECT items (kept, not deleted):** (1) `ids.rs::display_matches_the_raw_value` reads
  circular but `Display` is a separate code path from `get()` (a format/prefix change breaks it) — real; (2)
  `store/migrate.rs::refuses_a_schema_newer_than_this_build` is the module's **only** direct test.
- **One honest coverage note (NOT filled with a vanity test, per §15):** `store/migrate.rs`'s forward-migration
  branches (`< 1` → meta table, `< 2` → projects/trust tables, `user_version` bump) are covered only
  **transitively** via `store/lib.rs::open_enables_wal_and_migrates_to_the_current_version` (asserts
  `user_version == SCHEMA_VERSION` after a real open); only the downgrade-refusal branch is tested directly. A
  direct forward-migration test is the one worthwhile *addition* (not a deletion) — recorded here honestly,
  **not** papered over with a pretend test.
- **Verification (honest).** `just lint && just test` green before and after: **106** (Rust **96** / UI **10**),
  unchanged. clippy `-D warnings` clean — the scoped `#[allow(clippy::panic)]` on the `FakeSpawner` panic arm
  and the `impl Default`/`new()` patterns (active because the `testing` feature compiles the fakes into core's
  `not(test)` lib target) were **preserved across the move**. File-size guard: **zero outliers**. `Cargo.lock`
  untouched. Tests stay **inline** (R5 split the *shared fakes* module, not the inline `#[cfg(test)] mod tests`
  blocks — those stay with their code). Placeholder modules + stub crates untouched. The stray root
  `package-lock.json` was **not staged** (user decision: leave it). **R5 done; stopped for review before R6**
  per the agreed sequence.

### Cleanup R4 landed — purged demo scaffolding from the pure core (2026-06-19)
- **Baseline re-confirmed green first** (the start-and-end gate): `just lint && just test` → **106 tests**
  (Rust **96** / UI **10**); clippy `-D warnings`, rustfmt, tsc, ESLint, Prettier, dep-guard pass; file-size
  guard warns (non-gating) on the **one** outlier `core/testing.rs` (527 — R5 territory). R3 reviewed before
  proceeding (sound: `CorePorts`/builder, single composition root, no `too_many_arguments`, public surface
  byte-stable).
- **R4 executed (commit `65cf819`, one reviewable commit per the per-R-phase rule).** `core::facade` carried
  demo scaffolding in the *pure* core: `spawn_demo_process` + the `DEMO_PROJECT`/`DEMO_COMMAND` consts + a
  `std::env::current_dir()` call (`facade.rs`) — host/demo concern, kept alive only by
  `pty/tests/integration.rs` and duplicating `app/src/demo.rs`. Purged:
  - **Removed `spawn_demo_process` + `DEMO_PROJECT`/`DEMO_COMMAND` + the `std::env::current_dir` call** from
    `core::facade`, and trimmed the now-unused imports (`std::collections::BTreeMap`, `std::path::PathBuf`,
    `ProcessId`/`ProjectId`, `PtySize`/`SpawnSpec`, `ProcessKind`, `Registration`). A repo-wide grep confirms
    `core/src` now contains **zero** `std::env`/`std::process`/`current_dir` and no `spawn_demo_process`
    anywhere.
  - **Single-sourced the seam into `core::testing::terminal_registration(project, name, command)`** — the
    minimal launched-terminal `Registration` fixture (no `std::env`; `working_dir: "."`), the **first real
    cross-crate consumer** of the `testing` feature R1 set up. Used by both the facade unit test and the pty
    integration test (DRY, §15).
  - **The integration test (`facade_runs_the_full_thread_with_real_spawner_and_clock`) still proves the same
    path** — real `PtyProcessSpawner` → `TokioClock` → `Facade` → actor → `stop` → `Stopped` snapshot — now
    building its own `Registration` via the helper and additionally asserting the ungated start succeeds (its
    real coverage is preserved, not weakened).
  - **The facade unit test** (was `spawn_demo_registers_and_runs_a_process`, the demo-seam test) is renamed
    `the_facade_registers_starts_and_stops_a_process` and rewritten to register via the helper — keeping the
    register→start→stop-through-the-façade coverage at the fake-spawner level (no test retired; count holds).
- **Demo seeding now lives ONLY in the `app` adapter** (`app/src/demo.rs`, its own `DEMO_PROJECT` const,
  untouched) — the correct home per the composition-root rule.
- **Pure structural / dead-code removal** — no supervisor/FSM/trust-gate/port-trait/logic change; the only
  behavior moved is where the demo registration is built. **Public surface loses only the genuinely-dead
  `spawn_demo_process` method**; `lib.rs` re-exports are byte-for-byte unchanged.
- **Verification (honest).** `just lint && just test` green before and after: **106** (Rust **96** / UI **10**),
  unchanged. The load-bearing integration test re-run in isolation passes (`cargo test -p soloist-pty --test
  integration facade_runs_… → 1 passed`). File-size guard still reports **one** outlier — `core/testing.rs`
  grew 527 → **547** from the small shared helper (still R5's split target; non-gating). `Cargo.lock` untouched.
  Tests stay **inline**; placeholder modules + stub crates untouched. The stray root `package-lock.json` was
  **not staged** (user decision: leave it). **R4 done; stopped for review before R5** per the agreed sequence.

### Cleanup R3 landed — `CorePorts` parameter object + single composition root (2026-06-19)
- **Baseline re-confirmed green first** (the start-and-end gate): `just lint && just test` → **106 tests**
  (Rust **96** / UI **10**); clippy `-D warnings`, rustfmt, tsc, ESLint, Prettier, dep-guard pass; file-size
  guard warns (non-gating) only on `core/testing.rs` (527 — R5 territory). R2 reviewed before proceeding.
- **R3 executed (commit `71eafac`, one reviewable commit per the per-R-phase rule).** The two
  `#[allow(clippy::too_many_arguments)]` escapes (`facade.rs:51` on `Facade::new`; `supervisor.rs:78` on
  `Supervisor::new`, which took 7 `Arc<dyn Port>` + the bus) are **removed** by bundling the port set into a
  parameter object:
  - **`core::ports::CorePorts`** (+ **`CorePortsBuilder`**) — a struct of the 7 `Arc<dyn Port>` the core is
    built over. Required adapters (`spawner`/`clock`/`trust`/`projects`, no meaningful absence) are the four
    `CorePorts::builder(..)` args; the **optional driven subsystems** (`locks`/`runtime`/`orphan_control`)
    **default to their `Noop` port** and are overridden via chained setters (`.runtime(..)`/`.orphan_control(..)`).
  - **`Facade::new(CorePorts)`** (was 6 args) and **`Supervisor::new(&CorePorts, bus)`** (was 7 args) now take
    it. Adding a future port = **one field on `CorePorts`** (+ a builder setter if optional), not another
    constructor parameter threaded through every call site.
- **Builder chosen over a plain public-field struct (decision, recorded).** The builder's Noop defaults mean a
  *future* optional port (Notifier P6, Summarizer P7, …) is added with a default and **existing composition
  roots/tests don't change** — matches `plan/06` §8/§5.2. A plain struct would force every call site to spell
  out each new Noop. (plan/06 §7 R3 already specified "and a builder"; the prompt's "if it reads cleanly" — it
  does.)
- **`ports.rs` split into a folder to avoid a new god-file.** Adding the bundle to `ports.rs` pushed it to
  **412** non-test lines (a *new* >400 outlier — unacceptable in a cleanup phase). Converted `ports.rs` →
  **`ports/mod.rs`** (the port *traits*, ~338 lines) + **`ports/bundle.rs`** (the `CorePorts` composition
  object, 83 lines), keeping the path `crate::ports::CorePorts` identical (zero import churn; `mod.rs`
  re-exports). `git mv` preserved history. File-size guard back to **one** outlier (`testing.rs` 527).
- **Pure structural change** — no behaviour, FSM, trust-gate, or port-trait change. The one test-shape wart:
  the supervisor test harness (`test_support.rs`) now supplies a `FakeProjectRepo` it doesn't use, because
  `Supervisor::new(&CorePorts)` reads a *subset* of the full core port set — acceptable for one unified
  parameter object. **Public surface gains only** `CorePorts`/`CorePortsBuilder` in `lib.rs`'s `ports`
  re-export; every existing export (`Facade`/`Supervisor`/`Registration`/…) is byte-for-byte unchanged.
- **Docs (R3 deliverable, in the same commit).** Documented `app::build_facade` as **the single composition
  root** (exactly one per binary; optional subsystems default to their `Noop` port) in **`CLAUDE.md` §16** +
  **`plan/06` §8**, and **cleared the "to add (R3)" marker** on the Parameter Object/Builder row in
  **`ARCHITECTURE.md` §3** + **`plan/06` §4**.
- **Verification (honest).** `just lint && just test` green before and after: **106** (Rust **96** / UI **10**),
  unchanged. `grep too_many_arguments` over the tree is **clean** (no allow anywhere). clippy `-D warnings`
  clean; dep-guard green (`CorePorts` lives in `core`, bundles core ports — no adapter leaks in). `Cargo.lock`
  untouched. Tests stay **inline**; placeholder modules + stub crates untouched. **R3 done; stopped for review
  before R4** per the agreed sequence.

### Cleanup R2 landed — split `supervisor.rs` into cohesive submodules (2026-06-19)
- **Baseline re-confirmed green first** (the start-and-end gate): `just lint && just test` → **106 tests**
  (Rust **96** / UI **10**); clippy `-D warnings`, rustfmt, tsc, ESLint, Prettier, dep-guard pass; the
  file-size guard warned (non-gating) on `core/testing.rs` (527) **and** `core/supervisor.rs` (490).
- **R2 executed (commit `c04859a`, one reviewable commit per the per-R-phase rule).** `supervisor.rs` was
  490 non-test code lines (+573 inline tests), over the ~400 smell. Pulled cohesive concerns into new
  `crates/core/src/supervisor/` submodules, leaving the root as the thin C2 published surface (per-process
  lifecycle `start`/`stop`/`restart`/`register`/`shutdown`, the terminal-I/O surface, `guard_trust`/
  `launch_actor`/`actor_ports`, and `apply_transition`):
  - **`registration.rs`** — the `Registration` input type + its `command`/`launched` constructors.
  - **`bulk.rs`** — `StartSummary` + `start_all`/`stop_all`/`restart_running`.
  - **`reconcile.rs`** — `reconcile_orphans` + `adopt_orphan`.
  - **`test_support.rs`** — the shared `#[cfg(test)]` `Harness` + helpers (`harness`/`spawn_spec`/
    `command_spec`/`terminal`/`next_to`/`next_change`/`wait_all`/`status_of`/`PROJECT`), so each
    submodule's `#[cfg(test)] mod tests` builds against **one** fixture set (DRY, §15) — not relocated to a
    `tests/` dir (tests stay inline per the locked decision).
- **Inline tests moved WITH their code:** `bulk` owns its 3 tests, `reconcile` its 5 (+ `orphan_record`/
  `next_orphans` helpers), the **14** lifecycle/terminal/panic tests stay in the root. `registration.rs`
  has no tests (its constructors are exercised indirectly — no pretend test added, §15).
- **Pure structural move** — no behaviour, signature, or logic change. **Public surface unchanged:**
  `lib.rs:61` `pub use supervisor::{Registration, StartSummary, Supervisor, SupervisorError}` is byte-for-byte
  untouched (`Registration` re-exported from `registration.rs`, `StartSummary` from `bulk.rs`, the rest defined
  in the root). `lib.rs` not touched at all.
- **File-size-guard fix (necessary, not cosmetic):** the guard counts non-test lines as everything *before the
  first* `#[cfg(test)]` attribute. The shared `mod test_support;` declaration must therefore sit at the **test
  boundary** (bottom of `supervisor.rs`, with `mod tests`), not near the top — a top placement made the guard
  read the root as 22 lines and silently stop measuring it. Now it correctly reads **331** non-test lines.
- **Verification (honest).** `just lint && just test` green before and after: **106** (Rust 96 / UI 10),
  unchanged. clippy `-D warnings` clean (one needed fix in `bulk.rs` tests: dropped the unused `use super::*`
  glob and added `use crate::ports::TrustRepo` so `set_trusted` resolves — the trait used to arrive via the
  root test module's glob). No supervisor source file now exceeds the ~400 smell: root **331**, `actor.rs`
  **361** (untouched), `registry.rs` 248, `test_support.rs` 133, `reconcile.rs` 77, `adopt.rs` 78, `bulk.rs`
  58, `registration.rs` 76. The remaining guard outlier is `core/testing.rs` (527 — R5 territory). `Cargo.lock`
  untouched. **R2 done; stopped for review before R3** per the agreed sequence.

### Cleanup R1 landed — reusable `core::testing` behind a `testing` feature (2026-06-19)
- **Baseline re-confirmed green first** (the agreed start-and-end gate): `just lint && just test` →
  **106 tests** (Rust **96** / UI **10**); clippy `-D warnings`, rustfmt, tsc, ESLint, Prettier, dep-guard
  pass; the R0 file-size guard warns (non-gating) on `core/testing.rs` + `core/supervisor.rs`.
- **R0 reviewed before proceeding (sound).** `scripts/check-file-size.sh` is warn-only (`set -uo pipefail`,
  no `-e`, unconditional `exit 0` in both branches), measures **code** size (skips `tests/` + `*.test.ts(x)`,
  excludes a Rust file's inline `#[cfg(test)]` module), and is wired into `just lint` (after the dep-guard)
  + the CI `check` job. Confirmed it warns without failing the gate.
- **R1 executed (commit `4c80eb7`, one reviewable commit per the per-R-phase rule).** The DRY gap was that
  `core::testing` (the `MockClock`/`FakeSpawner`/`FakeTrustRepo`/`FakeProjectRepo`/`FakeRuntimeState`/
  `FakeOrphanControl`/`RecordingLockReleaser` fakes) was `#[cfg(test)] mod testing;` — **private to core's own
  tests**, so `store`/`pty`/future adapters could not reuse it (`plan/06` §6). Fix:
  - `crates/core/src/lib.rs`: `#[cfg(test)] mod testing;` → **`#[cfg(any(test, feature = "testing"))] pub mod testing;`**.
  - `crates/core/Cargo.toml`: new **`[features] testing = []`** (off by default — the fakes never compile into a
    production build).
  - `crates/store/Cargo.toml` + `crates/pty/Cargo.toml`: dev-dep **`soloist-core = { path = "../core", features = ["testing"] }`**.
- **Two lint-correctness fixes were required** because exposing `testing` as a real `pub` lib module subjects it
  to core's production clippy (under `cargo clippy --workspace --all-targets`, the `testing` feature is unified
  onto core's **lib** target, which compiles `not(test)` → `deny(clippy::panic)` active over `testing.rs`; it was
  previously `#[cfg(test)]`-exempt). Both idiomatic, both in `testing.rs`: added an **`impl Default for MockClock`**
  (`new_without_default`, matching every other fake) and a **scoped `#[allow(clippy::panic)]`** on the one
  `FakeSpawner` arm that panics by design to drive panic-isolation. The core no-panic gate for *production* code is
  unchanged (the deny stays `not(test)`; only the test fake is locally exempted).
- **Verification (honest).** No fake defined twice (grep of store/pty/app for `Mock*`/`Fake*`/`Recording*` is
  clean — they never re-rolled fakes; R1 is the *enabling* refactor, not a de-dup). **Reachability proven**: a
  pair of ephemeral integration tests (`crates/{pty,store}/tests/_r1_reach.rs`) that `use
  soloist_core::testing::{MockClock, FakeSpawner, FakeTrustRepo}` compiled and ran (`cargo test … --test
  _r1_reach` → `2 passed`), then were **deleted** (committing them would be vanity tests, §15). The first *real*
  cross-crate consumer lands in **R4** (pty integration test builds its `Registration` via a `core::testing`
  helper) and the future mcp/httpapi adapters. `just lint && just test` green before and after: **106** (Rust 96
  / UI 10), unchanged. `Cargo.lock` untouched (path-dep features don't change it; no `cargo update`). Tests stay
  **inline** (R1 changed *who can reach* the fakes, not *where tests live*). **R1 done; stopped for review
  before R2** per the agreed sequence.

### Phase-5 runtime baseline verified (render) + cleanup R0 landed (2026-06-19)
- **Baseline gate re-confirmed green:** `just lint && just test` → **106 tests** (Rust **96** / UI **10**);
  clippy `-D warnings`, rustfmt, tsc, ESLint, Prettier, dep-guard all pass. This is the pre-refactor safety net.
- **GUI observed at runtime for the first time — it RENDERS (evidence: screenshots).** Ran `just dev`
  (`GDK_BACKEND=x11`, host `DISPLAY=:0`); window **"Soloist v0.1.0"** came up (Vite ready, app process
  running). Confirmed on screen: the **grouped sidebar tree (I1)** with the demo stack — **Agents**(1)
  `assistant`, **Terminals**(1) `shell`, **Commands**(2) `build`/`web` — all **Stopped** (hollow grey dots),
  matching the acceptance criterion. Selecting a process updates the **pane header** (name + status + ▷↻□
  controls) and a stopped process shows the in-pane prompt *"This process hasn't started yet. Press Start to
  run it."* **The `freezePrototype` blank-window bug is confirmed fixed** — React mounted and xterm imported
  without throwing. (Screenshots were captured to `/tmp/soloist_*.png` — transient, not committed.)
- **Terminal ECHO is NOT verified — and not claimed.** To see echo a process must be **started**, but no
  **synthetic** click (xdotool/XTEST) on the actual controls (Start-all, per-row ▷, pane-header ▷, group
  collapse chevron) activated them — while **pure-frontend row-selection clicks did** register. Click
  coordinates were confirmed exact (no display scaling; `getmouselocation` lands on the window; the
  pane-header ▷ was hit dead-on, verified via a cropped pixel check). Most likely an **XWayland→WebKitGTK
  synthetic-input quirk** (XTEST events not activating `<button>`/Radix handlers), **but a real control bug
  is not ruled out.** **User decision (asked explicitly): "Accept render, proceed to R0"** — echo + whether a
  real human click starts a process is to be **confirmed manually before R2** (the first structural edit). If
  a human click also fails to start a process, that is a Phase-5 control bug to fix before refactoring.
- **Cleanup roadmap R0 landed** (commit `ea4bad1`, one commit per the per-R-phase rule). R0's blueprint docs
  (`plan/06`, `CLAUDE.md` §16, `ARCHITECTURE.md`) were already merged in the 2026-06-18 session; the only
  remaining R0 item was the guardrail: added **`scripts/check-file-size.sh`** — a **warn-only** (always
  `exit 0`, non-gating) signal for the **~400 non-test-line split smell**. It scans tracked `.rs`/`.ts`/`.tsx`
  sources, skips dedicated test files (`tests/`, `*.test.ts(x)`), and for Rust excludes the inline
  `#[cfg(test)]` module so it measures **code** size. Wired into **`just lint`** and the **CI `check` job**
  (after the dep-guard). It reports the current outliers: **`core/testing.rs` 519** (shared test fakes — R1/R5
  territory) and **`core/supervisor.rs` 490 code lines** (the **R2** split target; `#[cfg(test)]` at line 491,
  matching the roadmap's "491 code lines"). `just lint && just test` green before and after. **R0 done; stopped
  for review before R1** per the agreed sequence.
- **Stray untracked file flagged, not touched:** `package-lock.json` at the repo root (env showed
  `uncommitted=1`). It is **not mine** and the project uses **pnpm** (`crates/app/ui/pnpm-lock.yaml`) — left in
  place. Likely npm cruft to `rm` or add to `.gitignore`; flagged for a user decision, not actioned this session.

### Architecture blueprint + cleanup roadmap authored (2026-06-18, docs only — awaiting review)
- **User goal:** before new features, fully clean up / organize the codebase for long-term discipline —
  clear domain separation, reuse, single source of truth, honest tests, and **architecture rules that tell
  future sessions how to architect changes** so adapters (MCP/tools/agents/skills) can be added/removed
  without the app rotting. Asked for a comprehensive, **phased** plan file first; **no code yet**.
- **Research done (no fabrication):** read the full plan corpus (`00`–`05`, glossary, all 14 phase files)
  + the live tree (core/store/pty/app + frontend) + targeted web research (Rust test layout; shared-fixture
  patterns; hexagonal pluggability). Census facts: 8 crates; core has real C1–C3 + C8 and **7 empty
  placeholder modules** (agents/coordination/identity/idle/metrics/notify/portscan → their future
  contexts) + **4 stub adapter crates** (mcp/httpapi/cli/ipc); `supervisor.rs` = 491 code + 573 inline
  test lines (the one >400 outlier); `core::testing` fakes are `#[cfg(test)]`-**private** (not reusable by
  store/pty/future adapters — the real DRY gap); two `#[allow(too_many_arguments)]` (facade.rs:51,
  supervisor.rs:138); `core::facade::spawn_demo_process` is demo scaffolding in the pure core kept alive
  only by `pty/tests/integration.rs:262` (duplicates `app/demo.rs`); frontend split is already clean.
- **User decisions (locked this session):** (1) **tests stay inline** — trim pretend/oversized, do **not**
  relocate (reverses the opening "no tests in rust code"; user confirmed via the option); (2) **keep** the
  empty core modules **and** the 4 stub crates as **documented placeholders**; (3) **plan-first, then
  review** — write the doc + `CLAUDE.md` rules, stop before touching code.
- **Authored `plan/06-codebase-blueprint-and-cleanup.md`** (new): crate topology + placement map (incl. the
  one-allowed placeholder-module rule), design-patterns-in-practice catalog (with triggers + where), the
  *add-a-X* recipes (context behavior / port+adapter / MCP tool / HTTP-CLI-Tauri command / `DomainEvent` /
  UI), single-source + the test-fakes-reuse fix, the **adapter-independence guarantee** ("remove MCP, app
  survives" = independent crates + Null-Object `Noop` ports + one composition root), and the **R0–R6
  cleanup roadmap** (R0 blueprint+file-size guard · R1 reusable `core::testing` via a `testing` feature ·
  R2 split `supervisor.rs` · R3 `CorePorts` parameter object, kill both `too_many_arguments` · R4 purge
  core demo scaffolding · R5 honest-test audit · R6 doc/ledger converge). Each R-phase starts/ends `just
  lint && just test` green.
- **Updated `CLAUDE.md`:** added the doc to the canonical table + source-of-truth hierarchy (slot 4b,
  below `04`), and **new §16 "Architecture & structure rules — how to build any change"** (the load-bearing
  invariants, pointing to `plan/06`).
- **No code logic changed; gates not re-run** (docs only). **Awaiting user review of `plan/06` + §16
  before executing R0.**

### Codebase-discipline audit + plan-enforced gate (2026-06-18)
- **Audit (no code changed).** Line-count + structure pass over `crates/`. The codebase **already
  honors** the discipline: hexagonal layering holds (dep-guard green), bounded contexts intact,
  single-source domain types (`domain.ts`), reused components, files small — **TS** max **121** lines
  (`useTerminal.ts`); most **Rust** under ~330. Single notable outlier: `crates/core/src/supervisor.rs`
  = **1064 lines, but 491 code + 573 in-file tests** — the C2/C8 facade (~15 methods) + `Registration`/
  `StartSummary`/errors, with `actor`/`registry`/`adopt` already in `supervisor/`. Not a true god-file;
  it's the largest core module and a **candidate split** (e.g. pull bulk-ops + `reconcile_orphans` out),
  not urgent. `testing.rs` (519) is shared test-support (fakes) — acceptable, splittable later.
- **Encoded the discipline as an enforced gate (user request: "include in plan").** Avoided a second
  source of truth — `CLAUDE.md` §15 stays authoritative; the plan now adds the **enforcement hooks**:
  - `CLAUDE.md` §7 — added **definition-of-done item 6**: codebase-discipline gate (separation, reuse,
    small single-purpose files, clean) must pass; a regression is "not done" even if tests pass.
  - `plan/04` §10 — expanded the soft "module size discipline" bullet into a concrete **Codebase
    discipline** block (domain/service separation; single-source + DRY; small files with a **~400
    non-test-line split smell**; reusable component frontend; no dead code), pointing to `CLAUDE.md` §15.
  - `plan/04` §15 — new **Codebase discipline gate** checklist (mirrors the §14 longevity checklist) that
    every phase verifies; notes an optional future `scripts/check-file-size.sh` in `just lint`/CI.
- **Open follow-up (recorded below):** optionally split `supervisor.rs` and add the file-size lint —
  flagged for a decision, not done this session (touches Phase-3/4 verified-pending code).

### Research — Claude Code OAuth/interactive shell + full soloterm re-research (2026-06-18)
- **No code changed — research + plan-doc updates only** (user request).
- **Q: make Soloist "work with Claude Code using native OAuth login + an interactive shell."** Findings,
  no fabrication:
  - **Claude Code does its own auth; Soloist does/should manage none.** Native OAuth is the CLI's `/login`
    browser/loopback flow (paste-code fallback), writing **`~/.claude/.credentials.json`** (Linux:
    plaintext, mode 0600 — *its* file). Other methods: `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`,
    `apiKeyHelper`, `CLAUDE_CODE_OAUTH_TOKEN` (from `claude setup-token`), cloud providers. Source:
    [code.claude.com/docs/en/authentication](https://code.claude.com/docs/en/authentication) (fetched 2026-06-18).
  - **This matches Solo exactly** — now **citable** ([agents](https://soloterm.com/agents)): *"Solo does
    not farm OAuth tokens or route your work through a vendor account"*; agents *"keep using whatever
    accounts, subscriptions, API keys, and auth flows you already set up."*
  - **Requirement is largely already satisfied by our architecture.** The interactive PTY (Phase 4,
    `test -t 1`/`read x` verified) + xterm pane (Phase 5) is exactly the substrate the OAuth REPL needs.
    The missing piece is **first-class agent launch = Phase 7** (Not started). The only rule: launch the
    agent **interactively** (never `-p` for the main process) and pass env through (`$DISPLAY`/`BROWSER`,
    `ANTHROPIC_*`). No credential plumbing — we run the agent **on the host**, where the CLI's creds
    already live.
  - **`madarco/agentbox` researched** (cloned to `/tmp/agentbox-research`). It always runs the agent in
    an **isolated box** (Docker/Vercel/E2B/Hetzner/Daytona), so it must **stage/forward** host
    `~/.claude/.credentials.json` into the box (symlink pivot, token-refresh backups) + tmux+node-pty
    attach. **~90% of that is N/A for us** (local execution); the one transferable idea is launching
    `claude "<seed prompt>"` interactively — already how Phase 7 plans to launch.
  - **The plan never named agent auth** (grep of `plan/`: every "login" = unix login shell, every "auth"
    = the HTTP `X-Soloist-Local-Auth` header). Recorded it now: **`05` §6** (Solo's stance, cited),
    **matrix `E8`** (v1), **phase-07** (scope/Task 3/acceptance/risk). No new divergence (we match Solo).
- **Full soloterm re-research pass (vs `05`/`02`).** `05` was already very thorough (and *more* complete
  than the new pass on the 10/60s limit, port 24678, `X-Solo-Local-Auth`). **Genuinely untracked Solo
  features found** (all verified verbatim against [changelog](https://soloterm.com/changelog)) and added
  as **`later`** (non-gating, no v1 gold-plating):
  - **Activity Monitor view** (v0.6.1) — cross-project flat/tree process+subprocess monitor, filters,
    sortable CPU/mem/port columns, quick actions → `05` §10 + matrix **`I12`** (+ descendant-stat data
    **`D12`**).
  - **Prompt templates** (v0.8.2) — UI view + optional MCP tools (placeholders, global/project scope) →
    `05` §10/§7 + matrix **`F14`** (MCP) and **`I13`** (UI).
  - **Nested child-agent display** (v0.6.4) — spawned agents nested under parent in sidebar → matrix
    **`I14`** (`05` §10 already noted "nested child agents").
  - **Dropped as unverified:** the subagent's "Kitty keyboard protocol" claim did **not** confirm on the
    changelog re-fetch — not added (no fabrication).

### Phase 5 build — Dashboard UI / interactive core slice (2026-06-16)
- **Session scope (user-approved):** the "interactive core slice" — `DESIGN.md` + the Tauri/TS plumbing +
  sidebar/status/controls/live-status + the **xterm.js terminal pane**. **Deferred** to a focused
  follow-up: trust dialog (A6/A9 UI), orphan dialog (B8 UI), project load/switch + the deferred
  `ConfigEngine → register → reconcile_orphans → start_all` wiring. Color-blind-safe status encoding
  **confirmed** (shape+color+label); **neutral + restrained azure accent** visual direction confirmed.
- **`DESIGN.md` seeded + approved (hard §5 prerequisite — it was missing).** Ran `/impeccable document`:
  "The Instrument Panel" north star; cool-slate near-monochrome surface + one azure accent
  (`oklch(0.55 0.13 245)`); **saturated color spent only on process status**, mapped 1:1 to `ProcStatus`
  as glyph+color+label. `index.css` implements the OKLCH tokens — the azure accent **replaces the shadcn
  neutral/purple `primary`/`sidebar-primary`** (fixes the PRODUCT.md "no purple" anti-reference), adds the
  `--status-*` palette, tightens radius 0.625rem→0.375rem, adds Geist Mono. The skill offered its v3.6.0
  update (per its directive) → user chose **skip** (stay v3.5.0). The `.impeccable/design.json` sidecar is
  **not** generated yet (deferred until components stabilise — recorded follow-up).
- **Tauri adapter (skills used: `tauri-calling-rust` / `-frontend` / `tauri-ipc`).** `lib.rs` split into
  small modules: `commands.rs` (thin wrappers → one core behaviour), `pty_bridge.rs` (single-slot
  forwarder lifecycle), `demo.rs` (app-level demo seed). **PTY streaming uses
  `tauri::ipc::Channel<Vec<u8>>`** — the skill's high-throughput single-consumer primitive (→ `Uint8Array`
  on the JS side), **not** events (which the skill states are not for high throughput). The scrollback
  replay is sent as the **first** Channel message so the core's atomic attach (no gap/dup, C9) survives
  the IPC boundary; `PtyBridge` aborts the prior forwarder on re-attach so no streaming task leaks. New FE
  deps (verified maintained; legacy `xterm` is deprecated → `@xterm/xterm`): `@xterm/xterm` 6.0.0,
  `@xterm/addon-fit` 0.11.0, `@fontsource-variable/geist-mono` 5.2.8. No new Rust deps.
- **One core change (single-source):** `DomainEvent::ProcessSpawned` gains `project: ProjectId` — the
  event must carry what `ProcessView` needs to group, since a process registered after the UI mounts
  arrives only as an event. Emitted in `supervisor::register`; no core test matched the variant.
- **Frontend architecture (§15).** `domain.ts` is the single TS mirror (ProcessView + `project`/
  `exit_code`; the full 8-variant `DomainEvent` union incl. ConfigChanged/Terminal*/OrphansFound — mirrored
  even though their dialogs are deferred, so the reducer switch stays exhaustive). `lib/status.ts` is the
  single ProcStatus→display map. `store/` keeps pure reducers (`projection`, `grouping`) + `useProcesses`
  (snapshot-then-deltas; actions route to the core, never optimistic) + persisted collapse. Components are
  small/presentational; `ProcessControls`/`StatusIndicator` reused across the sidebar and terminal header.
  Removed the superseded `ProcessList`/`StatusBadge`.
- **Demo stack (`demo.rs`, app scaffolding, temporary).** Registers one Agent + one Terminal (ungated
  `bash`) + two **pre-trusted** Commands (a chatty ticker + a build-then-idle) under demo project 1, so all
  three sidebar groups render and the controls/terminal are exercisable **without** the deferred trust
  dialog. Auto-start off → all show `Stopped` at launch (matches the acceptance). Replaced when
  project-load lands.
- **Verification reality (honest, §10/§12).** `just lint && just test` green: **106 tests** (Rust 96 / UI
  10; UI +4 real tests — grouping ×3, projection updated). tsc strict + clippy `-D warnings` + dep-guard
  green. **NOT yet observed at runtime:** the rendered dashboard, live terminal echo, and the Playwright
  e2e — **GUI auto-launch was denied** and **Playwright/`tauri-driver` are not installable offline**. So
  this is `Done — pending verify`, not Verified. Manual path: `just dev` (host has `DISPLAY=:0`).
- **Blank-window bug found + fixed (user-reported on first launch).** Console showed `TypeError: Attempted
  to assign to readonly property` at **xterm's module-load** (`@xterm_xterm.js:1698`, the namespace line
  `o.toString = s`). Cause: Phase 0's speculative **`freezePrototype: true`** (`tauri.conf.json` security)
  `Object.freeze`s `Object.prototype`, so the inherited `toString` is non-writable and xterm's plain
  assignment throws in strict mode → the import fails → React never mounts → blank window. (Phase 1 never
  imported xterm, so it never tripped.) **Fix:** `freezePrototype: false` (Tauri's documented default;
  confirmed via the `tauri-configuration` skill). Tradeoff: drops one prototype-pollution hardening; our
  CSP, capabilities, and IPC scope are unaffected. The config is embedded via `generate_context!`, so the
  **binary was rebuilt**. Revisit only if xterm changes the namespace pattern (unlikely).

### Phase 4 review fixes (2026-06-15)
Reviewed the Phase 4 PR (commit `c234b64`, range `16b7229..c234b64`) across every dimension via
`REVIEW-PROMPT.md`. Library usage was verified against docs (context7 + docs.rs: `portable-pty` 0.9
`openpty`/`CommandBuilder` env-inherit/`ExitStatus::signal()→Option<&str>`; `vte` 0.15 `advance(&[u8])` +
`Perform` dispatch). No blockers. **Applied every Should-fix and Nit**; gates re-verified green (`just
lint`, `just test` — **102 tests**, core 74 / pty 10 / store 12 / UI 6):
- **Flaky reap test fixed (should-fix).** `forceful_kill_reaps_a_signal_resistant_child` asserted
  `killpg→ESRCH` once, racing init's *asynchronous* reap of the `sleep` grandchild reparented after the
  group SIGKILL — reproduced ~2/20 under CPU contention (`left: None, right: Some(ESRCH)`). Added a polling
  `await_group_gone(pgid)` helper (≤2 s) and routed all three group-reap asserts through it
  (`forceful_kill`, `spawns_into_a_group`, `start_stop_fifty`). Re-stressed: **0/40** suite runs failed.
- **Trailing PTY output no longer lost (should-fix).** `drain_output` used `try_recv` (only already-buffered
  chunks), racing the adapter's reader thread vs the reaper — final pre-exit bytes (e.g. a crash line) could
  drop, contradicting its own doc. Now a **bounded async drain**: `select!` `recv()` (biased) until the
  channel closes (EOF → all captured), bounded by `DRAIN_GRACE` (100 ms) so a forked grandchild holding the
  slave open can't wedge the actor.
- **No more blocking I/O on the async actor (should-fix, §6/§8).** (a) `MasterIo::write`/`resize` now run the
  blocking PTY ops via `spawn_blocking` (handles `Arc<Mutex<…>>`-shared; added `rt` to `pty`'s tokio); a
  stuck write to a non-reading child no longer stalls the runtime. (b) `record_orphan`/`forget_orphan` offload
  the runtime-state file write via `spawn_blocking` (awaited); recording now happens **before** the `Running`
  announcement so a crash right after still leaves a reconcilable record.
- **Global scrollback cap implemented (should-fix, §3 invariant).** Added `ScrollbackBudget` (a shared
  relaxed-atomic byte counter, default **16 MB**) across all per-process raw buffers: each buffer accounts
  its bytes, sheds its oldest when the aggregate is over budget, and releases on `Drop`. Per-process 256 KB
  caps unchanged. Two new tests (aggregate bound; drop frees the budget).
- **Reconcile duplicate-identity guard (nit).** Two live leftover groups sharing `{root,name,command}`: the
  second now **surfaces** for a user decision instead of being silently dropped after losing the
  `begin_launch` claim. New test `reconcile_surfaces_a_duplicate_that_loses_the_adoption`.
- **Comment policy (nit, §8).** Removed the two `Phase-5` phase-number references from `events.rs`
  (`OrphansFound`) and `orphans.rs` (`OrphanInfo`) doc comments.
- **Locale-fragile assertion (nit).** `spawns_into_a_group…` asserted the exact `SIGTERM` *number*, which
  `signal_number` derives from the locale-sensitive `strsignal` description. Now asserts the robust property
  (`signal.is_some() && code.is_none()`); added a `pty` unit test covering the description→number mapping
  directly (locale-independent).
- **Doc drift (nit).** Annotated phase-04 "Interfaces" + `plan/01` (the `PtyOutput`/`subscribe_pty` sketch
  never shipped — raw bytes ride a per-process broadcast via `attach_pty`); recorded the Task 8 env-hygiene
  reality (`TERM` set, env inherited, `COLUMNS`/`LINES` deliberately not exported — winsize is authoritative).
- **OSC test precision (nit).** `an_osc_title_and_a_bell…` now asserts **exactly one** bell (the OSC's BEL
  terminator is consumed, not rung), not merely "any".

### Phase 4 build — PTY & Terminal I/O / context C3 (2026-06-15)
- **Scope (user-approved):** work Phase 4 now and **fold Phase 3's deferred Task 4 (output capture / log
  ring) into Phase 4's PTY read loop** — the ring is built once, in final form, over the PTY (phase-04
  Task 2 + phase-03 Task 4 agree: "same buffer/event contract; Phase 4 swaps to PTY"), avoiding throwaway
  pipe capture. **B8 (orphan adoption) stays the one open Phase-3 v1 row** (independent of PTY I/O); Phase
  3 remains `In progress`. The xterm.js frontend (C8 `later`, phase-04 Task 9) → Phase 5 via `/impeccable`
  (DESIGN.md still unseeded), matching the Phase 2/3 frontend-deferral rhythm.
- **Library verification (no assumptions, §4):** confirmed `portable-pty` 0.9 + `vte` 0.15 APIs via
  context7 + docs.rs **before** coding. Key finding: portable-pty's `ExitStatus::signal()` returns a
  `strsignal` **description** ("Terminated"/"Killed"), locale-sensitive — the exact signal *number* isn't
  faithfully recoverable. Resolved by keying the actor's crash classification off `success()` (correct on
  a signal death) and mapping the description back best-effort (C-locale table + `Signal {n}` fallback);
  the number is inspected only by one adapter test, whose `signal == Some(SIGTERM)` assertion empirically
  passes on this host.
- **New deps:** `vte` 0.15 in **core** (pure ANSI parser; pulls only `arrayvec`+`memchr`, already in tree;
  dep-direction guard still green — vte is not a forbidden adapter); `portable-pty` 0.9 in the **pty**
  adapter (pulls `serial2`/`shell-words`/`downcast-rs`/`filedescriptor` + its own `nix` 0.28, a duplicate
  of our 0.29 — acceptable). Real `.deb`/AppImage size impact is **measured in Phase 12**, not guessed.
- **Port contract evolved (justified, like Phase 3's `SpawnSpec`):** `SpawnSpec` gains `size: PtySize`;
  `Spawned` gains `output: mpsc::Receiver<Vec<u8>>` (bounded → backpressure) + `io: Box<dyn PtyIo>`
  (write/resize); new `PtyIo` port. `FakeSpawner` updated + a `streams_then_exits` variant for the actor
  output-drain test.
- **Design decisions (recorded):**
  - **PTY bytes are a per-process broadcast, NOT a `DomainEvent::PtyOutput` on the main bus.** High-rate
    output must not flood the low-rate status stream or make status subscribers lag (§5 isolation, §8
    backpressure). Only low-rate OSC **title**/**bell** are `DomainEvent`s; raw bytes flow over
    `attach_pty`'s broadcast. A deliberate divergence from the phase-04 interface sketch.
  - **`subscribe_logs` (live `LogLine` stream) folded:** the `Ring<LogLine>` is exposed as a bounded
    snapshot (`rendered()`); live consumers use the raw `attach_pty` stream (lines are derived). Avoids a
    duplicate fan-out (§15 single-source).
  - **Rendered output is line-oriented, not a cell grid** — `KNOWN-DIVERGENCES.md` **D-3**. The frontend
    xterm.js is the real emulator (consumes the byte-exact raw buffer); the core's rendered text answers
    "what plain text printed" (exact for CLI output, approximate for cursor-addressed TUIs). Avoids a
    redundant grid emulator in core (§6).
  - **`attach_pty` is race-free:** the recorder publishes to the live stream *while holding the buffers
    lock*, so an attaching viewer sees each chunk in exactly one of {scrollback snapshot, live stream} —
    no gap, no duplicate (C9).
  - **Restart keeps the terminal buffers; a fresh stop-then-start resets them** (the actor `open`s the
    channel once per launch; restart-in-place reuses it).
- **Tauri:** no Tauri code this phase — phase-04 v1 is headless ("drive PTYs from Rust"). The terminal
  pane + `pty_write`/`pty_resize` commands + `PtyChunk`/`RenderedScreen` TS mirror land in Phase 5 via
  `tauri-calling-rust`/`tauri-calling-frontend` + `/impeccable`. The only app change was the one-line
  `PtyProcessSpawner` rename.

### Phase 3 B8 build — Orphan adoption (2026-06-15, same session)
- **Closed the last Phase-3 v1 row** (user chose "build B8 now" after Phase 4 landed green) → Phase 3 is
  now `Done — pending verify`.
- **Adoption reuses the existing actor (key design):** rather than a second actor type, an adopted orphan
  is driven through the normal actor by handing it a *synthesized* `Spawned` over the existing pgid — its
  exit future polls `OrphanControl::is_alive` on the `Clock` (resolving when the group dies), its control
  signals the group via `killpg`, its output is closed (the original PTY died with the previous run —
  historical output unrecoverable, matching Solo), its I/O is a no-op. The actor gained an optional
  `initial: Option<Spawned>` (first iteration uses it; restart re-spawns fresh). `supervisor/adopt.rs`.
- **Reconcile is a pure classifier (`orphans.rs`):** `classify(records, is_alive, matcher)` →
  adopt/surface/prune, unit-tested in isolation. `Supervisor::reconcile_orphans()` performs the side
  effects: adopt (re-attach to a resting registered command matched by project_root+name+command), surface
  (`DomainEvent::OrphansFound` — the Kill/KillAll/Leave dialog is Phase-5 UI; core only emits), prune
  (forget dead records). Adoption is **ungated** (the process is already running; we re-attach, not start —
  matches Solo).
- **New ports:** `RuntimeState` (record/forget/load; `NoopRuntimeState` default) + `OrphanControl`
  (is_alive/signal a pgid; `NoopOrphanControl` default) + `OrphanRecord`. The actor records on Running /
  forgets on each child-end. `Registration` gained `project_root` (the adoption identity).
- **Real adapters:** `store::FileRuntimeState` — a small **JSON file** (`runtime-state.json` in the data
  dir, **NOT SQLite** per plan/04 §7), mirrored in memory behind one lock (serializes concurrent actors),
  atomic temp-file+rename writes, tolerant of a missing/corrupt file; round-trip tested. `pty::PgidOrphanControl`
  — `killpg(pgid, None)` liveness (`Ok`/`EPERM`=alive, `ESRCH`=gone) + SIGTERM/SIGKILL; real-OS is_alive
  test. New dep `serde_json` in **store** (`OrphanRecord` gained serde derives); dep-guard green.
- **App:** recording is **live now** (`FileRuntimeState` + `PgidOrphanControl` in `Facade::new`). The
  reconcile-on-launch **call is deferred to Phase 5**: it must run *after* config commands are registered
  (so adoptable leftovers match instead of being mis-surfaced), and that registration wiring is Phase 5.
  Calling it now (demo-only app, no config commands) would only prune/surface — so the call lands with
  config-registration. Recorded in open threads.

### Phase 3 review fixes (2026-06-15)
Reviewed the Phase 3 PR (commit `cdb6367`, range `25d2e73..cdb6367`) across every dimension via
`REVIEW-PROMPT.md`; the Tauri-adapter review was grounded in the project-local `tauri-calling-rust`
skill + the official Tauri v2 docs (`RunEvent`, `async_runtime::block_on`, `App::run`). No blockers;
gates re-verified green (`just lint`, `just test`). Applied **every** finding:
- **Deterministic shutdown now wired (should-fix; `plan/04` §8, §3 no-orphans, phase-03 Task 8).**
  `Supervisor::shutdown()` existed but was never called or tested. `crates/app/src/lib.rs` now uses
  `Builder::build(…)?.run(|app, event| …)` and, on `RunEvent::ExitRequested`,
  `block_on(facade.supervisor().shutdown())` — so a normal quit reaps every managed **process group**
  rather than relying on `kill_on_drop` SIGKILLing only the bare child PID (which would leak a forking
  command's grandchildren). New core test `shutdown_stops_and_reaps_every_live_process` proves the
  await-each-actor contract on `FakeSpawner` + `MockClock`.
- **Comment-policy citations removed (should-fix, §8).** Dropped the two `plan/04 §8` plan citations
  from `supervisor.rs` doc comments (source carries no plan/doc citations).
- **phase-03 FSM diagram reconciled to the code (nit, §2).** The restart edge read `Running ─►
  Stopping ─► Starting`; the code (correctly, per the canonical enum) routes through the dedicated
  `Restarting` state, so the diagram now reads `Running ─► Restarting ─► Starting ─► Running`.
- **Shell resolution gains the passwd fallback (nit, `plan/05` §5).** `crates/pty` resolved `$SHELL →
  /bin/sh`; it now does `$SHELL → passwd entry → /bin/sh` via `nix::unistd::User::from_uid` (added the
  `user` feature to the existing `nix` dep — no new crate; `Cargo.lock` unchanged), so a desktop launch
  that does not export `$SHELL` still uses the user's real login shell. `-lc` vs Solo's `-ilc env`
  capture stays a Phase-11 deferral (documented in the adapter).
- **Liveness keyed off status, not a stale handle (nit, §15).** Actor handles are never reclaimed on
  completion, so `stop()` could return a false `true` and `live_in` over-reported. Added
  `ProcStatus::is_active()` (single source) and switched `stop()` + `live_in` to it; `with_live_actor`
  stays handle-based as the belt-and-suspenders shutdown set (awaiting a finished actor is a harmless
  no-op), with its doc corrected to say so.
- **Start path made race-free (nit, §8 single-writer).** The `is_active` check and the `→ Starting`
  transition in `start()`/`launch_actor` were not atomic, so two concurrent starts could briefly
  double-spawn. New `Registry::begin_launch` claims a resting process and moves it to `Starting` under
  one lock; `launch_actor` now returns whether it won the claim, and `start_all` reports `started` only
  for the ones it actually launched.

### Phase 3 build — Process supervisor / context C2, core slice B1–B7 (2026-06-15)
- **Session scope (user decision):** "Core first" — land **B1–B7 + A2/A6** fully tested this session;
  defer **B8 (orphan adoption)** and **Task 4 (output capture/log ring)** to a focused next session so
  B8 (the highest-risk sub-piece) gets dedicated test time. The phase stays `In progress` until those
  land; not marked done.
- **`Supervisor` (C2) built on the Phase-1 actor.** New `crates/core/supervisor/` (`registry` +
  `actor`) under the `supervisor` module root. Each managed process is one supervised `tokio` task with
  a bounded **mailbox** (`ActorMsg::Stop`/`Restart`, cap 4) — restart cycles the child *in place*
  (`Running→Restarting→Starting→Running`) under the same actor, so there is one owner per process. The
  registry's `Mutex` guards only the lookup map. Panic isolation retained (inner task + `is_panic()` →
  `Crashed` + lock release). `apply_transition` is a single shared FSM helper used by both the
  supervisor (reads `from` from the registry) and the actor (threads its local mirror) — DRY (§15).
- **Trust gate enforced in core (A6).** `start`/`restart`/`start_all` refuse an untrusted command
  variant via the shared `TrustRepo`; terminals/agents are ungated (`trust_variant: None`). Proven
  refused by **every** path (`an_untrusted_command_cannot_run_by_any_path`).
- **Fields honored at runtime (A2/B5).** The `pty` spawner now runs `$SHELL -lc <command>` in the
  resolved `working_dir` with per-process `env` layered onto the inherited env (process wins — the
  documented precedence). Verified on a **real** shell by exit code (`runs_a_command_in_its_working_dir_with_its_env`).
  `auto_start` gates `start_all` candidacy. (Full `$SHELL -ilc env` capture/caching stays Phase 11 / I10.)
- **`SpawnSpec` evolved** `{program,args}` → `{command, working_dir, env}` (a Phase-1 contract change,
  justified by B5). **`Spawned` unchanged** this session — the output channel lands with Task 4's ring.
- **FSM refinement:** added the `Crashed→Starting` edge — a user can restart a crashed command (matches
  Solo; the prior FSM only allowed `RestartExhausted→Starting`). Tested (`a_terminal_process_can_be_restarted`).
- **Exit classification (gap-decision, encoded in the phase FSM):** clean `exit(0)` → `Stopped`;
  non-zero code or an unsolicited terminating signal → `Crashed` (+ `exit_code` on
  `ProcessStatusChanged`). A user-initiated stop is a separate path and is always `Stopped`, even when
  escalated to SIGKILL. (Solo doesn't document the exact boundary; this matches the phase-03 FSM.)
- **Graceful group stop (B6):** SIGTERM→**5s grace** (mock-clock-driven, no real waiting)→SIGKILL→reap,
  always on the **process group**. Real-OS evidence: pgroup reaped, grandchildren reaped (`$SHELL -lc
  "sleep 30"`), a `trap '' TERM` shell escalates to SIGKILL, and **start/stop 50 processes leaves zero
  surviving groups** (the Phase-13 soak precursor).
- **Stop releases locks (B7):** the actor calls a `LockReleaser` port on **any** terminal transition
  (stop *and* crash), matching Solo's "locks auto-release when the owning process closes". Real impl is
  C6 (Phase 9); `NoopLockReleaser` until then. "Clears crash tracking" is the other half — deferred to
  Phase 6 (no restart/crash policy state exists yet to clear).
- **Façade (C8) now owns C2 + C1.** `Facade::new(spawner, clock, trust_repo, project_repo)` builds the
  `Supervisor` + `Projects`/`TrustStore`/`ConfigEngine` over **one shared `TrustRepo`** and one bus, and
  exposes `supervisor()`/`projects()`/`trust()`/`config()` accessors so adapters route to a single impl
  (no per-adapter reimplementation). The Phase-1 demo (`spawn_demo_process`) now registers + starts an
  ungated terminal through the **real** supervisor path (keeps the Phase-1 manual GUI verify valid).
- **Tauri touch (skill used).** Invoked the project-local **`tauri-calling-rust`** skill before editing
  `crates/app/src/lib.rs`; the only changes were `build_facade` (one `Arc<SqliteStore>` feeding the
  trust + project repos) and `stop_process` (now `facade.supervisor().stop`). Managed-state + async-
  command contract unchanged; `Facade` stays `Send + Sync`.
- **No new dependencies** (dev-only `tempfile` added to `crates/pty` for the cwd test — not shipped, no
  §6 size impact). No frontend changes (the TS `ProcessView`/`ProcessStatusChanged` mirror updates land
  with the Phase-5 UI wiring, as in Phase 2). Dep-direction guard green.

### Phase 2 review fixes (2026-06-15)
Reviewed the Phase 2 PR (`3601d6d`, range `7ef2334..3601d6d`) across all dimensions via
`REVIEW-PROMPT.md`. No blockers; gates re-verified green (`just lint`, `just test`). Applied every
finding:
- **Test-count evidence corrected (should-fix, §10).** The build note + commit message claimed "59
  tests (core 41)"; `cargo test` actually showed **60** (core **42**). The review-fix test below makes
  it **61** (core 42 / pty 3 / store 10 / UI 6) — every count in this ledger now matches the runner.
  (The commit message is already pushed and immutable; the ledger is the corrected record.)
- **`ConfigEngine::sync` single-writer + blocking-I/O contract documented (should-fix, `plan/04` §5).**
  The method releases its lock for file I/O + the trust lookup, so concurrent same-project calls could
  race the snapshot and double-publish `ConfigChanged`. Documented that it must be driven by one
  debounced writer per project and invoked off-thread (`spawn_blocking`); the Phase 6 watcher must honor
  this (open thread updated). No behaviour change — latent until the live watcher lands.
- **Removed speculative `Serialize` from `SoloYml`/`ProcessSpec` (nit, §15 YAGNI).** Nothing serializes
  the model (`ConfigChanged` carries only the name-based `ConfigSync`); Phase 5 re-adds it when wiring
  config to the UI. Dropped the now-dead `skip_serializing_if` field attributes with it.
- **Migration downgrade guard (nit).** `store::migrate` now refuses a DB whose `user_version` exceeds
  `SCHEMA_VERSION` (an older build opening a newer schema) instead of silently downgrading it, and writes
  the version only when advancing. New test `refuses_a_schema_newer_than_this_build` (store 9→10).
- **Doc/comment nits.** Dropped a `(ref §3)` plan citation from a `load.rs` test doc (§8); renamed
  `Trust::Trusted { variant }` → `{ variant_hash }` to match the documented enum (CLAUDE.md §3);
  refreshed the stale `testing.rs` module doc to mention `FakeTrustRepo`/`FakeProjectRepo`.

### Phase 2 build — Config & Projects / context C1 (2026-06-15)
- **Built C1 headless on the Phase 1 skeleton.** `crates/core`: `config/` split into `model` (types +
  documented defaults: `auto_start` default **true**, all else off/empty; `deny_unknown_fields`;
  `IndexMap` preserves `processes` order; `ProcessSpec::variant_hash`), `load` (pure `parse` + I/O
  `load`/`load_or_empty`; 1 MB cap; empty/comment-only = empty; typed `ConfigError`, never panics),
  `diff` (`ConfigSync` add/update/remove + **unambiguous rename** detection by command string), `sync`
  (`ConfigEngine`: content-hash skip → diff → `requires_trust` → emit `DomainEvent::ConfigChanged`).
  New modules: `hash` (SHA-256 `Hash`, hex round-trip, length-prefixed `Hasher`, `content_hash`),
  `trust` (`TrustStore` + the `Trust` enum), `projects` (`Projects` registry, canonicalized-root
  identity), `debounce` (`Debouncer`, pure Clock-driven quiet-window coalescer). New core ports
  `ProjectRepo`/`TrustRepo` + `ProjectRecord`; new `DomainEvent::ConfigChanged{project,diff,requires_trust}`.
- **`crates/store` grown to the repository pattern.** Split into `meta`/`projects`/`trust` modules +
  `migrate` (schema **v2**: `projects(id,root UNIQUE,name,icon)` + `trust(project_id→FK CASCADE,
  variant_hash)`); `foreign_keys` pragma now set on **both** durable and in-memory opens (so trust
  cascades). `SqliteStore` implements `Store`+`ProjectRepo`+`TrustRepo`.
- **Durable `ProjectId` (design decision).** Trust must persist across restart (A7), so a project's
  identity is its **canonical absolute root path** (natural key); the SQLite rowid is the durable
  `ProjectId`, reconstructed via `from_raw` on later runs. `ids.rs` doc updated: `ProjectId` is durable
  (store-assigned), `ProcessId` stays per-run. Verified by store `ids_are_stable_across_reopen` +
  `trust_persists_across_reopen`.
- **Scope decisions (surfaced two contradictions; user-approved both recommendations).**
  - **A5 (JSON Schema) + A10 (auto-detection) deferred.** The phase-02 file listed them (Tasks 3, 8 +
    acceptance) but the parity matrix (higher source of truth, §2) marks both `later`/non-gating. Per §2
    "the higher doc wins; fix the lower one" — fixed `plan/phases/phase-02-*.md` (annotated Tasks 3/8 +
    struck the two acceptance lines). A8/A12/A13 also remain `later`. No gold-plating into v1.
  - **Live `notify` watcher → Phase 6.** Phase 2 ships the deterministic sync engine + a Clock-driven
    `Debouncer` (tested on the mock clock) behind the `FileWatcher` port; the OS watcher lands with
    Phase 6's glob file-watch restart (D6) on the same `notify` infra. `KNOWN-DIVERGENCES.md` **D-2**.
- **Trust variant scope (Solo-behavior divergence, recorded).** Variant hash = command+working_dir+env
  (Phase 2 Task 5 / Solo's variant definition). Solo additionally re-trusts on auto_start/auto_restart/
  watch changes; we don't (those change *when/whether*, not *what* runs). `KNOWN-DIVERGENCES.md` **D-1**.
  Started `KNOWN-DIVERGENCES.md` (first real divergence; §7/§9).
- **YAML crate verified, not assumed.** `serde_yaml` is archived; checked via context7 (which surfaced
  the controversial `serde_yml` + newer `serde-saphyr`) and `cargo add --dry-run` for versions. Chose
  **`serde_norway` 0.9.42** (maintained `serde_yaml` fork, drop-in API, precise error locations for A4,
  `deny_unknown_fields` + IndexMap). Dropped `globset` from this phase (glob *matching* is Phase 6;
  only minimal empty-glob validation now) to protect the §6 size budget.
- **No frontend changes.** C1 is headless and not yet wired to the Tauri adapter; the TS `DomainEvent`
  mirror gains `ConfigChanged` in Phase 5 when the event is wired through `/impeccable` UI work — avoids
  speculative, hand-rolled frontend (§5/§15). `just lint && just test` green: **61 tests**.

### Phase 1 review fixes (2026-06-14)
Reviewed the Phase 1 PR (`82fa935`, `main...phase-1-walking-skeleton`) across all dimensions via
`REVIEW-PROMPT.md`. No blockers; gates re-verified green (`just lint`, `just test`). Applied the
review's one should-fix + the mechanical nits:
- **Snapshot-then-deltas ordering (should-fix).** `store/useProcesses.ts` now attaches the event
  listener *before* reading the snapshot (was racing them), so a delta emitted between the snapshot
  and the subscription can't be lost. Latent in Phase 1 (the demo only spawns on a user click) but
  would bite Phase 2 auto-start, which spawns at launch.
- **Honest test names (`crates/core/src/ids.rs`).** Replaced the overclaiming
  `distinct_id_types_do_not_share_a_counter_value_meaning` (which only checked `Display`) with
  `display_matches_the_raw_value` + a real `from_raw_round_trips_a_wire_value` (the IPC-decode path).
  Core tests 9 → 10.
- **Trimmed redundant dev-deps (`crates/pty/Cargo.toml`).** Dropped `soloist-core`/`nix` from
  `[dev-dependencies]` (already normal deps; integration tests see both) and set tokio dev features to
  what the test actually uses (`macros, rt, sync, time` — `sync` was previously only present via
  feature unification from `core`).
- **Documented the FSM bypass (`crates/core/src/supervisor.rs`).** Added a comment explaining why the
  panic-isolation path forces `Crashed` directly instead of through `ProcStatus::transition`.
- **Deferred (with reason), not applied:** (1) a `tracing::warn` on swallowed illegal transitions —
  doing it right means wiring the `tracing` span infra (an observability task, not a Phase-1 nit), and
  a bare half-measure conflicts with core's panic/dependency discipline; (2) replacing the
  `open_in_memory().expect()` launch fallback — every "graceful" alternative either masks a real
  storage failure (dangerous once trust persists in SQLite) or shows no usable app, so the loud fail
  on a can't-happen double-failure stays. Both recorded as open threads.

### Phase 1 build — walking skeleton (2026-06-14)
- **Built the hexagonal spine end to end.** `crates/core` (pure): newtype IDs, closed `ProcStatus`/
  `ProcessKind` enums + an explicit FSM (`ProcStatus::transition`), `DomainEvent` over a bounded
  `tokio::sync::broadcast` bus, the `Facade` (C8), and a supervised process **actor** that owns a child +
  cancellation token, drives the status FSM, and is wrapped in a panic-isolation boundary. Adapters:
  `crates/pty` `TokioProcessSpawner` (spawns into a fresh **process group**, signals the group via
  `nix::killpg`), `crates/store` `SqliteStore` (WAL + `user_version` migration + `meta` repo), and the
  `crates/app` Tauri command/event wiring. Core deps added: `tokio`, `tokio-util`, `async-trait`,
  `thiserror`, `serde` (all allowed — only `tauri`/`rmcp`/`axum`/`rusqlite`/`notify-rust` are forbidden;
  guard green).
- **Grace policy split (clean hexagonal seam):** the SIGTERM→grace→SIGKILL *timing* is a core policy
  driven by the `Clock` port (so it's testable on the mock clock with no real time), while the *signals*
  live in the adapter (`ProcessControl::terminate`/`kill`). This is why the actor needs the two-method
  control split now.
- **`Error`→`Crashed` (closed-enum reconciliation):** the phase file says a panicked unit is marked
  "Error", but the canonical `ProcStatus` (CLAUDE.md §3 / `plan/04` §4) has **no** `Error` variant. Per
  the source-of-truth hierarchy the closed enum wins, so a supervised panic surfaces as `Crashed`. No new
  enum variant invented.
- **`EventSink` via the broadcast bus:** all 7 ports from the phase scope are defined; the outbound event
  port `EventSink` is realized by `EventBus` (the `tokio::broadcast` model from `plan/04` §5).
  `FileWatcher`/`Notifier`/`Summarizer` are documented trait stubs (methods added in their phase — "add
  methods only when a phase needs them").
- **Playwright deferred to Phase 5 (doc contradiction surfaced):** the Phase 1 test plan lists a
  Playwright e2e smoke, but CLAUDE.md §14 + this ledger say Playwright starts Phase 5. Per §2 the higher
  docs win → deferred. The acceptance's substance (real `sleep` spawned, PID/process-group, stop → group
  gone) is instead proven by the `pty` **integration tests** at the facade level; only the literal in-GUI
  click is unautomated.
- **Library choices (docs-verified):** `rusqlite` **0.40** with the `bundled` feature (self-contained
  SQLite → AppImage-portable; **adds to binary size — measure in Phase 12**); `nix` **0.29** (`signal` +
  `process`) for `killpg`. Verified via context7; Tauri command/event/state APIs verified via the
  `tauri-*` skills + the official v2 docs (`Emitter` trait, `.manage()`/`State`, JS `listen`).
- **Codebase-discipline pass (user instruction — now CLAUDE.md §15):** added a strict discipline section
  (single source of truth, no magic strings/numbers, DRY, small files, real tests, reusable
  component-based frontend, no unnecessary code/comments). Acted on it immediately: **removed all 15
  Phase-0 `placeholder()` pretend-tests** across the crates; DRY'd the poison-safe lock into one
  `core::sync::lock`; named the demo spawn spec + simulated signal constants; restructured the UI into
  `domain.ts` (single type source) · `api.ts` (typed IPC) · `store/` (pure `applyEvent` reducer + hooks)
  · reusable `components/` (`Toolbar`, `ProcessList`, `StatusBadge`) with a thin `App.tsx`. Saved as a
  feedback memory.

### Phase 0 review + cleanup (2026-06-14)
- Reviewed the Phase 0 commit (`963e072`) across all dimensions; gates re-run green (`just lint`,
  `just test`) and the `.deb` rebuilt (2.3 MB, stripped). Applied the review's should-fix / nit items:
  - Removed a `plan/01` citation from `crates/app/Cargo.toml` (comment policy, CLAUDE.md §8).
  - Added a restrictive **CSP** + `freezePrototype: true` to `tauri.conf.json` (was unset → no policy).
  - Resolved the CLI-transport contradiction toward **HTTP client** (per `plan/04` §8/§10): fixed the
    `ipc` crate doc and the `ipc/` lines in `plan/01`/`plan/04` — `ipc` = app↔mcp UDS transport + shared
    message types; the CLI is a thin HTTP client of the loopback API.
  - Renamed core module `ports` → **`portscan`** (network port discovery); the hexagonal port *traits*
    keep the name "ports" to avoid the collision.
  - `vite.config.ts` target → `safari13` (dropped dead Windows branch); moved `shadcn` to
    `devDependencies` (lockfile regenerated; `--frozen-lockfile` passes); added deb-only `just deb`;
    hardened `check-core-deps.sh` to also catch sub-crates (`tauri-*`, `axum-core`).
  - De-staled `phase-00` task #8 + risk (22.04-only build; 20.04 = runtime-via-AppImage).
  - **Not changed:** the `dev.soloist.app` identifier (locked §9; its macOS `.app` build warning is
    harmless on Linux-only).
- **glibc / distribution finding (important):** the clean-container smoke (Ubuntu 22.04) showed a `.deb`
  **built on this host won't run on 22.04** — the host runs glibc **2.43** and the binary needs up to
  `GLIBC_2.39`, but 22.04 ships **2.35**. Rust binaries link the build host's glibc, so **distributable
  debs must be built on 22.04** (the CI `bundle` job already is). Added a CI **`smoke`** job (installs the
  artifact on 22.04, asserts `ldd` resolves, launches under Xvfb non-gating) + a CONTRIBUTING warning.

### Phase 0 build (2026-06-14)
- Stood up the **8-crate Cargo workspace** (`core/store/pty/app/mcp/httpapi/cli/ipc`): a pure `core`
  with the 14 bounded-context modules, a Tauri v2 desktop shell + Vite/React/TS UI, the `app_info()`
  Rust↔WebKit bridge, a `justfile` (dev/test/lint/bundle), the **dependency-direction guard**
  (`scripts/check-core-deps.sh`), GitHub Actions CI (`.github/workflows/ci.yml`, `ubuntu-22.04`), and a
  `.deb` bundle. All gates green; `CLAUDE.md` §14 filled with verified commands; `CONTRIBUTING.md` added.
- **Frontend stack change (user instruction):** adopted **shadcn/ui (Radix + Tailwind CSS v4)** for
  components; `plan/03` updated. React is **19** (resolver picked latest, not 18). Theme tokens are
  shadcn's OKLCH light/dark, OS-followed via a `prefers-color-scheme` class toggle. Visual design still
  goes through `/impeccable` (Phase 5); shadcn supplies primitives, not the visual identity.
- **Comment policy (user instruction):** source carries docblocks + genuinely important comments only —
  **no phase numbers, plan citations, or changelog notes in code.** Scaffolding cleaned to match.
- **Toolchain:** Rust 1.96 stable, pnpm 11.6, tauri-cli 2.11.2, just (all installed). `Cargo.lock` pins
  `brotli-decompressor` 5.0.0 + `alloc-stdlib` 0.2.2 to resolve a Tauri-transitive `alloc-no-stdlib`
  2↔3 conflict (upstream brotli 8.0.3 packaging bug). **Unpin when brotli fixes it.**
- **Build host = Ubuntu 22.04+** (Tauri v2 needs WebKitGTK 4.1; 20.04 ships only 4.0). 20.04 is a
  *runtime* target via the AppImage. This corrects the Phase 0 doc's assumption that 20.04 could build
  with 4.0. GitHub removed `ubuntu-20.04` hosted runners, so CI is 22.04-only.
- Fixed two build-tooling gotchas worth remembering: Vite 8 dropped bundled esbuild (use a boolean
  `minify`, not `"esbuild"`); TS 6 deprecates `baseUrl` (use `paths` alone); Tauri runs
  `beforeBuildCommand` from the frontend dir, so it is `pnpm build` (not `pnpm -C ui build`).
- Doc fixes: corrected stale "no git" lines in `SESSION-START-PROMPT.md` and `plan/03`.

### Planning session (2026-06-14)
- Propagated **coordination layer = v1** across matrix (G1–G11, E7), Phase 9, decisions, estimate, README.
  **Summarization off by default** confirmed.
- Added `CLAUDE.md` (operating manual) + this ledger; later extended `CLAUDE.md` with §4 (authoritative
  external sources), §5 (required skills), §6 (performance/size budget).
- Mandated all UI/UX through the project-local **`/impeccable`** skill; ran `/impeccable init` → wrote
  `PRODUCT.md`. `DESIGN.md` deferred by the user.
- Confirmed the project-local `tauri-*` skill suite is the Tauri authority (backed by official docs).
- **Git initialized** + private GitHub remote **`ArtMin96/soloist`** created and pushed.
- Added `SESSION-START-PROMPT.md`.

---

## Open threads / unresolved

- **Phase-5 runtime echo/control gate — CLOSED by a real human click (2026-06-19), R2 unblocked.** The user
  ran `just dev` (host `DISPLAY=:0`), selected the `shell` process in the sidebar, clicked its **per-row Start**,
  typed `echo hi` → it **started and echoed**. So the control wiring, the core start path, and the one untested
  boundary (`Channel<Vec<u8>>`→`Uint8Array`→rAF coalescing in `useTerminal.ts`) all work end to end. The earlier
  failure to activate controls was the **synthetic-XTEST/XWayland quirk** (a test-harness artifact), not a real
  control bug. **R2 is no longer blocked.**
- **"Start all" (toolbar bulk) does nothing on the demo — expected behavior + a real parity gap (NOT an R1
  regression).** Traced: toolbar → `store.startAll` → `stack_start` → `Supervisor::start_all` (`supervisor.rs:248`),
  which launches only **trusted `auto_start` candidates** (`registry.auto_start_candidates`; asserted by
  `start_all_starts_only_trusted_auto_start_commands`, `supervisor.rs:770`). The demo commands have
  `auto_start=false`, so the candidate set is empty → it correctly starts nothing (per-row Start works because it
  bypasses the auto_start filter). **The gap:** Solo separates **`start-all`** (all trusted commands) from
  **`start-auto`** (auto_start only) — distinct HTTP endpoints (`05` §8) and `start_all_commands` = "trusted
  commands only" with no auto_start filter (`05` §7). We implemented only the *auto* semantics but the button is
  labeled "Start all". Fix belongs to the **Phase-5 follow-up / matrix B4 bulk ops** (decide the start-all vs
  start-auto split; "Start all" should start all trusted commands, or relabel to "Start auto"); deferred, not
  done. Non-blocking for the cleanup R-phases.
- **Stray `package-lock.json` at repo root (untracked) — user decision: LEAVE IT (2026-06-19).** Project uses
  pnpm; asked, user chose to leave it in place. Stays flagged; not gitignored, not removed.
- **Cleanup roadmap status: COMPLETE (R0–R6 all done, 2026-06-19).** **R0** (`ea4bad1`) + **R1** (`4c80eb7`)
  + **R2** (`c04859a`) + **R3** (`71eafac`) + **R4** (`65cf819`) + **R5** (`3f07350`: split `core/testing.rs`
  547 → `testing/` per-concern submodules, file-size guard zero outliers; honest-test audit found **zero
  deletions**) + **R6** (`2dce185` direct `store::migrate` forward-migration test + the docs-convergence
  commit). Each R-phase stopped for review before the next per the agreed sequence. **R6 = converge docs &
  ledger** (`plan/06` §7): fixed `plan/03` `serde_yaml`→`serde_norway`, the post-refactor structural claims
  in `plan/04`/`plan/06`/`ARCHITECTURE.md` (`ports/`, `supervisor/`, `core::testing/`, the live file-size
  guard), added roadmap completion banners, and folded in the R5 coverage note as a direct migrate test
  (count **106 → 107**). `KNOWN-DIVERGENCES.md` reviewed — no new entry (the serde swap is an internal dep
  choice, not Solo behavior). **The cleanup track is finished; next is real feature work** (do not start it
  without the user confirming the cleanup is signed off).
- **Plan review:** user may still skim `plan/05` (Solo behavior), `plan/04` (architecture), `plan/02`
  (parity) and confirm before deep feature work — not blocking Phase 1.
- **Agent native OAuth/login (E8) → Phase 7, no new work beyond launching right.** When Phase 7 lands,
  launch the agent interactively (no `-p`) with `$DISPLAY`/`BROWSER`/`ANTHROPIC_*` passed through;
  manage no agent credentials. A quick manual proof is possible **now** without Phase 7: register a
  Command running `claude`, open its terminal, complete the login — validates the substrate. Recorded in
  `05` §6, matrix E8, phase-07.
- **Codebase-discipline gate now enforced (CLAUDE.md §7.6, plan/04 §10/§15).** Two optional follow-ups,
  flagged for a decision (not done — would touch verified-pending code): (1) **split `supervisor.rs`**
  (491 code lines; pull bulk-ops + `reconcile_orphans` into `supervisor/` submodules); (2) add
  `scripts/check-file-size.sh` to `just lint`/CI (warn on non-test source files over ~400 lines), the
  way `check-core-deps.sh` guards layering. Everything else already meets the bar.
- **New `later` parity rows added this session (tracked, non-gating):** `D12` descendant subprocess
  stats (Phase 6); `F14` prompt-template MCP tools (Phase 8); `I12` Activity Monitor view, `I13` prompt
  templates UI, `I14` nested child-agent display (Phase 11; I14 also Phase 5). Build when their consuming
  phase needs them — do **not** pull into v1.
- **`DESIGN.md` — DONE (Phase 5).** Seeded via `/impeccable document` + user-approved ("The Instrument
  Panel": cool-slate neutral + one azure accent; saturated color spent only on status, encoded as
  shape+color+label). `index.css` implements its OKLCH tokens. **Still open:** generate the
  `.impeccable/design.json` sidecar (deferred until the components stabilise) so the impeccable live panel
  renders the real primitives; and a **status-hue contrast audit** in both themes (impeccable AA — chosen
  to clear the thresholds but **not yet browser-verified**).
- **`KNOWN-DIVERGENCES.md`** created this session (Phase 2): **D-1** trust variant = command+dir+env
  (narrower than Solo's sync re-trust set), **D-2** live `solo.yml` watcher deferred to Phase 6. Phase 13
  parity walk reads this file.
- **Phase 2 deferred `later` rows (tracked, non-gating):** A5 JSON Schema (`schemars` → `solo.schema.json`),
  A8 "automatically trust command changes" setting, A10 command auto-detection, A12 local-vs-shared
  (`Visibility`) commands, A13 project icon rendering. Build when their consuming phase needs them.
- **A2/A6 — CLOSED in Phase 3.** A6 (untrusted cannot run by any path) is enforced in core on
  start/restart/start_all (`an_untrusted_command_cannot_run_by_any_path`); A2 (fields honored at
  runtime) is verified on a real shell via exit code. Phase 13's parity walk re-confirms.
- **Config→supervisor wiring — STILL DEFERRED (now the Phase-5 follow-up).** `Facade` owns
  `Projects`/`TrustStore`/`ConfigEngine` + the `Supervisor` over one shared `TrustRepo`/bus, but **nothing
  yet connects** `ConfigEngine` (a loaded `solo.yml`) → `Supervisor::register` of its commands. Phase 5
  used a temporary `crates/app/src/demo.rs` seed instead (one Agent + one Terminal + two pre-trusted
  Commands). The follow-up wires "open project → register commands → `reconcile_orphans` → `start_all`"
  (reconcile **after** registration) and **replaces the demo seed**. Phase 5 already surfaces `ProcessView`
  (with `project`/`exit_code`) in the UI + TS mirror; the deferred UI for this thread is the **trust/sync
  dialog** (A6/A9, on `ConfigChanged`) and the **orphan dialog** (B8 UI, on `OrphansFound`) — commands
  `project_load`/`project_switch`/`config_trust`/`orphans_resolve` are not built yet.
- **B8 orphan adoption — DONE this session** (Phase 3 → `Done — pending verify`). The mechanism (record/
  reconcile/adopt/surface/prune) + real adapters (`FileRuntimeState`, `PgidOrphanControl`) are complete and
  tested. **One deferred wiring:** the app must **call `Supervisor::reconcile_orphans()` on launch *after*
  registering config commands** (so a leftover that matches a `solo.yml` command is adopted, not
  mis-surfaced). That belongs in the Phase-5 "open project → register commands → reconcile → start_all"
  sequence; recording is already live. B7's **"clears crash tracking"** half remains a Phase-6 item.
- **Phase 4 frontend follow-ups — DONE (Phase 5), with one divergence.** The **xterm.js terminal pane**
  + `pty_write`/`pty_resize` + the `attach_pty` bridge all landed. **Divergence from the phase-04/`plan/01`
  sketch:** raw bytes ride a single **`tauri::ipc::Channel<Vec<u8>>`** opened by `pty_attach` (high-
  throughput, single-consumer; the scrollback replay is its first message), **not** a per-process
  `pty:<id>` *event* channel — events are explicitly not for high throughput (`tauri-calling-frontend`).
  `domain.ts` now mirrors `RenderedScreen`/`LogLine` + the `TerminalTitleChanged`/`TerminalBell`/
  `OrphansFound`/`ConfigChanged` variants. **Still unverified (manual):** live terminal echo / "TUI renders
  & accepts input" — pending the user's GUI run (the `freezePrototype` fix unblocked the blank window).
  **Refinement noted:** `useTerminal` re-creates the xterm on a resting↔active status flip (correct —
  scrollback is replayed from the core — but mildly janky); make it re-attach without re-creating.
- **PTY footprint (revisit Phase 13 soak):** `portable-pty`'s blocking reader/wait means **2 persistent OS
  threads per *running* process** (drain + reap). Input writes/resizes are no longer inline-blocking — they
  run on the **shared `spawn_blocking` pool** (transient, not per-process), as do runtime-state file writes,
  so neither stalls the tokio runtime (review fix). For many processes still consider moving reads to
  `tokio::AsyncFd` + `try_wait` polling to drop the two persistent threads. Measure in the §6/Phase-13
  footprint pass before optimizing.
- **Live `FileWatcher` adapter (Phase 6).** The port is still a methods-less stub; Phase 6 adds its
  methods + a `notify`-backed adapter that drives `ConfigEngine::sync` through the `Debouncer`, and also
  serves glob file-watch restart (D6). Pick the watcher-adapter crate home then (new `crates/watch` vs
  fold into an adapters crate). **`ConfigEngine::sync` is documented single-writer + blocking** (Phase 2
  review): drive it from **one debounced task per project** and invoke it off-thread (`spawn_blocking`)
  so it neither races the snapshot/double-publishes `ConfigChanged` nor stalls the `tokio` runtime.
- **Clean-container `.deb` smoke** now run (docker) and added as a CI `smoke` job. It found the glibc
  floor (above): **build distributable debs on Ubuntu 22.04**, not a newer host. Remaining: the CI
  `smoke` job's Xvfb GUI launch is **non-gating** (headless flakiness) — make it gating once a 22.04-built
  artifact is observed launching a window; and the container *GUI launch* on a 22.04-built deb is still
  unconfirmed (only install + `ldd` were proven; the host-built deb can't be used for it due to glibc).
- **Placeholder app icon** (`crates/app/app-icon.png` → generated `crates/app/icons/`): a simple "S"
  glyph; replace with real art in Phase 11/12.
- **Phase 1 GUI click-through unautomated:** the Start/Stop button thread is wired and the Rust path is
  proven by the `pty` facade integration test, but the in-webview click is not yet automated (Playwright
  is a Phase 5 deliverable). Confirm manually via `just dev`, then mark Phase 1 `Verified`.
- **Illegal-transition observability (deferred from Phase 1 review):** `supervisor::transition` silently
  drops an illegal FSM edge (current state retained). Add a `tracing::warn` once the `tracing` span infra
  is wired (logging keyed by `ProcessId`/`ProjectId`, per `plan/04` §10) — not before, to avoid a
  half-measure that conflicts with core's panic/dependency discipline.
- **Store init failure handling (deferred from Phase 1 review):** `app::build_facade` degrades
  durable→in-memory, then `expect()`s if even in-memory fails (a can't-happen double-failure). Revisit
  when durable state becomes load-bearing (trust in Phase 2): a silent no-op store would mask a real
  storage failure, so any change must fail loudly or surface a dialog rather than swallow it.
- **TS↔Rust type mirror (single-source risk):** the TS domain types in `crates/app/ui/src/domain.ts` are
  hand-mirrored from the core enums. They live in one place per side, but drift is possible. Consider
  generating them from Rust (e.g. `ts-rs`) when the surface grows — flag for the user (size/build
  trade-off) before adding the dep.

---

## Next session should start with

0. **Cleanup track — COMPLETE (R0–R6 all done, 2026-06-19).** Commits: `ea4bad1` (R0 file-size guard) ·
   `4c80eb7` (R1 reusable `core::testing` behind a `testing` feature) · `c04859a` (R2 split `supervisor.rs`)
   · `71eafac` (R3 `CorePorts` param object + single composition root; both `too_many_arguments` allows gone;
   `ports.rs` → `ports/{mod,bundle}.rs`) · `65cf819` (R4 purged the demo seam from the pure core) · `3f07350`
   (R5 split `core/testing.rs` 547 → `testing/{mod,clock,spawner,lock_releaser,runtime_state,repos,fixtures}.rs`;
   file-size guard **zero outliers**; honest-test audit, **zero deletions**) · `2dce185` + the docs-convergence
   commit (R6: direct `store::migrate` forward-migration test, count **106 → 107**; reconciled `plan/03`
   `serde_yaml`→`serde_norway`, the post-refactor structural claims, roadmap completion banners).
   `just lint && just test` green: **107** (Rust **97** / UI **10**); file-size guard zero outliers.
   **DO NOT start new feature work without the user confirming the cleanup is signed off** (the agreed gate
   after the last R-phase). The locked decisions hold: tests stay **inline**; the 7 empty placeholder modules
   and the 4 stub adapter crates **stay**; the stray root `package-lock.json` is **left** (do not rm/gitignore/
   stage). Once signed off, begin real feature work — the deferred Phase-5 follow-up (items 2–3 below).
1. **Runtime echo/control gate — CLOSED (2026-06-19).** A real human click on per-row **Start** for `shell`
   started it and `echo hi` echoed in the xterm — control wiring + core start path + the
   `Channel<Vec<u8>>`→`Uint8Array`→rAF boundary in `useTerminal.ts` all work. No longer blocks R2. **One
   Phase-5 follow-up finding to fold into B4 bulk ops:** the toolbar **"Start all"** does nothing because
   `Supervisor::start_all` only launches `auto_start` candidates (demo commands are `auto_start=false`) — Solo
   separates `start-all` (all trusted commands) from `start-auto` (auto_start only); implement the split or
   relabel the button. Deferred, non-blocking (see open threads). Still pending verify (cosmetic, non-blocking):
   status-hue **contrast** AA in both themes.
   - **Playwright e2e (Task 6, still pending):** assert grouping, `[data-status]`, control enable/disable,
     selection, empty state via **`@tauri-apps/api/mocks` `mockIPC`** (installed) with a fixture stack; full
     PTY echo needs `tauri-driver` + `WebKitWebDriver`. Drive via the `webapp-testing` skill.
2. **Then build the deferred Phase-5 follow-up** (still `/impeccable`-driven): **trust dialog** (A6/A9 —
   on `ConfigChanged{requires_trust}`, show command+working_dir+env+diff → `config_trust`; Start disabled
   for untrusted), **orphan dialog** (B8 UI — on `OrphansFound`, Kill/KillAll/Leave → `orphans_resolve`),
   and **project load/switch**. Add the matching commands (`project_load`/`project_switch`/`config_trust`/
   `orphans_resolve`) + forward `TerminalTitleChanged`/`TerminalBell` to the terminal header.
3. **Wire "open project → register config commands → `reconcile_orphans()` → `start_all`"** (connect
   `ConfigEngine` to `Supervisor::register` per command), replacing the temporary `demo.rs` seed. The
   **reconcile call must come *after* registration** so a leftover matching a `solo.yml` command is
   adopted, not mis-surfaced (B8's one deferred wiring; mechanism + recording already done/tested).
4. **Smaller follow-ups recorded below:** generate the `.impeccable/design.json` sidecar once components
   stabilise; consider lazy-loading xterm to trim the 167 KB-gzip bundle (§6, measure in Phase 12);
   refine `useTerminal` so a resting↔active status flip doesn't re-create the xterm (it currently
   re-attaches/replays — correct but mildly janky).
5. **Still open from Phase 1 (independent, user-only):** confirm the in-GUI Start/Stop click via `just
   dev`, then flip the Phase 1 row to `Verified`.
6. **Do not pull deferred `later` rows into v1** (A5/A8/A10/A12/A13, B9, C8 webgl) and **do not** build
   the live `notify` watcher before Phase 6 — all recorded above with rationale.
