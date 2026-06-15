# PROGRESS.md — Soloist State Ledger

> **This file is the shared memory across sessions.** Git history complements it, but this ledger is
> where a session reads what's done and what's next. **Read it at the start of every session** (per
> `CLAUDE.md` §1) and **update it at the end of every session** (per `CLAUDE.md` §10–§11). Keep it
> factual and evidence-backed — never mark `Verified` what you didn't verify.

---

## Current state

- **Overall:** **Phase 4 (PTY & Terminal I/O, C3) — `Done — pending verify`.** Real pseudo-terminals
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
  `just lint && just test` green: **98 tests** (core 71 / pty 9 / store 12 / UI 6). All v1 rows **C1–C7,
  C9** verified headless on a real PTY (`test -t 1`, `read x`, `tput cols`, OSC title/bell, raw-vs-rendered,
  attach replay); **B8** verified via core reconcile/adopt tests + real-adapter tests.
- **Active phase:** **Phase 4 (PTY & Terminal I/O)** — `Done — pending verify`. The pending piece is the
  **xterm.js terminal pane** (parity **C8** `later` + phase-04 Task 9), deferred to Phase 5 via
  `/impeccable` (matches the Phase 2/3 frontend-deferral rhythm). **Phase 3 is now also `Done — pending
  verify`** — B8 (orphan adoption) landed this session, so B1–B8 are complete. Next up is **Phase 5
  (Dashboard UI)**.
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
- **Last session:** 2026-06-15 — built Phase 4 / context C3 (C1–C7, C9 + folded Task 4); details under
  "Decisions / changes".

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
| 4 | PTY & terminal I/O (rendered+raw, input, resize, OSC) | **Done — pending verify** | **C1–C7, C9 v1 delivered (C3 context).** Real PTY per process via `portable-pty` (`$SHELL -lc` on the slave; child sees a tty); `pty` adapter rewritten (`PtyProcessSpawner`) keeping pgroup reaping. Core `terminal/` (`ring`/`buffers`/`parser`): bounded raw scrollback (256 KB, **C5**) + `vte`-driven rendered `Ring<LogLine>` (5,000 lines, **C4** + folded Task 4) with `\r` overwrite/tab stops; OSC **title**+**bell** → `DomainEvent`s (**C7**); live raw bytes via per-process broadcast. `Supervisor`: `write_stdin`/`resize` (**C3**/**C6**), `attach_pty` (atomic replay+live, **C9**), `pty_scrollback`/`rendered`. **Evidence:** core 66 (+10: 5 buffers, 2 ring, 3 supervisor PTY), pty 8 real-OS (`test -t 1`→tty **C1**, `read x`→input echo **C3**, `tput cols`→resize **C6**, + the Phase-3 pgroup/reap suite green). `just lint && just test` green (**90**). **Pending verify:** xterm.js terminal pane (**C8** `later` + phase-04 Task 9) → Phase 5 via `/impeccable`; "vim/htop visually render" is the Phase-5/manual check. |
| 5 | Dashboard UI (sidebar tree, status dots, terminal pane, trust dialog) | Not started | Playwright e2e starts here; drive UI through `/impeccable`; seed `DESIGN.md` first |
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

- **Plan review:** user may still skim `plan/05` (Solo behavior), `plan/04` (architecture), `plan/02`
  (parity) and confirm before deep feature work — not blocking Phase 1.
- **`DESIGN.md` pending** (color/type/components/motion). shadcn's OKLCH tokens are the current base;
  seed `DESIGN.md` via `/impeccable document` before the first real UI work (Phase 5). When seeding,
  `WebFetch` soloterm.com as a clean-room *reference* (feel, not assets); confirm color-blind-safe
  status encoding.
- **`KNOWN-DIVERGENCES.md`** created this session (Phase 2): **D-1** trust variant = command+dir+env
  (narrower than Solo's sync re-trust set), **D-2** live `solo.yml` watcher deferred to Phase 6. Phase 13
  parity walk reads this file.
- **Phase 2 deferred `later` rows (tracked, non-gating):** A5 JSON Schema (`schemars` → `solo.schema.json`),
  A8 "automatically trust command changes" setting, A10 command auto-detection, A12 local-vs-shared
  (`Visibility`) commands, A13 project icon rendering. Build when their consuming phase needs them.
- **A2/A6 — CLOSED in Phase 3.** A6 (untrusted cannot run by any path) is enforced in core on
  start/restart/start_all (`an_untrusted_command_cannot_run_by_any_path`); A2 (fields honored at
  runtime) is verified on a real shell via exit code. Phase 13's parity walk re-confirms.
- **C1 now wired to the façade.** `Facade` owns `Projects`/`TrustStore`/`ConfigEngine` + the
  `Supervisor` over one shared `TrustRepo`/bus. **Still to wire:** `ConfigEngine::open`→`Supervisor::
  register` of a project's commands (loading a `solo.yml` into managed processes) — Phase 3's B8 session
  or Phase 5 will connect "open project → register its commands → start_all". Phase 5 surfaces
  `ConfigChanged`/`ProcessView` (now carrying `project`/`exit_code`) in the UI + the TS mirror via `/impeccable`.
- **B8 orphan adoption — DONE this session** (Phase 3 → `Done — pending verify`). The mechanism (record/
  reconcile/adopt/surface/prune) + real adapters (`FileRuntimeState`, `PgidOrphanControl`) are complete and
  tested. **One deferred wiring:** the app must **call `Supervisor::reconcile_orphans()` on launch *after*
  registering config commands** (so a leftover that matches a `solo.yml` command is adopted, not
  mis-surfaced). That belongs in the Phase-5 "open project → register commands → reconcile → start_all"
  sequence; recording is already live. B7's **"clears crash tracking"** half remains a Phase-6 item.
- **Phase 4 follow-ups (Phase 5 frontend):** the **xterm.js terminal pane** (C8 `later`, phase-04 Task 9)
  + the Tauri `pty_write`/`pty_resize` commands + a per-process `pty:<id>` event channel bridging the core
  `attach_pty` broadcast to the webview — all through `/impeccable` + `tauri-calling-rust`/
  `tauri-calling-frontend`. The **TS domain mirror** gains `PtyChunk`/`RenderedScreen`/`LogLine` + the
  `TerminalTitleChanged`/`TerminalBell` `DomainEvent` variants (hand-mirrored in `domain.ts`, as in
  Phase 2/3). "vim/htop visually render & accept input" is the Phase-5/manual acceptance check for C1/C2.
- **PTY footprint (revisit Phase 13 soak):** `portable-pty`'s blocking reader/writer/wait means 2 OS
  threads per *running* process (drain + reap). Correct and bounded, but for many processes consider
  moving reads to `tokio::AsyncFd` + `try_wait` polling to drop the threads. Measure in the §6/Phase-13
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

1. Run `just lint && just test` first to confirm the baseline is still green (**98 tests**: core 71 /
   pty 9 / store 12 / UI 6). Phases 1–4 are all `Done — pending verify`; **Phase 5 is next.**
2. **Phase 5 — Dashboard UI** (consumes the Phase 2/3/4 read models). First **seed `DESIGN.md`** via
   `/impeccable document` (WebFetch soloterm.com as a clean-room *feel* reference; confirm color-blind-safe
   status encoding) — a hard prerequisite (CLAUDE.md §5). Then drive the dashboard through `/impeccable` +
   `webapp-testing` (Playwright starts here): sidebar tree (Agents/Terminals/Commands, I1), status dots,
   trust dialog, the **xterm.js terminal pane** wired to the core's `attach_pty` (replay + live) via a
   Tauri `pty:<id>` channel + `pty_write`/`pty_resize` commands (`tauri-calling-*`), and the
   **OrphansFound** Kill/KillAll/Leave dialog. Add the TS mirror (`PtyChunk`/`RenderedScreen`/`LogLine` +
   the `TerminalTitleChanged`/`TerminalBell`/`OrphansFound` events).
3. **Wire "open project → register config commands → `reconcile_orphans()` → `start_all`"** (connect
   `ConfigEngine` to `Supervisor::register` per command). The **reconcile call must come *after*
   registration** so a leftover that matches a `solo.yml` command is adopted, not mis-surfaced (B8's one
   deferred wiring; the mechanism + recording are already done and tested).
4. **Still open from Phase 1 (independent, user-only):** confirm the in-GUI Start/Stop click via `just
   dev` (the demo terminal `sleep 60` now runs on a real PTY: `stopped → starting → running`, then
   `stopping → stopped`), then flip the Phase 1 row to `Verified`. Headless automation waits for Phase 5
   Playwright.
5. **Do not pull deferred `later` rows into v1** (A5/A8/A10/A12/A13, B9 "resume last session", C8 webgl)
   and **do not** build the live `notify` watcher before Phase 6 — all recorded above with rationale.
