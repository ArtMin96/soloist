# PROGRESS.md — Soloist State Ledger

> **This file is the shared memory across sessions.** Git history complements it, but this ledger is
> where a session reads what's done and what's next. **Read it at the start of every session** (per
> `CLAUDE.md` §1) and **update it at the end of every session** (per `CLAUDE.md` §10–§11). Keep it
> factual and evidence-backed — never mark `Verified` what you didn't verify.

---

## Current state

- **Overall:** **Phase 7 (Agents & idle detection) — `Verified` (all v1 rows E1–E5, E8; user-confirmed at
  runtime 2026-06-22). Phase 8 (MCP server core) is the active phase: session 1 — the MCP walking skeleton
  (rmcp stdio → IPC/UDS → app → `Facade`; identity/scope + 7 read/identity tools; F1/F3/F4 + read F5/F6) —
  landed on `feat/phase-8-mcp-skeleton` and was then **independently reviewed and review-fixed (2026-06-23)**:
  a latent `IpcResponse` serialization bug fixed (`list_processes`/`list_projects` now serialize over the
  wire), data-dir creation single-sourced + the socket **0700**-hardened, the IPC request bounded by a
  timeout, the `select_project`/`register_agent` tools completed, and the per-id read DRY'd; gate re-green.
  See the top Decisions entry + "Next session" item A for the tool fan-out.** _Prior detail:_ **Phase 7 (Agents & idle detection) — all v1 rows (E1–E5, E8) code-complete; `Done — pending
  verify` (runtime verify via `just dev` owed). Phases 5 & 6 also remain `Done — pending verify` (runtime
  checks are user-only).** Newest (2026-06-22): **E5 — the 5-state idle FSM — landed** on branch
  `feat/phase-7-idle-detection` (see the top Decisions entry + the "Active phase" line below). _The long
  historical narrative that follows is prior-session detail, kept for continuity._ Older newest (2026-06-20):
  **the D6/D7 file-watch restart — CORE POLICY ONLY** (the real
  `notify` OS adapter is the next session). New **C5 `core/filewatch/` domain** mirroring `core/metrics`/
  `core/portscan`: owns its own `FileWatcher` port (moved out of the `ports/mod.rs` stub) with a `Noop`
  default; a pure `policy.rs` (globset matching relative to root, `*` crosses separators, **D7 default
  ignores** `.git`/`node_modules`/`target`/`dist`/`.venv`); a `Clock`-driven `WatchReactor` that **reuses
  `core/debounce::Debouncer`** to coalesce a save burst and routes to the new `Supervisor::file_restart`
  (which delegates to the **existing `Supervisor::restart`** — one restart behaviour, trust gate +
  crash-tracking reset reused). New `DomainEvent::FileRestart` mirrored in `domain.ts`/`projection.ts`.
  `globset` added to core (pure; dep-direction guard still green). Wired into `CorePorts` (Noop default) +
  `Facade::file_watch_loop()` spawned in the composition root (inert under Noop until the adapter lands).
  Gate **225 = Rust 183 / UI 42** (+12 Rust filewatch tests; mock-clock, deterministic; reviewed +
  running-only fix applied — see the top Decisions entry). Branch
  **`feat/phase-6-file-watch`** (new PR), off `main` (PR #8 merged). The prior **OS-probe slice (D1/D2/D3)**
  merged as **PR #8**; the **crash auto-restart policy** (D4 + D11) as PR #7. **D6/D7 are now LIVE** (notify
  OS adapter + dynamic re-watch, `79de1cc`, PR #9) and **D8 native notifications are DONE** (C7 `notify` domain
  + Tauri notification plugin, on the stacked branch **`feat/phase-6-notifications`** → stacked PR based on
  `feat/phase-6-file-watch`). Gate **234 (Rust 192 / UI 42)**. **Newest (2026-06-20, branch
  `feat/phase-6-soak` off `main` — PRs #9/#10 merged):** the **nightly soak gate**, the **Phase-6 UI
  surfacing**, and a **metrics-accuracy fix** all landed (3 commits → one PR). (1) Soak gate
  (`crates/pty/tests/soak.rs`, `#[ignore]`d): start/stop loop of 40 real processes → flat fd/OS-thread/tokio-task
  counts + zero leaked groups; crash→auto-restart storm → exactly the 10/60s gate, no zombies, flat RSS; metrics
  sampler self-restarts after a panicking sample. New `.github/workflows/soak.yml` (schedule nightly +
  workflow_dispatch, `--test-threads=1`) + `just soak`. (2) UI surfacing: running rows show `:port  cpu% rss` in
  muted mono at rest, swapping to controls on hover (selected → terminal header); `restarting k/N`, `not ready`,
  and `Exhausted` (status glyph) badges. Event-derived via a coalesced `SignalsProvider`/`useSignal` context
  (`MetricsTick`/`RestartScheduled`), off the read-model list. (3) **Metrics fix** (user-reported 550% CPU / 9 GB
  RSS): the `sysinfo` probe summed per-process RSS across a subtree (double-counts shared memory) and used the
  per-core CPU convention. Rewrote it over `/proc` with exact process-group membership: memory = summed **PSS**
  (`smaps_rollup`), CPU = whole-machine (100% = all cores, never above) with per-pid baselines. Dropped `sysinfo`
  entirely (added `libc` for `sysconf`). Measured: a 3-core busy group now reads **37% / 6.8 MB** (was ~300% /
  inflated). Gate **green: Rust (core 160, sys 5+2+4+3, pty 9 + soak 3 ignored, store 13) / UI 60**. **Newest
  (2026-06-21, branch `feat/phase-6-restart-banner` off `main`, PR #11 merged): D5 restart banner DONE — the
  last Phase-6 v1 build.** On a relaunch the process's terminal scrollback is **retained** and a muted
  `── restarted ──` banner is drawn **before** the new run's output. Root fix: the crash auto-restart path
  spawns a *fresh actor* whose `Terminals::open` previously replaced the channel with empty buffers + a new
  live sender — wiping the last crash output **and freezing** the attached pane (still subscribed to the
  dropped sender). `open` now **reuses** an existing process's buffers + live sender on a relaunch (fresh input
  channel only); new `Recorder::mark_restart` writes the banner **iff** there's prior output, called once at the
  actor's spawn-loop top so **one rule** covers every relaunch trigger (crash/file/manual/user-start) — no
  frontend or Tauri-adapter change (the `pty_attach` forwarder keeps draining the reused live sender straight
  to the webview Channel). Banner = dim ANSI in the raw stream, plain text in the rendered projection (MCP/logs).
  Gate **green: Rust core 163 (+3) / sys 14 / pty 10 (+soak 3 ignored) / store 13 / UI 60**. **Next session
  should start with: runtime verification of the full Phase-6 acceptance walk via `just dev`, then flip Phase 6
  → Verified** (kill -9 → auto-restart + **banner before new output** + toast; busy process → sane CPU/RSS; dev
  server → port + readiness; edit watched file → restart). Deferred (with reason): the discrete **file-restart**
  row cue (Task 9 lists only CPU/RSS/restarting/not-ready/exhausted; the status already cycles through
  Restarting) and D9/D10 in-app toasts + attention bell (`later`). Also open: **R7** (driven-port ownership
  drift, `plan/06` §7). See the top Decisions entry.
  Prior 2026-06-20 work: **projects became
  a first-class feature** — a **project-grouped sidebar** (each opened project a collapsible node: icon +
  name + running count + per-project bulk controls, over its non-empty kind subgroups), a single-sourced
  **project read-model** (`ProjectView`/`ProjectOpened`, durable in SQLite; `load_project` now persists the
  `solo.yml name:` it previously dropped), **A13 project icons pulled into v1** (capped `project_icon` data
  URL + monogram fallback), and **session restore on launch** (durable projects re-register *resting*, so the
  sidebar isn't empty across runs), then **consolidated into a single Projects domain/module** (backend
  `core/projects/` + a `ProjectService` lifecycle; frontend `store/projects/`; the icon now arrives inside
  the project read-model (resolved like the name) instead of a separate `project_icon` call — see the top
  Decisions entry). Gate **186 (Rust 146 / UI 40)**. Commits moved to a dedicated branch (see the top
  Decisions entry). _Runtime verification is the user's (restart `just dev`)._ A prior fourth
  2026-06-19 session **built A10 (command auto-detection) — now v1, code-complete** (opening a folder with no `solo.yml`
  auto-creates one from detected commands, with a friendly confirmation), **finished the deferred
  adversarial review** of the Phase-5 follow-up (applied 2 fixes; recorded the rest), and added a **full
  `solo.yml` reference** to `README.md`. Gate **green: 174 — Rust 138 / UI 36**. See the top "fourth
  session" entry under Decisions. A prior third session fixed the silent empty-project-load (`72b526e`)
  and the user **runtime-confirmed** project-load via the picker. The rest of this block describes
  the prior interactive core slice.** The visible
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
- **Active phase:** **Phase 7 (Agents & Idle Detection, C4)** — `In progress` (started 2026-06-22 per the
  user's directive). **E1/E2/E3 code-complete** (agent-tool registry + `--version` autodetect, on `main`
  via PR #13). **E4 backend + E8 code-complete (2026-06-22, branch `feat/phase-7-agent-launch`, `a7235c6`):**
  `Facade::launch_agent` runs a configured tool as an interactive-PTY Agent process in the project dir with
  the env passed through (no credential injected); thin `agent_list`/`agent_detect`/`agent_launch` Tauri
  commands + the `AgentTool`/`AgentKind`/`DetectedTool` TS mirror. **E4 launch picker UI — done (`2eb3f75`):**
  a `Cmd/Ctrl+T` shadcn `Command` (cmdk) palette with progressive "agent with flags" (Alt+Enter) + active-project
  targeting, user-signed-off visuals. **So E4 + E8 are complete** (code; runtime verify still owed). **E5 — the
  5-state idle FSM — is now code-complete (2026-06-22, branch `feat/phase-7-idle-detection` off `main`):** a new
  C4 `core/agents/idle/` subdomain (AgentActivity enum; per-provider Strategy — output-delta for
  Claude/OpenCode, OSC-title stability for Codex/Amp, OSC-title status for Gemini, output-delta default for the
  rest; conservative permission-cue detector; edge-triggered classifier; ProcessId→AgentKind tracker keeping
  AgentKind out of C2; Clock-driven self-supervised sampler mirroring `MetricsSampler`), C3 exposing one
  `TerminalActivity` snapshot (output counter, latest title, rendered tail), `AgentActivityChanged` emitted on
  transitions, C7 toasting on Permission/Error, wired through the facade (track at launch + `idle_sampler_loop`)
  and the composition root. Frontend: `AgentActivity` mirror, an event-derived activity signal (off the
  read-model list), and a consolidated `ProcessIndicator` (activity-for-running-agent vs ProcStatus) extending
  the existing glyph+color+label vocabulary — shaped via `/impeccable`, label on the shadcn `Tooltip`, one new
  `--status-attention` token, user-signed-off vocabulary. **So all of Phase 7's v1 rows (E1–E5, E8) are
  code-complete**; **E6** (summarization) `later` + OFF by default, **E7** in P9. Reviewed, then **merged to
  `main`** via **PR #15** (`b95dc6a`; review-fixes `8763948` included; branch deleted). Gate **green: Rust core
  202 / store 15 / sys 5 (+10 integration) / pty 11 (+3 ignored) / UI 77**; `just lint` + `just test` clean.
  Runtime verify (idle FSM tracking a real agent via `just dev`) is owed (user-only). See
  the top Decisions entry + "Next session should start with" item A.
- **Phase 6 (Monitoring, Auto-Restart & Notifications)** — `Done — pending verify` (carried, **not** yet
  `Verified`). **All v1 rows are code-complete and gate-green:** D1/D2/D3 OS-probe, D4+D11 restart-policy,
  D6/D7 file-watch (live `notify` adapter), D8 native notifications, the nightly soak gate + UI surfacing, and
  **D5 restart banner** (2026-06-21). The only thing between here and `Verified` is the **runtime acceptance
  walk via `just dev`** (user-only — see "Next session should start with" item B1). Phase 5 also remains
  `Done — pending verify` (runtime checks are user-only).
  **Phase-5 follow-up — now CODE-COMPLETE (2026-06-19 second feature session).** The two remaining pieces
  landed, each a gated single commit: **(1) project-load UI** (`d497241`) — a `project_load` Tauri command →
  `Facade::load_project`, a native folder picker via **`tauri-plugin-dialog`** (`dialog:allow-open`), an "Open
  project" affordance (toolbar + empty-state primary CTA), a `useProjects` store; **`demo.rs` deleted** so an
  empty app shows the empty state until a project is opened. **(2) trust review (A6/A9)** (`45461d0`) —
  `ProcessView.requires_trust` (carried on `ProcessSpawned`), `ConfigChanged` enriched with each pending
  command's detail, `Facade::trust_command` (+ `ConfigEngine::spec` accessor) behind the one gate; the sidebar
  blocks an untrusted command's Start and offers an inline **Trust** affordance, and a `solo.yml` change that
  needs trust pops a **review dialog** (`TrustDialog` + `useTrust`). `just lint && just test` green: **132
  tests** (Rust **103** / UI **29**). **First-open trust UX = Option B** (inline sidebar trust; the dialog is
  for yml *changes*), per plan/05 §4. **Still `Done — pending verify`, not Verified:** the runtime/manual
  observations are not done this session — opening a real `solo.yml` in the GUI, the inline trust path, and the
  B8 dialog need a `just dev` run; A9's *end-to-end* trigger (the dialog on a live file edit) awaits the
  **Phase-6 file watcher** (the dialog + its wiring are covered now by an emit-driven test, and the sync engine
  builds the diff/commands).

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
- **Last session:** 2026-06-19 — completed the Phase-5 follow-up: project-load UI (`d497241`, folder picker
  + `demo.rs` removed) and trust review A6/A9 (`45461d0`). Gate green at **132** (Rust 103 / UI 29). See the
  top of "Decisions / changes this session".

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
| 5 | Dashboard UI (sidebar tree, status dots, terminal pane, trust dialog) | **Done — pending verify** | **Update (4th 2026-06-19 session):** **A10 command auto-detection BUILT (now v1, code-complete)** — opening a folder with no `solo.yml` auto-creates one from detected commands (npm/Cargo/Go/Procfile/Make/Just/Compose) via a C1 Registry/Strategy detector set, trust-gated, with a friendly confirmation; full `solo.yml` reference added to README. **Deferred adversarial review FINISHED** (security re-verified sound; 2 fixes applied — `useTrust` apply-after-resolve `b637b50`, atomic `O_EXCL` create `8f8c524`; rest recorded as tracked findings). Gate **green: 174 (Rust 138 / UI 36)**. _(3rd session: silent empty-project-load fixed `72b526e`; project-load runtime-confirmed by the user.)_ — **Interactive core slice:** `DESIGN.md` seeded (`/impeccable`) + approved; full Tauri command/event/PTY-Channel adapter; TS domain mirror re-synced; sidebar tree (I1), color-blind-safe status (shape+color+label), per-row + bulk controls (B2/B3/B4), live status, xterm.js terminal pane (C1–C7 UI), empty/error states. **Follow-up now CODE-COMPLETE (2026-06-19):** mockIPC dashboard test; **orphan dialog (B8 UI)** + `kill_orphan`/`orphans_resolve`; **terminal title/bell → header**; **`Facade::load_project`** wiring; **project-load UI** (`d497241`: `project_load` command + `tauri-plugin-dialog` folder picker + "Open project" affordance + `useProjects`; `demo.rs` removed); **trust review A6/A9** (`45461d0`: `ProcessView.requires_trust` + enriched `ConfigChanged` + `Facade::trust_command` + inline sidebar Trust + `TrustDialog`/`useTrust`). `just lint && just test` green (**132**: Rust **103** / UI **29**). **Pending verify (runtime/manual):** render + a real human click started a process + echoed (2026-06-19, prior); **not yet observed this session** — opening a real `solo.yml` in the GUI, the inline trust path, the B8 dialog; **A9 end-to-end** (dialog on a live yml edit) awaits the **Phase-6 watcher** (emit-tested now); the real-window WebdriverIO/tauri-driver e2e (not Playwright) remains the automated gap. |
| 6 | Monitoring, restart (10/60s), file-watch, notifications | **Done — pending verify** | **Restart-policy slice (D4 + D11)** code-complete (`90d51ac` + review `9438f66`). **OS-probe slice — D1 + D2 code-complete (2026-06-20):** D1 per-process CPU/mem (`e0fa32e`) — new **C5 metrics domain** (`core/metrics/`, owns its `MetricsProbe` port + `ProcessMetrics`) + self-supervised, mock-clock-tested `MetricsSampler` + `MetricsTick`; **`crates/sys` created** (sysinfo adapter, process-subtree aggregation, per-core CPU%). D2 port discovery (`be1711a`) — **C5 portscan domain** (`core/portscan/`, owns its `PortProbe` port) + `PortScanner` → `ProcessView.ports` + `PortsChanged`; `crates/sys` `ProcPortProbe` reads `/proc` (subtree → socket inodes → `/proc/net/tcp{,6}` LISTEN). Self-supervision extracted to `core/supervision.rs` (shared by both samplers). D3 readiness (`4b4d930`) — `Facade::wait_for_port` (portscan `waiter.rs`, reuses `PortProbe`) polls until the port binds or times out; `ProcessView.ready` (now a `Readiness` enum: `Ungated` / `Waiting` / `Ready`) + `ReadyStateChanged`; the future MCP `wait_for_bound_port` (P8) is the production caller. **Review-fixes pass applied (2026-06-20):** pgid-guarded `set_ports`/`set_ready` (no stale-resurrect race), OS reads via `spawn_blocking`, exact `/proc` process-group membership (not parent-subtree), `Readiness` enum, supervisor read-model accessors split to `supervisor/monitoring.rs`. Gate **213 (Rust 171 / UI 42)**. **D6/D7 file-watch — CORE POLICY code-complete (2026-06-20):** new **C5 `core/filewatch/` domain** (owns its `FileWatcher` port + `Noop`, moved out of the `ports/mod.rs` stub) — pure `policy.rs` (`globset` matching relative to root, `*` crosses separators, **D7 default ignores**), `Clock`-driven `WatchReactor` reusing `core/debounce::Debouncer` → `Supervisor::file_restart` (delegates to the existing `Supervisor::restart`); `DomainEvent::FileRestart` (mirrored FE); `restart_when_changed` threaded `Registration`→`Registry`→`watch_targets()`; wired into `CorePorts` (Noop default) + `Facade::file_watch_loop()` spawned in the composition root (inert under Noop). 12 mock-clock tests; gate **225 (Rust 183 / UI 42)**. Branch `feat/phase-6-file-watch`. **Reviewed + fixed (2026-06-20):** file-watch reloads a *running* command only (no resurrecting a stopped/restored-resting one), `plan/05 §4`/parity-row citations stripped, reactor spawned after restore — see the top Decisions entry. **D6/D7 went LIVE (2026-06-20, `79de1cc`, PR #9):** `NotifyFileWatcher` (recursive `notify`, off-runtime, best-effort) in `crates/sys` + reactor **dynamic re-watch on `ProjectOpened`** (closes the once-at-startup limitation) + `build_facade .file_watcher(...)`; 4 real-inotify integration tests + 1 reactor re-watch test. **D8 native notifications DONE (2026-06-20, stacked branch `feat/phase-6-notifications`):** C7 `core/notify/` domain (owns `Notifier` port + `NoopNotifier` + `NotificationReactor`, global on/off) → desktop toast on crash/restart-exhausted; adapter = **Tauri notification plugin** (`TauriNotifier` in `crates/app`, per user directive — `plan/04` §1 updated); 4 notify mock-bus tests. Gate **234 (Rust 192 / UI 42)**. **Soak gate + UI surfacing + metrics fix DONE (2026-06-20, `feat/phase-6-soak`):** nightly soak (`crates/pty/tests/soak.rs` + `.github/workflows/soak.yml` + `just soak`) — flat fd/thread/task/PID + crash-storm-at-10/60s + sampler self-restart, all green/deterministic; UI surfacing of CPU%/RSS/ports + restarting(k/N)/not-ready/Exhausted (Task 9) via a coalesced `useSignal` context; and a **/proc metrics rewrite** (PSS + whole-machine CPU, `sysinfo` dropped) fixing user-reported 550%/9GB. Gate **Rust (core 160 / sys 14 / pty 9 +soak 3 ignored / store 13) / UI 60**. **D5 restart banner DONE (2026-06-21, `feat/phase-6-restart-banner`):** relaunch retains the terminal scrollback + draws a muted `── restarted ──` banner before new output. Fixed the crash-path buffer wipe + pane freeze — `Terminals::open` now **reuses** an existing process's buffers + live sender on relaunch (fresh input only); `Recorder::mark_restart` injects the banner iff prior output, called once at the actor's spawn-loop top so **one rule** spans crash/file/manual/user-start relaunches; no FE/Tauri change. Banner = dim ANSI raw / plain rendered. matrix D5 `later`→`v1`; plan/05 §12 records the every-relaunch scope decision. Gate **green: Rust core 163 / sys 14 / pty 10 +soak 3 ignored / store 13 / UI 60**. **All v1 code complete; remaining for `Verified` = the runtime acceptance walk via `just dev` (user-only).** Deferred: discrete file-restart row cue + D9/D10 toasts/bell (`later`). **R7 (port-ownership drift) logged** in `plan/06` §7. |
| 7 | Agents & idle detection (5-state FSM, optional summarization) | **Verified** | **E1/E2/E3 code-complete (2026-06-22, `feat/phase-7-agent-tools`, `55b3808`).** New **C4 `core/agents/` context** (promoted from the flat placeholder to a module folder that **owns its own driven ports**, like `notify`/`metrics`): `tool.rs` (closed `AgentKind` {Claude,Codex,Amp,Gemini,OpenCode,Copilot,Kimi,Generic} + `PromptMode` + `AgentTool` + the built-in provider set), `repo.rs` (`AgentToolRepo` durable port + `NoopAgentToolRepo`), `detect.rs` (`VersionProbe` port + `NoopVersionProbe` + `DetectedTool`), `mod.rs` (`Agents` surface: `list_tools` + `detect_installed`, probes run off-runtime via `run_blocking`). **store**: `AgentToolRepo` over SQLite (tool stored as its own JSON → persisted shape can't drift from the domain type); **migration v3** creates `agent_tools` + seeds the built-ins idempotently. **sys**: `CommandVersionProbe` runs `<command> --version` off-runtime, bounded timeout, hung probe killed+reaped. Wired through `CorePorts` (Noop defaults) + `Facade::agents()` + the composition root. **7 built-in tools seeded** (Claude/Codex/Amp/Gemini/OpenCode + Copilot/Kimi); **auto-detection covers the 5 Solo documents probing** — Copilot/Kimi (built-in types) and Generic are outside the probe set and report not-installed. (E1/E2/E3 merged to `main` via PR #13.) **E4 backend + E8 code-complete (2026-06-22, `feat/phase-7-agent-launch`, `a7235c6`):** `Facade::launch_agent(project, tool, extra_args)` resolves the tool + the project's working dir, composes the command line (`AgentTool::launch_command_line`, POSIX-quoted; `Agents::tool(name)` resolves a selection), and registers + starts an ungated `ProcessKind::Agent` on the interactive PTY (never `-p`) — **empty env overrides so the agent inherits Soloist's env unchanged (E8: `$DISPLAY`/`$BROWSER`/`ANTHROPIC_*` pass through; no credential stored/injected)**. `LaunchAgentError` types the failures. Adapter: thin `agent_list`/`agent_detect`/`agent_launch` Tauri commands → one Facade method each; `domain.ts` mirrors `AgentKind`/`PromptMode`/`AgentTool`/`DetectedTool`, `api.ts` the typed IPC. Tests: command-line composition + quoting; facade launch (Agent + Running, unknown-tool, unknown-project); a real-PTY integration test launching a stub agent (proves working dir + env inheritance, no global `set_var`). **E4 launch picker UI done (`2eb3f75`):** a `Cmd/Ctrl+T` shadcn `Command` (cmdk) command palette over the configured tools — Enter launches into the active project, Alt+Enter opens a one-shot flags field, multi-project fallback chooser, installed badges (shape+label); `store/useAgents` + `AgentPicker` (presentational) + `lib/tokenizeArgs`; `dialog.tsx` gained optional `showCloseButton` (single source, no overwrite). Gate **green: Rust core 177 / store 15 / sys 15 / pty 11 (+3 ignored) / UI 70**; `just lint` + UI build clean. **So E4 + E8 are code-complete.** **PR-review fixes applied** (shadcn `CommandGroup`; dropped dead `input-group`/`textarea`; source parity-IDs de-cited; `facade_tests.rs` extracted; `tokenizeArgs` unmatched-quote fix +1 test) — see the top Decisions entry. **E5 (5-state idle FSM) code-complete (2026-06-22, `feat/phase-7-idle-detection`):** new C4 `core/agents/idle/` subdomain — `AgentActivity` enum; a per-provider `IdleStrategy` (output-delta for Claude/OpenCode + the no-doc-heuristic defaults, OSC-title stability for Codex/Amp, OSC-title status for Gemini); an isolated conservative permission-cue detector; an edge-triggered classifier; a `ProcessId`→`AgentKind` tracker (keeps `AgentKind` out of C2); a `Clock`-driven self-supervised `IdleSampler` mirroring `MetricsSampler`. C3 exposes one `TerminalActivity` snapshot (monotonic output counter + retained title + rendered tail) via `Supervisor::terminal_activity`. `DomainEvent::AgentActivityChanged` emitted on transitions; C7 `NotificationReactor` toasts on Permission/Error. Wired through `Facade` (track at launch + `idle_sampler_loop`) + composition root. Frontend: `AgentActivity` mirror, an event-derived activity signal in `store/signals` (off the read-model list, cleared when an agent leaves Running), and a consolidated `ProcessIndicator` (activity-for-running-agent vs ProcStatus) replacing `StatusIndicator` — extends the existing glyph+color+label vocabulary (Working ▶ / Thinking ◐ pulse / Idle ○ / Permission ◆ / Error ✕), shaped via `/impeccable`, label on the **shadcn Tooltip** (added via CLI), one new `--status-attention` token, user-signed-off. Heuristics are pure + fixture-tested; one sampler test drives a real supervisor on the mock clock. Gap recorded: `KNOWN-DIVERGENCES.md` D-5 + plan/05 §12 (idle thresholds/cues are our approximation). **So all Phase-7 v1 rows (E1–E5, E8) are code-complete.** **Merged to `main`** via **PR #15** (`b95dc6a`); review-fixes `8763948` (settle-gated permission, cheap terminal tail, idle-sampler snapshot skip) included. Gate **green: core 202 / store 15 / sys 5 (+10 integration) / pty 11 (+3 ignored) / UI 77**. **E7 completes in P9; E6 (summarization) `later`, OFF by default.** Runtime verify: **user-confirmed working at runtime 2026-06-22** (the project owner verified the agent idle FSM + native login in the running app; per-state screenshot evidence not captured this session — recorded on the owner's confirmation) → **`Verified`**. |
| 8 | MCP server core (`soloist-mcp` stdio, scope+identity, tools) | **In progress** | **Session 1 — the MCP walking skeleton landed (2026-06-22, branch `feat/phase-8-mcp-skeleton`).** Proves the full path **agent → `soloist-mcp` (rmcp stdio) → IPC (UDS) → app → `Facade` → core**, with identity/scope and a read-only tool slice. **F1** (transport + a bundled stdio helper; `.deb`/`.AppImage` `externalBin` bundling deferred to P12), **F3** (effective project scope), **F4** (`bind_session_process`/`register_agent`/`whoami`/`select_project` + `SOLOIST_PROCESS_ID` injected into every managed process), and the **read subset of F5/F6** (`whoami`, `list_projects`, `get_project_status`, `list_processes`, `get_process_status`). New **C8 `core/identity`** (the `Identity` session registry + `Origin`/`Whoami`/`SessionId`; effective-project resolution composed in the façade); new **`crates/ipc`** (length-prefixed JSON framing + `IpcRequest`/`IpcResponse`/`IpcError` reusing core's domain/view types — `serde`-only DTOs, no drift — + the single `data_dir()`/`socket_path()` resolution the store now delegates to); the **app-side UDS server** (`ipc_server.rs`, **compile-time gated behind the `mcp` feature**, default on; one `Facade` method per request; degrades to a logged no-op if the socket can't bind); and **`crates/mcp`** (the `soloist-mcp` binary over **rmcp 1.7.0** — `#[tool_router]`/`#[tool_handler(router = self.tool_router)]`, clean-room `schemars` param schemas, structured results; a lazy, single persistent IPC connection that auto-binds via `SOLOIST_PROCESS_ID` and returns a clear "Soloist not running" when the app is down). Removability **verified**: `cargo check -p soloist-app --no-default-features` builds (no IPC server, no direct `soloist-ipc` dep). **Deferred to later P8 sessions:** F6 mutations, **F7** (`send_input`+`wait_ms`), F8 bulk, F9 output, F10 services, **F11 `spawn_agent`** (routes to the existing `Facade::launch_agent` — the E7 unblock), **F13** action trust+scope gating, and the helper's package bundling. F2/F12/F14 stay `later`. Gate **green: `just lint` + `just test` exit 0** — Rust core **215** (+13 identity) / store 15 / sys 5 (+10 integration) / pty 11 (+3 ignored) / **ipc 8** / **app 9** (IPC routing) / **mcp 9** (socket round-trip + handler) / UI 77; dep-direction guard green (core stays framework-free; `rmcp` never touches core). **Review-fix pass (2026-06-23):** fixed a latent `IpcResponse` serialization bug (internal→adjacent tagging — `list_processes`/`list_projects` now serialize over the wire), single-sourced data-dir creation + **0700** socket hardening, bounded the IPC request with a timeout, completed the `select_project`/`register_agent` tools, and DRY'd the per-id read; binding-authenticity deferred to F13 (read tools open by design; recorded `plan/05` §12). See the top Decisions entry. |
| 9 | Coordination layer (scratchpads/todos/timers/leases/kv) | Not started | **v1 scope.** Sequence: durable store → leases/locks → timers/idle-watchers → scratchpads/todos → key-value. High-risk |
| 10 | HTTP API & CLI (`127.0.0.1:24678` + `soloist` CLI) | Not started | |
| 11 | UX polish & execution profiles (palettes, deep links, themes) | Not started | |
| 12 | Packaging (`.deb` + `.AppImage`, x86_64) | Not started | Add containerized 20.04 AppImage smoke (webkit 4.0 runtime) here |
| 13 | Parity QA + longevity gate | Not started | The v1 definition-of-done; runs the soak/leak gate and parity walk |

Estimated v1 critical path: **~14–18 focused weeks** (one experienced Rust+TS dev); Phases 3, 8, 9 carry
the most risk. See `plan/phases/phase-13-parity-qa-testing.md` appendix for the per-phase breakdown.

---

## Decisions / changes this session

### Performance optimization — research pass, measurement tooling & evidence-based backlog (2026-06-23, user-directed, cross-cutting)
**This is a cross-cutting performance pass at the user's explicit direction — it does NOT change the
active phase (Phase 8, MCP) or any phase's status, and the Phase-8 / Phase-6 "Next session should start
with" pointers above stand unchanged.** User directive: optimize performance/responsiveness over time
*without breaking anything*; keep the app clean and working (the stated top priority); drive all perf work
through the Tauri skills + official docs / valid sources; no assumption, no fabrication; **append to this
ledger, never overwrite it.**

- **Process followed (now codified as a CLAUDE.md rule).** Loaded five Tauri skills —
  `tauri-performance-optimization`, `tauri-binary-size`, `tauri-calling-frontend`, `tauri-process-model`,
  `tauri-configuration` — plus official-doc + web research (the Tauri v2 size doc's `removeUnusedCommands`
  semantics; WebKitGTK-on-Linux jank notes; `rollup-plugin-visualizer` maintenance/compat). Ran a
  read-only Rust hot-path audit and read the frontend 60 fps path. Grounded the pass in `plan/00` (vision),
  `plan/04` (longevity), `plan/05` (identity), `plan/06` (blueprint) and the §6 budget / §8 longevity
  invariants — per the user's reminder not to tunnel on perf and ignore what the app is / where it heads.
- **CLAUDE.md — performance-workflow rule added (the "add the rule" directive):** §6 gained a
  **"Doing a performance pass — the workflow (MANDATORY)"** block: skills + valid sources first (never
  memory); measure before *and* after (`just bloat`, `just bundle-size`, the soak gate, webview
  devtools); stay in adapters / the composition root, never the pure `core`, never weaken a cap / test /
  typed boundary for a micro-win; and an explicit **locked-non-changes** checklist. (The "append to this
  ledger" instruction was a session-only directive, per the user — not codified as a permanent rule.)
- **Applied safe, verified code win (behavior-identical):** `core::terminal::buffers::RawScrollback::to_vec`
  now bulk-copies the ring's two `as_slices()` halves via `extend_from_slice` instead of a byte-by-byte
  `iter().copied().collect()` — the up-to-256 KB raw-scrollback replay on **every** PTY attach is now two
  memcpys, not N byte-pushes. Proven behavior-identical by the existing `raw()`-asserting tests
  (`rendered_strips_escapes_while_raw_keeps_them`, `the_raw_scrollback_never_exceeds_its_byte_cap`, the
  global-budget tests). No port/boundary/test changed.
- **Measurement tooling (zero behavior change — "measure first" made a one-command habit):** `just bloat`
  (cargo-bloat on the release app binary; a tool, **not** a Cargo dep), `just bundle-size` (real
  `.deb`/`.AppImage` + frontend `dist` per-asset bytes), `just ui-analyze` (frontend treemap →
  `dist/bundle-stats.html`). Added the **maintained** `rollup-plugin-visualizer` 7.0.1 as a **dev-only**
  dep, wired into `vite.config.ts` **gated behind `ANALYZE`** so a normal build is byte-identical
  (verified: no stats file without the flag; 809 KB treemap with it; `pnpm-lock.yaml` re-synced).
- **First measured baseline (real numbers, not fabricated — frontend, dev `pnpm build`):** a single JS
  chunk **697.51 kB (gzip 200.06 kB)**, CSS **56.60 kB (gzip 10.54 kB)**, fonts ~76 kB woff2. Rolldown
  itself warns the JS chunk exceeds 500 kB and suggests `import()` code-splitting — concrete evidence for
  the §6 "lazy-load / code-split" target (xterm.js, cmdk, radix-ui, lucide are all eagerly bundled today).
- **Evidence-based performance backlog (measure-first; mapped to the phase built to measure it — do NOT
  apply blindly):**

  | Item | Where | Severity | Why deferred / next step |
  |------|-------|----------|--------------------------|
  | Code-split / lazy-load the frontend bundle (xterm.js, cmdk) | `crates/app/ui` | Med (measured: 697 kB chunk) | Phase 12 — act from the `just ui-analyze` treemap once bundle size is measured for real |
  | `/proc` full-sweep + duplicate per-member `stat` read each sampler tick | `crates/sys/src/{proc,metrics,portscan}.rs` | Med (CPU; scales w/ machine PID count) | Sweep is **correctness-required** (double-forked descendants keep their pgrp; no `/proc/<pgid>/members`); the duplicate member read is low-ROI vs the DRY/clarity cost. Measure idle CPU in the Phase-13 soak before acting |
  | PTY chunk path: 3 alloc/copies per chunk (`Vec`→`Arc` realloc, `Arc`→`Vec` at the IPC boundary) | `crates/pty/src/lib.rs`, `core/src/terminal.rs`, `app/src/commands.rs` | Med (high-throughput only) | Needs a throughput benchmark; the step-2 fix changes the `ProcessSpawner` output channel type + ripples through tests. Measure first |
  | `opt-level` 3 → `"s"`/`"z"` (size vs speed) | `Cargo.toml [profile.release]` | — (size) | A **Phase-12 measured** decision (`just bundle-size` before/after); not a blind flip. `LTO`+`codegen-units=1`+`strip` already on |
  | `removeUnusedCommands: true` | `crates/app/tauri.conf.json` | — (size) | Strips command handlers absent from the ACL; needs **every** app command added to a capability **and** a user-only `just dev` runtime verify before it's safe. `tauri@2.4+` available (we're on 2.11.2) |
  | `rendered()` clones all 5 000 lines per query | `core/src/terminal/buffers.rs` | Low | Only on an explicit `get_process_output`, not a hot path; act only if a caller polls it |
  | signals `new Map()` per `MetricsTick` (O(N)/tick) | `crates/app/ui/src/store/signals.ts` | Low (scale only) | Fine at current scale; revisit if the "hundreds of processes" target is exercised |

- **Locked non-changes — confirmed and NOT touched (deliberate per §3 / `Cargo.toml`):** `panic = "unwind"`
  (the supervisor catches task panics for fault isolation — `panic = "abort"` would break it, despite the
  generic skill advice), `freezePrototype = false` (true breaks xterm.js → blank window), the `Cargo.lock`
  brotli pins, release `opt-level` (the size-vs-speed call is Phase-12), and `removeUnusedCommands` (see
  backlog).
- **Gate: `just lint && just test` green.** `cargo fmt` + `clippy --workspace --all-targets -D warnings`
  clean across all 9 crates; dep-direction guard green (core stays framework-free); UI typecheck / ESLint /
  Prettier clean; Rust workspace tests + **UI 77/77** pass; vitest re-confirmed green with the modified
  `vite.config.ts`. The file-size advisory (`facade.rs` 446 / `registry.rs` 404) is **pre-existing**
  (`plan/06` §7 split roadmap), untouched this pass.
- **Flagged (pre-existing, not created here, left alone):** `git status` shows an untracked directory
  `.claude/skills/tauri-performance-optimization\n/` (a stray trailing-newline name). I did not create it
  and left it untouched — the working skill loaded from the correct path; worth a cleanup look.
- **Next perf steps (these do NOT disturb the Phase-8 pointer above):** at Phase 12, measure the real
  `.deb`/`.AppImage` via `just bundle-size`, then decide `opt-level` and code-split the 697 kB frontend
  chunk (lazy-load xterm/cmdk) using the `just ui-analyze` treemap; at Phase 13 (soak), measure idle
  RSS/CPU and only then weigh the `/proc` sampler sweep + the PTY 3-copy chunk path (both want the soak
  number first).

### Phase 8 session 1 — review-fix pass (2026-06-23, `feat/phase-8-mcp-skeleton`)
Independent `/soloist-review` of PR #16 returned **fix-then-ship**; every finding + nit applied this session
and the gate re-run green. No locked decision touched; the read-only tool surface is unchanged in behaviour
(one latent serialization bug fixed — see below). User directive: single trusted source, no duplication, no
assumptions, strict CLAUDE.md.
- **Latent bug fixed (headline) — `list_processes`/`list_projects` could not serialize over the wire.**
  `IpcResponse` was **internally** tagged (`tag = "ok"`), which serde cannot use for the `Processes(Vec)` /
  `Projects(Vec)` newtype-of-sequence variants — the app's `write_frame` would error and drop the connection.
  Switched to **adjacent** tagging (`tag = "ok", content = "data"`), which serializes every variant shape.
  The shipped tests missed it (none serialized those variants; the in-process `handle_request` test never hit
  the wire). Now guarded at the source by an `ipc` `every_response_variant_round_trips_through_json` test and
  by the new `mcp` handler tests.
- **Single-source data-dir creation + 0700 socket hardening (security).** New `ipc::paths::ensure_data_dir`
  (resolve → create → `0o700` on unix) + `ensure_socket_path`; the app's IPC server and the store's
  `open_default` + runtime-state all create the data dir through it, so it is made **once** and restricted to
  its owner (another local user can no longer reach the socket or the SQLite DB). Removed the duplicated
  `create_dir_all` in the IPC server and the redundant `store::data_dir` wrapper (one internal caller; now
  calls `soloist_ipc` directly). Binding-*authenticity* (peer-credential → process-group match) is **deferred
  to F13** — the read tools are open by design, so no project boundary is crossed yet — recorded in `plan/05`
  §12 (user-approved 2026-06-23).
- **IPC request is now bounded.** `mcp::client` wraps every `exchange` in a named `REQUEST_TIMEOUT` (30 s), so
  a wedged app surfaces as `ClientError::Timeout` instead of hanging the MCP host; deterministic paused-clock
  test added.
- **F4 identity surface completed.** `select_project` + `register_agent` exposed as MCP tools (the Facade/IPC
  plumbing already existed; only the rmcp wrappers were missing), so an external agent can label itself and
  set scope. The single-project default + ambiguity rule recorded in `plan/05` §12.
- **DRY: one per-id read.** New `registry.view` / `supervisor.view` / `Facade::process_view` replace the
  `snapshot().into_iter().find(id)` clone-the-whole-list pattern (`effective_project` + `get_process_status`).
- **Comment accuracy.** The app `mcp`-feature comment no longer claims it "drops the soloist-ipc dependency
  entirely" (the crate stays via `store`); reworded to the app's *direct* dependency + the server adapter.
- **Doc drift.** `plan/06` §2 crate table regained the missing `sys` row and `sys` in the app deps.
- **Gate:** `just lint` + `just test` **exit 0**; dep-direction guard green; `cargo check -p soloist-app
  --no-default-features` still builds (removability holds). Tests: core **215** / store 15 / sys 5 (+10
  integration) / pty 11 (+3 ignored) / **ipc 8** (+1 response round-trip) / app 9 / **mcp 9** (+7: 1 request
  timeout, 6 handler) / UI 77. Advisory (non-gating) file-size: `facade.rs` 446 + `registry.rs` 404 over the
  ~400 non-test smell — `facade.rs` is on the `plan/06` §7 split roadmap; `registry.rs` crossed by the 6-line
  `view` accessor (left in place — correctly homed beside `label_of`/`snapshot`).

### Phase 8 session 1 — the MCP walking skeleton (2026-06-22, `feat/phase-8-mcp-skeleton`)
- **Scope (user-approved):** the MCP walking skeleton + **compile-time** Cargo-feature removability. Built the
  full path end-to-end (rmcp stdio → IPC/UDS → app → `Facade` → core) with a read-only tool slice, de-risking
  the transport/identity before fanning out the ~30 tools (the Phase-1 "architecture before features" move).
  F-rows landed: **F1, F3, F4** + the read subset of **F5/F6**; the rest of the v1 F-set is later P8 sessions.
- **`core::identity` (C8).** Filled the placeholder: a per-connection `Identity` session registry
  (`SessionId → {Origin, selected_project}`), closed `Origin` enum (`Unbound`/`Process`/`External`), `Whoami`
  view, and `PROCESS_ID_ENV`. The **effective-project** resolution (selected → bound-process's project → sole
  project → ambiguous) is composed in the **façade** (which alone sees projects + supervisor), keeping the
  registry pure. `SOLOIST_PROCESS_ID` is injected once in the **actor** (`run`), so every managed process —
  and every restart — carries its own id for the agent to bind to.
- **`crates/ipc` reuses core types (single-source) — doc reconciled.** The stub already depended on `core`;
  kept that and made the wire DTOs **reuse** `ProcessView`/ids/`Whoami` (added `Deserialize`+`PartialEq` to
  `ProcessView`) rather than parallel DTOs that could drift — except a lean `ProjectSummary` (no UI icon blob
  for agents). `ipc` now owns the **single** `data_dir()`/`socket_path()` resolution; **`store` delegates** to
  it (removed the duplicated env logic). So `ipc` depends on `core`+`serde`+`tokio`, not "serde only" —
  **fixed `plan/06` §2 + `ARCHITECTURE.md`** (and noted the CLI→ipc→core type-linkage consequence to revisit
  in P10 if size matters). Framing is length-prefixed JSON with a hard `MAX_FRAME` cap (bounded buffers, §8).
- **Removability = compile-time Cargo feature (user's choice).** The app-side IPC server links into the app,
  so it is gated `#[cfg(feature = "mcp")]` (`mcp = ["dep:soloist-ipc", "tokio/net"]`, default on). **Verified**
  `cargo check -p soloist-app --no-default-features` builds with no IPC server and no direct `soloist-ipc`
  dep — the "remove MCP, the app still builds and runs" guarantee, mechanically checked.
- **rmcp 1.7.0 (current, maintained).** Tools via `#[tool_router]`/`#[tool]`; used
  `#[tool_handler(router = self.tool_router)]` (verified against the macro source) so the router is built once
  (not per call) and the cached field is read — fixing a `dead_code` warning **honestly**, not by `allow`.
  Tool results are `CallToolResult::structured(serde_json::to_value(..))` so the core types need **no**
  `schemars` derive (schemars stays confined to the mcp crate's own param structs — core stays minimal).
- **`mcp-builder` skill is not installed this session** — fell back to the official MCP docs
  (`code.claude.com/docs/en/mcp*`) + the `rmcp` docs via context7 (§4), as the rules direct.
- **Gate:** `just lint` + `just test` both exit 0. New tests: identity 13 (8 registry + 5 façade), ipc 7
  (framing + protocol), app 9 (IPC routing contract), mcp 2 (real-socket round-trip + not-running). No tests
  removed; no existing behaviour changed (launch/restart/UI untouched).

### Phase 7 → `Verified`; Phase 8 (MCP core, v1 rows) chosen as next (2026-06-22)
- **Phase 7 runtime-confirmed by the project owner** ("I checked phase 7 and its working"). Flipped Phase 7
  → `Verified`. Recorded honestly as **user-confirmed at runtime 2026-06-22** — the owner verified the agent
  idle FSM (E5) + native login (E8) in the running app; per-state screenshot evidence was **not** captured
  this session (recorded on the owner's confirmation, not an in-session observation). PR #15 (`b95dc6a`) was
  already merged + CI-green on `main` (the session briefing's "PR #15 still open / merge it" was stale —
  resolved against PROGRESS + git: `b95dc6a` is the merge commit, branch deleted, `4f787ee` records it).
- **`later`-row triage + next-step decision (user).** Surveyed all 21 `later` rows. The owner chose to **stay
  on the v1 critical path** and start **Phase 8 (MCP server core), v1 rows only** (F1, F3–F11, F13) — **not** a
  `later` sweep and **not** folding F2/F12 in yet. E6 (auto-summarization) stays locked **OFF** by design;
  F2/F12 (setup snippet + `setup_agent_integration`) remain `later`, to be reconsidered per-row inside Phase 8.
  Rationale: Phase 8 is the metaharness centerpiece, high-risk, and unblocks Phase 9 (Coordination, v1) + E7.

### PR #15 review + fixes — settle-gated permission, cheap terminal tail (2026-06-22, `feat/phase-7-idle-detection`)
- **Independent review of PR #15 (E5 idle FSM) via `/soloist-review`; verdict ship, with 2 should-fixes + 1 nit
  applied at the user's request and pushed. No behaviour removed; all gates re-run green.**
- **S1 — cheap terminal tail.** `Terminals::activity()` was cloning the whole rendered scrollback (≤5,000 lines)
  every sample (~1 Hz per running agent) to keep the last 8. Added `Ring::tail(n)` + `TerminalBuffers::tail(n)`
  (the last `n` lines, including the in-progress line the permission detector reads) — activity now clones ≤8
  lines, not the buffer.
- **S2 — no sticky false `Permission`.** `OutputDelta` now checks the permission cue only on the **quiet branch**
  (once output has settled), so a terminal still producing output reads as `Working` even if a just-answered
  prompt lingers in the tail — the failure mode D-5 itself rates as the worst (a free agent reported blocked).
  Detection of a real prompt is delayed by one ~1 s sample; multi-line menu prompts still match (`SCAN_LINES`
  kept at 3). `KNOWN-DIVERGENCES.md` D-5 + `plan/05` §12 updated to the settled-output rule; +1 regression test.
- **N1 — idle sampler skips the snapshot when no agents are tracked** (after the shutdown `upgrade()` check, so
  shutdown still terminates the loop), avoiding a `supervisor.snapshot()` + map build each second in the common
  no-agents-running case.
- **Gate after fixes:** `just lint` + `just test` green — Rust **core 202** (+1 net test) / store 15 / sys 5
  (+10 integration) / pty 11 (+3 ignored) / UI 77. Commit **`8763948`**; **merged to `main`** via PR #15
  (`b95dc6a`, branch deleted). Runtime acceptance still owed (user-only) — see "Next session should start with"
  item A.

### E5 — the 5-state agent idle FSM (2026-06-22, `feat/phase-7-idle-detection` off `main`)
- **Why `AgentActivity` exists (understood, not taken on faith — the user asked).** It is the observable
  substrate the coordination layer needs: a way to know — without a human watching — whether an agent is
  **busy, available, or blocked**. Two questions: *busy vs available* (`Working`/`Thinking` vs `Idle`) and
  *needs a human* (`Permission`/`Error`). The load-bearing distinction is **`Permission` ≠ `Idle`**: a quiet
  terminal can be a blocked prompt, which a Phase-9 fire-when-idle timer must **not** treat as done (else the
  delegation deadlocks). That is why the state is richer than a quiet/active boolean. Immediate consumers:
  notifications (now) + the UI; future: fire-when-idle timers (P9). It only *informs*, never auto-acts (the
  signal is a heuristic — "a quiet terminal is not always completed work").
- **Clean C4 decomposition.** New `core/agents/idle/` subdomain: `activity.rs` (the closed `AgentActivity`
  enum), `strategy.rs` (the `IdleStrategy` trait + 3 per-provider heuristics + `strategy_for` registry —
  exhaustive over `AgentKind`), `permission.rs` (an isolated, conservative permission-cue detector),
  `classifier.rs` (edge-triggered FSM — emits only on a transition), `tracker.rs` (the `ProcessId`→`AgentKind`
  map — deliberately keeps `AgentKind` out of C2's `Registration`, so the process model stays free of the agent
  taxonomy), `sampler.rs` (Clock-driven, self-supervised, reuses `supervision::supervise`, mirrors
  `MetricsSampler`). The heuristics are **pure functions over a small `AgentMemory`**, so the fuzzy
  provider-specific logic is fixture-tested with no clock/PTY.
- **C3 reports, C4 interprets.** C3 gained one read-model — `TerminalActivity` (monotonic output counter +
  retained latest title + rendered tail) via `Supervisor::terminal_activity`; the output counter + last title
  were added to `TerminalBuffers` (bumped/captured in `ingest`). C4 reads these raw facts; all interpretation
  (busy/idle/permission, per provider) stays in the agents context.
- **Per-provider Strategy (faithful to plan/05 §6).** Output-delta → Claude/OpenCode (+ Copilot/Kimi/Generic,
  which Solo documents *no* heuristic for, so they default to the most universal signal); OSC-title stability →
  Codex/Amp; OSC-title status → Gemini. `AgentActivityChanged` emitted on transitions; C7's `NotificationReactor`
  learned two arms (Permission/Error → toast) — one attention vocabulary, no new mechanism.
- **UI surfacing via `/impeccable` + shadcn (per the user's two directives).** DESIGN.md §2 already reserved
  this ("extends this same shape+color+label system… do not introduce a parallel status vocabulary"), so the
  indicator extends the **custom** status component, not a shadcn Badge (a Badge would be the forbidden parallel
  pill). Consolidated the row + header indicator into one `ProcessIndicator` (chooses activity-for-running-agent
  vs ProcStatus), deleting `StatusIndicator`. Vocabulary (user-signed-off): **Working ▶** green, **Thinking ◐**
  amber (pulse), **Idle ○** slate, **Permission ◆** new `--status-attention` gold, **Error ✕** red — reuses 4
  existing tokens + 1 new. Label rides the **shadcn Tooltip** (added via CLI; uses the existing `radix-ui` dep)
  in the dense row, inline in the header. `App` wrapped in `TooltipProvider`. Activity is an **event-derived
  signal** in `store/signals` (off the read-model list, like metrics; cleared when an agent leaves Running).
- **Gap recorded (clean-room §9).** The exact quiet window (3 unchanged samples ≈3 s), permission cue set, and
  title keywords are undocumented by Solo → our approximation, recorded in `KNOWN-DIVERGENCES.md` **D-5** +
  `plan/05` §12. Permission detection is deliberately conservative (prefers a miss to a false block).
- **Gate after E5:** `just lint` + `just test` green — Rust **core 201** (+24: 21 idle + 3 notify attention) /
  store 15 / sys 5 (+10 integration) / pty 11 (+3 ignored); **UI 77** (+5 ProcessIndicator + 2 signals). No
  tests removed; no launch/restart behaviour changed.

### Review fixes on PR #14 (E4/E8 agent launch) — shadcn composition, dead-code, discipline (2026-06-22, `feat/phase-7-agent-launch`)
- **Independent review of PR #14; the agreed fixes applied. No launch behaviour changed** — the core
  launch path, the env passthrough, and the picker flow are untouched; the fixes are
  composition/discipline/cleanup.
- **shadcn composition.** The `AgentPicker` tool list and project chooser now wrap their `CommandItem`s
  in a `CommandGroup` (the shadcn "items inside their group" rule); cmdk worked without it, but the
  grouping is the sanctioned structure.
- **Dead-code dropped.** `CommandInput` was rewritten as a plain bordered search wrapper, so the command
  palette no longer pulls in `components/ui/input-group.tsx` (only 2 of 6 exports were used) or
  `components/ui/textarea.tsx` (entirely unused) — both files deleted. The `Command`/`CommandDialog`
  radius `rounded-xl!` → `rounded-lg`, matching `DialogContent` and the 6px radius scale.
- **Discipline.** Parity-matrix IDs stripped from source comments — `(E4)`/`(E8)`/`(E4/E8)` in
  `facade.rs`, `commands.rs`, `pty/tests/integration.rs`, plus the pre-existing `(A6)` in `supervisor.rs`
  (only the C1–C8 context IDs are sanctioned in source; matches the prior "parity-row citations stripped"
  review). The `Facade` test module moved out of `facade.rs` into a sibling `facade_tests.rs` (`#[path]`),
  matching the 2026-06-20 separate-file convention the rest of this PR follows.
- **`tokenizeArgs` edge case.** An unmatched quote in the "agent with flags" field is now kept as a
  literal (e.g. `O'Brien`) instead of being silently dropped; a test covers it. The core still re-quotes
  each token safely.
- **Gate after fixes:** `just lint` + `just test` green — Rust unchanged (core 177 / store 15 / sys 15 /
  pty 11 +3 ignored), **UI 70** (+1 tokenizer test). No launch behaviour changed; no tests removed.

### E4 + E8 — agent launch on the interactive PTY with env passthrough (2026-06-22, `feat/phase-7-agent-launch`)
- **Branch off `main` (user-confirmed).** PR #13 is merged — HEAD `10b484f` is the PR-#13 merge commit, so
  E1/E2/E3 + the reactor/waiter determinism fixes are on `main`. Branched **`feat/phase-7-agent-launch`** off
  it. One feature commit (`a7235c6`) + this `docs(progress)`.
- **Scope this session (user-confirmed): E4 backend + E8; STOP before E5.** The E4 **launch picker UI** is the
  one remaining E4 piece and is **gated on the user's visual sign-off** (a new surface; DESIGN.md is the
  source) — deliberately not built this session.
- **One core launch behaviour, `trust_command`-shaped (not a new service).** `Facade::launch_agent(project,
  tool, extra_args)` orchestrates the three Facade-owned contexts directly — Agents resolve the tool → Projects
  resolve the working dir → Supervisor register + start — mirroring `trust_command` rather than a
  `ProjectService`-style service (~6 lines, and the one shared entry point for the Tauri command now and the
  MCP `spawn_agent` tool later, E7; extract to a service if B9/prompt-modes grow it — YAGNI). The pure
  agent-domain logic stays in the agents context: `AgentTool::launch_command_line(extra_args)` composes
  `command + default_args + extra_args` with POSIX single-quote escaping (single source for the command line);
  `Agents::tool(name)` resolves a picker selection. `LaunchAgentError` types unknown-tool / unknown-project /
  store / supervisor.
- **E8 = passthrough, zero injection.** The spawn uses **empty env overrides**, so the agent inherits Soloist's
  process env unchanged (the PTY adapter's `CommandBuilder` seeds from the current env — `$DISPLAY`/`$BROWSER`/
  `ANTHROPIC_*` pass through) and runs on the **interactive PTY** (`Registration::launched` →
  `ProcessKind::Agent`, never `-p`). Soloist stores/injects no credential and never touches the CLI's
  credential file (plan/05 §6). The fresh-`claude` native-login acceptance is the **manual** smoke (test plan).
- **Adapter is thin (plan/06 §5.5).** `agent_list` (instant, no probe), `agent_detect` (async `--version`, for
  installed badges), `agent_launch` → one Facade method each, registered in `invoke_handler`; `domain.ts`
  mirrors `AgentKind`/`PromptMode`/`AgentTool`/`DetectedTool` once; `api.ts` holds the command-name strings
  (`extra_args`↔`extraArgs` per Tauri's snake→camel arg mapping, like the existing `on_chunk`↔`onChunk`).
- **Tests (honest, deterministic).** `launch_command_line` order + quoting (an arg with spaces → one token, an
  embedded `'` → `'\''`); facade launch registers an Agent + reaches Running, plus unknown-tool and
  unknown-project; and a **real-PTY integration test** (`crates/pty/tests/integration.rs`) launches a stub
  agent script that writes its `pwd` + `$HOME` to a project-relative file — the file landing under the project
  root proves the working dir, the matching `$HOME` proves env inheritance (E8). No global `env::set_var`
  (avoids the `setenv`/`getenv` data race that would reintroduce flakiness). Gate **green: core 177 (+7) /
  store 15 / sys 15 / pty 11 (+3 ignored) / UI 60**; `just lint` (clippy `-D warnings`, fmt, tsc, ESLint,
  Prettier, dep-direction, file-size) all pass.
- **E4 launch picker — DONE (2026-06-22, `2eb3f75`), shaped via `/impeccable`, built with shadcn.** A
  `Cmd/Ctrl+T` command-palette overlay over the configured tools, fully keyboard-driven: Enter launches the
  highlighted tool instantly into the active project; **Alt+Enter** opens a one-shot flags field ("agent with
  flags"); when several projects are open and none is active it asks which first, and the footer always names
  the target. Detected tools are badged (shape + label, **not** the saturated status palette — install is not a
  `ProcStatus`). Also reachable via a Toolbar "Launch agent" action. **User signed off on the visuals**
  (progressive-flags layout + active-project-with-switcher targeting). Built on the **shadcn `Command` (cmdk)**
  inside the existing `Dialog`; `dialog.tsx` gained the upstream-standard optional `showCloseButton` (defaults
  true → TrustDialog/OrphanDialog unchanged) so the palette omits the X — **single source kept, the existing
  primitive was not overwritten**. Structure: `store/useAgents` (lists instantly, merges `--version` detection,
  routes launch to the one core method), `AgentPicker` (presentational, no IPC), `lib/tokenizeArgs` (quote-aware
  argv split; the core still re-quotes — **no shell-quoting logic duplicated**). New shared `vitest.setup.ts`
  polyfills ResizeObserver/scrollIntoView (jsdom gaps cmdk needs). New dep **`cmdk`** (the canonical
  command-palette primitive; UI bundle ~167→**187 KB gzip** — a §6 item to weigh against the Phase-12
  xterm-lazy-load work). Gate **green: UI 69 (+9: 4 picker + 5 tokenizer)**; `just lint` clean; UI build OK.
- **Not done / next:** **E5** — the 5-state idle FSM (`IDLE/PERMISSION/THINKING/WORKING/ERROR`) sampler with a
  per-provider Strategy + activity surfacing (reusing the glyph+color+label vocabulary). E6 `later`; E7 in P9.
  Branch `feat/phase-7-agent-launch` is **not pushed / no PR** — awaiting the user's call. `package-lock.json`
  left untracked (recorded decision).

### Review fixes on the Phase-7 PR — flaky reactor tests + discipline nits (2026-06-22, `feat/phase-7-agent-tools`)
- **Independent review of PR #13 (this branch); the agreed fixes applied. No feature behaviour changed —
  the agent-tool slice's code is untouched; the fixes are test-stability + discipline.**
- **Flaky filewatch/notify reactor tests fixed at the root (the headline).** Under full-workspace parallel
  load the `filewatch::reactor::tests` (and the same-pattern `notify::reactor::tests`) intermittently failed
  (reproduced **7/40** under CPU load, all at the `start_running` helper). Cause: the helpers waited for an
  async effect via a **fixed `yield_now` budget** — fine for cooperative effects, but the supervisor actor's
  path to `Running` depends on blocking work, so a yield budget can exhaust before it completes. The file's
  docstring also falsely claimed the waits were "deterministic on the mock clock." Fix: the generic
  event-stream waiters (`next_change`/`next_to`/`wait_all` + a new `next_matching`) moved out of
  `supervisor/test_support.rs` into **`core::testing` as the one source** (re-exported there for the
  supervisor's existing callers, so they are unchanged); the filewatch/notify suites now **await** the real
  signal — `wait_all` for a status transition, `FakeFileWatcher::established()` (new `Notify`) for a watch,
  `RecordingNotifier::wait_until_shown` (new `Notify`) for a toast, `next_matching` for a `FileRestart` —
  instead of polling. Cooperative clock-advance retry loops (the debounce window, negative assertions) stay,
  since the reactor's arming is purely cooperative. Docstrings corrected. **Pre-existing** (the suites are
  Phase-6 code; not introduced by this PR), but they made the gate non-deterministic.
- **Discipline nits applied.** New `crates/store/src/agents.rs` tests moved to a sibling
  `agents_tests.rs` (the 2026-06-20 separate-file directive; matches the core half of this PR).
  `AgentTool` doc now records the persisted-JSON **field-evolution rule** (`#[serde(default)]`/migration for
  any later field). `plan/05 §6` now cites the **Copilot/Kimi CLI-command grounding** (`copilot`/`kimi`,
  web-sourced) so the clean-room trail is complete. This `idle.rs` ledger line corrected (no such file
  exists yet). **Still deferred to E4 (noted, not defects):** `prompt_mode`/`default_args` are persisted but
  unconsumed until launch lands, so E3's "defaults applied on launch" is not yet verifiable; the per-tool
  "tool-type mode (auto-detect/manual)" field (in `plan/05`/phase-07 Task 1, not in matrix E3) is deferred
  to the editing/launch slice.
- **Gate after fixes:** `just lint` + `just test` green; the flaky suites re-run under CPU load (40×) with
  zero failures. Counts unchanged (core 170 / store 15 / sys 15 / pty 9 +3 ignored / UI 60) — refactors and
  a test-file move, no tests added or removed.

### Phase 7 begins — agent-tool registry + `--version` auto-detection (E1/E2/E3) (2026-06-22, `feat/phase-7-agent-tools`)
- **Phase pivot (user directive).** The user directed **Phase 7** while Phase 6 stays **Done — pending
  verify** (its only gap is the user-only runtime acceptance walk, not code). Proceeding on Phase 7 per
  source-of-truth #1 (the user); Phase 6's runtime walk is still owed before it flips to **Verified**.
- **Phase 7 sliced like Phase 6 was** (a ~5–7-day phase is not one session). User-confirmed first slice =
  **E1/E2/E3** (registry + autodetect, pure core + store + sys, no UI). Branch `feat/phase-7-agent-tools`
  off `main`; one feature commit (`55b3808`) + this `docs(progress)`. `just lint && just test` **green**:
  clippy `-D warnings`, fmt, tsc, ESLint, Prettier, **dep-direction** (core still framework-free — the
  agents ports live in core; the subprocess probe lives in `crates/sys`) and **file-size** guards all pass.
  Gate **Rust core 170 (+7) / store 15 (+2) / sys 15 (+1) / pty 9 (+3 ignored) / UI 60**.
- **C4 built to the newer-domain bar (the R7 target), not the old shared-`ports/` shape.** The flat
  `agents.rs` placeholder became a `core/agents/` module folder that **owns its own driven ports**
  (`AgentToolRepo`, `VersionProbe` + their `Noop`s) — mirroring `notify`/`metrics`/`portscan`/`filewatch`
  rather than adding to the `ports/mod.rs` god-file. The 5-state idle FSM is a later slice — no module
  exists for it yet (the `agents/` folder is `mod.rs`/`tool.rs`/`repo.rs`/`detect.rs`; idle lands when built).
- **Persisted shape = the domain type's own JSON (single source).** The store keys `agent_tools` by `name`
  and stores each tool's `serde_json` as the `definition` column (+ `position` for order), so the durable
  encoding cannot drift from `AgentTool`; no per-column mapping, no magic strings. Migration **v3** seeds the
  built-ins from `AgentTool::builtin_defaults()` (the one source) idempotently (`INSERT OR IGNORE`, version
  gate) — a reopen never re-seeds, and a user-edited tool is never clobbered. Seed-data evolution (built-ins
  changing after install) is intentionally left to the launch/settings slice when editing lands.
- **Probe is bounded + reaping (longevity §8).** `CommandVersionProbe` runs `<command> --version` off the
  async runtime (the core calls it via `run_blocking` → `spawn_blocking`), with a 2s default timeout; a hung
  child is killed and reaped so the probe never leaks a process. The sys test is deterministic — it probes
  temporary executables (exit 0 / exit 3 / a sleeper for the timeout path), so the result never depends on
  which agent CLIs the machine has.
- **Built-in tool set vs auto-detect set, kept distinct (faithful to `05` §6).** Two facts that must not be
  conflated: the **built-in tool types** (Claude/Codex/Amp/Gemini/OpenCode + Copilot/Kimi + Generic — what you
  can launch) vs the **documented `--version` auto-detect set** (the five: claude/codex/amp/gemini/opencode).
  So `AgentTool::builtin_defaults()` seeds **7** providers and `AgentKind::auto_detectable()` returns true for
  exactly the **5**. **Copilot/Kimi added** (per the user's "add if grounded" directive): their CLI commands
  were grounded via web search — Copilot CLI = `copilot` (npm `@github/copilot`, GA 2026-02, `--version`
  confirmed); Kimi CLI = `kimi` (MoonshotAI/kimi-cli) — so this is grounding, not fabrication (§4/§9). They are
  seeded as launchable built-in tools but stay **outside** the probe set (Solo documents probing only the
  five; we don't invent that it probes Copilot/Kimi, which also sidesteps the unconfirmed `kimi --version`).
  Generic is the closed-enum fallback, never probed. No `KNOWN-DIVERGENCES.md` entry — this matches Solo's
  documented behavior on both axes.
- **Contradiction surfaced (CLAUDE.md §12), not silently overridden.** A stray root `package-lock.json`
  (npm lockfile in this pnpm workspace) is present and untracked. I first anchored it in `.gitignore`
  (matching the existing clean-room stray anchors), then found pointer **0a** explicitly records "leave the
  stray root `package-lock.json` — do not rm/gitignore/stage" — so I **reverted** the `.gitignore` change to
  respect the prior decision. It stays **untracked, never committed**. Open question for the user: keep that
  "don't gitignore" stance, or anchor it like the other strays? (Awaiting the user's call.)
- **Not done / next:** **E4** — agent launch: `Agents::launch` (Agent-kind process via the supervisor, in
  the project dir, interactive PTY, env passthrough = **E8**) + the launch picker / "agent with flags" UI
  (via `/impeccable`; needs a Tauri command + a TS `AgentTool`/`AgentKind` mirror — confirm visual specifics
  with the user, DESIGN.md is the source of truth). Then **E5** (idle FSM sampler + activity surfacing). The
  branch is **not pushed / no PR** — awaiting the user's call (see the session summary).

### D5 restart banner — retain scrollback + draw a banner across relaunches (2026-06-21, `feat/phase-6-restart-banner`)
- **The last Phase-6 v1 build.** Branch `feat/phase-6-restart-banner` off `main` (PR #11 merged). One feature
  commit (`e75adc8`) + this `docs(progress)` commit. `just lint && just test` **green: Rust core 163 (+3) / sys
  14 / pty 10 (+soak 3 ignored) / store 13 / UI 60**; clippy `-D warnings`, fmt, tsc, ESLint, Prettier,
  dep-direction (core still framework-free), file-size guards all pass.
- **Contradiction surfaced + resolved (CLAUDE.md §1.4/§2).** `plan/02` marked **D5 `later`** while the session
  prompt + this ledger treated it as the last Phase-6 **v1** build. The user (top of §2) confirmed: build as v1
  and fix the matrix. **`plan/02` D5 `later`→`v1`.**
- **Root cause (not just "no banner").** The crash auto-restart path spawns a *fresh actor* (the prior one
  exited on the crash), whose `Terminals::open` **replaced** the channel with empty buffers **and a new live
  sender** — so the last crash output was wiped *and* the already-attached pane froze (still subscribed to the
  dropped sender; `useTerminal` attaches once and never re-attaches). The in-place restart path (same actor)
  kept the buffer but drew no banner.
- **Fix (core only — single rule, no FE/Tauri change).** `Terminals::open` now **reuses** an existing process's
  buffers + live broadcast sender on a relaunch, replacing only the input channel (whose receiver the dead actor
  owned) — output history survives and viewers keep streaming across the restart. New `Recorder::mark_restart`
  writes a banner into the buffers + live stream **iff** there is prior output (`TerminalBuffers::has_output`);
  the actor calls it **once at the top of its spawn loop**, so the same rule covers every relaunch trigger
  (crash auto-restart, file-watch restart, manual restart, user start after stop) without special-casing the
  path. Confirmed end-to-end against the Tauri side via the `tauri-calling-frontend` skill: the `pty_attach`
  forwarder drains the *reused* live sender straight to the webview `Channel`, so no re-attach and no adapter
  change is needed.
- **Banner look (user-chosen).** Dim ANSI `──────────  restarted  ──────────` in the raw stream (matches
  DESIGN.md's calm muted surface); stripped to plain `restarted` in the rendered projection MCP/logs read.
  Neutral label — the injection point in the terminal stream does not know the cause.
- **Behavior scope (user-chosen "all relaunches").** Solo documents keep-output+banner for *crash* auto-restart
  only; we apply it to every relaunch. Recorded as a gap decision in **`plan/05` §12** (Restart banner scope).
- **Tests (honest, mock-clock).** New `crates/core/src/terminal_tests.rs` (separate-file convention): banner
  only after prior output; a relaunch reuses buffers **and** the live stream (a viewer attached before the
  relaunch still receives the banner + new output — the freeze fix). New supervisor test
  `a_crash_auto_restart_keeps_the_last_output_and_marks_the_boundary` proves the crash → new-actor path retains
  run-one's output with a banner before run-two. Added `FakeSpawner::streams_then_crashes` (generalized the
  streaming fake to carry an exit status — DRY).
- **Not done (user-only):** the runtime acceptance walk via `just dev` (banner visible on `kill -9`
  auto-restart; toast; CPU/RSS; port/readiness; file-edit restart) — Phase 6 stays **Done — pending verify**
  until observed, then flips to **Verified**. PR not pushed/opened yet — awaiting the user's call (see the
  session summary).

### Review fixes on the soak PR — metrics contract single-source + CPU clamp (2026-06-21, `feat/phase-6-soak`)
- **Independent review of PR #11 (this branch), then the agreed fixes applied. `just lint && just test` green
  (UI 60 / sys 14 — +1 from a new CPU-clamp unit test); the soak leak gate was also run live (`just soak`):
  3 passed, deterministic ~3.25s, flat fd/thread/task baselines and zero leaked process groups.**
- **Metrics CPU/memory contract re-synced to its implementation (single trusted source — the headline fix).**
  The 2026-06-20 `/proc` metrics rewrite (`70b3d26`) changed the convention to **whole-machine CPU (≤100)** and
  **exact process-group membership** (PSS memory) and dropped `sysinfo`, but updated only the adapter's own doc;
  the upstream contracts still described the old **per-core / process-subtree / `sysinfo`** behaviour — two
  contradictory sources of truth, with the *contract* describing behaviour the implementation no longer had.
  Brought all of them into line so the concept is defined once: the `MetricsProbe` port contract +
  `ProcessMetrics` doc (`core/metrics/probe.rs`, incl. the double-fork-now-counted and PSS notes),
  `DomainEvent::MetricsTick` (`core/events.rs`), and the TS mirror chain (`domain.ts`, `store/signals.ts`,
  `lib/format.ts`). A stale FE test (`format.test.ts`, "keeps multi-core values above 100%") asserted an input
  the backend can no longer emit — replaced with a whole-machine near-saturation case.
- **CPU% invariant made real.** `cpu_percent` now clamps to `100.0` (`crates/sys/src/metrics.rs`), so the
  documented "never exceeds 100" holds even under tick-quantisation jitter and the `tests/metrics.rs` `<= 100`
  assertion can no longer flake. New unit test `a_reading_over_the_ceiling_is_clamped_to_one_hundred`.
- **TS `ProcessMetrics` single-sourced.** `store/signals.ts` now derives it from the `MetricsTick` payload
  (`Pick<Extract<DomainEvent, …>>`) instead of re-declaring `{ cpu_pct; rss }`, so the reading shape cannot
  drift from the event.
- **Clean-room strays gitignored (§9).** Anchored `/solo.yml`, `/crates/solo.yml`, `/processes.webp` (the Solo
  reference screenshot) so an accidental `git add .` can't commit a Solo asset; they leave the untracked list.
- **Not changed (flagged, not skipped):** the `/proc/<pid>/stat` parse duplicated in
  `crates/pty/tests/soak.rs::child_pids` vs `crates/sys/src/proc.rs` — DRYing it would expose `crates/sys`
  OS-parsing internals cross-crate for one test reading different fields; accepted as test-only duplication
  rather than worse coupling.

### Soak gate + Phase-6 UI surfacing + metrics-accuracy fix (2026-06-20, `feat/phase-6-soak`)
- **Branch.** PRs #9 (file-watch) and #10 (notifications) merged to `main` (`c1efc1b`, `89b355f`), so this
  session branched **`feat/phase-6-soak` off `main`** (per the prior pointer's rule). Three commits → one PR.
  Strays never committed: `solo.yml`, `crates/solo.yml`, `processes.webp` (clean-room).
- **Soak gate (`fe282af`).** Headless longevity tests over **real fixture processes** through the `Facade`
  (real `PtyProcessSpawner` + `TokioClock`), in `crates/pty/tests/soak.rs`, `#[ignore]`d so per-change CI skips
  them: (1) start/stop loop of 40 → identical fd / OS-thread / tokio-task counts (tokio `num_alive_tasks`, stable
  in 1.52) + zero surviving process groups; (2) crash→auto-restart storm → **exactly 10/60s** then exhausted, no
  zombies, flat RSS/fd/task; (3) metrics sampler self-restarts after a `panic_once` probe while the facade keeps
  serving. Every figure read from `/proc/self` + the live runtime (measured, not fabricated); 5× deterministic.
  Nightly CI: new **`.github/workflows/soak.yml`** (`schedule` cron `0 4 * * *` + `workflow_dispatch`,
  ubuntu-22.04, no system libs needed, **`--test-threads=1`** because each test measures the whole process's
  fd/thread/task counts) + a `just soak` recipe. Confirmed cron/schedule semantics against GitHub Actions docs
  (scheduled runs use the **default branch**, so the nightly activates once merged); used the
  `tauri-pipeline-github` skill. Added `rt-multi-thread` to `crates/pty` dev-deps (the soak runs on a realistic
  multi-thread runtime, like the app).
- **UI surfacing (`0ef1804`, via `/impeccable` + `shadcn`).** Confirmed the row layout with the user first
  (CLAUDE.md §5): **at-rest telemetry → controls on hover**, selected process's telemetry in the terminal header.
  Running rows show `:port  cpu% rss` in muted **mono** (Spent-on-Status Rule — saturated colour stays on the
  status glyph); `restarting k/N` (k/N from a mirrored `RESTART_LIMIT` const, the sanctioned cross-boundary
  mirror), `not ready` (Readiness::Waiting), `Exhausted` (already the status glyph). Telemetry is event-derived
  (`MetricsTick` + `RestartScheduled`), coalesced in a `SignalsProvider`/`useSignal` **context** read at the
  leaves (no prop-drilling through 3 pass-through components), kept **off** the read-model list projection. New:
  `lib/format.ts`, `store/signals.ts` (pure reducer) + `signalsContext.ts` + `SignalsProvider.tsx`,
  `components/sidebar/ProcessMeta.tsx`; row + header wired. **No backend change** — the events already flowed and
  the composition root already spawns the samplers. shadcn consulted: the bespoke muted-mono telemetry is
  correct per DESIGN.md (NOT a shadcn `Badge`/`Tooltip`, which the design system rejects); reused the existing
  shadcn `Button`. +14 UI tests (format, signals reducer, ProcessRow render).
- **Metrics-accuracy fix (`70b3d26`) — user-reported 550% CPU / 9 GB RSS.** Root cause (measured, not guessed):
  the user's `dev` process runs `just dev` (a full parallel Rust+Tauri build) **inside** Soloist; the `sysinfo`
  probe summed **per-process RSS** across the subtree (double-counts shared pages → tens of GB) and used the
  **per-core** CPU convention (build across N cores → N×100%). Rewrote the probe over **`/proc`** with **exact
  process-group membership** (matching the port scanner — extracted into a shared `crates/sys/src/proc.rs`):
  memory = summed **PSS** (`/proc/<pid>/smaps_rollup`, shared pages counted once; `statm` RSS fallback); CPU =
  whole-machine delta (**100% = all cores, never above**, user-chosen convention) with per-pid tick baselines so
  membership churn never spikes it. **Dropped the `sysinfo` dependency entirely** (its only user; smaller
  bundle); added `libc` for `sysconf`. Verified on an 8-core box: a 3-core busy group reads **37% / 6.8 MB**
  (was ~300%/inflated). Unit-tested the CPU normalisation; the integration test drives a real spawned process
  group and asserts a plausible PSS figure (regression guard against the gigabyte double-count).

### D8 native notifications — C7 `notify` domain + Tauri notification plugin (2026-06-20)
- **Slice 2 of this session (stacked branch).** Built **D8** (native desktop notifications on crash /
  restart-exhausted) into the **pre-existing C7 placeholder `crates/core/src/notify.rs`** — promoted to a
  `core/notify/` domain module (sibling of `metrics`/`portscan`/`filewatch`), which **owns its own driven
  port** (the "context owns its port" decision). `notifier.rs` = `Notifier` port + `Notification` +
  `NoopNotifier` (the `Notifier` stub was **moved out of `ports/mod.rs`** into the domain; `Summarizer` stays
  in `ports/mod.rs` — see the R7 open thread below). `reactor.rs` = `NotificationReactor`: subscribes to the
  bus, composes a toast for `ProcessStatusChanged{to: Crashed}` and `RestartExhausted` (resolving the label via
  the new `Supervisor::label_of`), and honors a **global on/off** (`Facade::set_notifications_enabled` /
  `notifications_enabled`, default on). Weakly held, ends when the bus closes — mirrors the other reactors.
- **Adapter = the Tauri notification plugin (user directive "use tauri skills for notification"), not
  `notify-rust`.** This lands the `Notifier` adapter in the **`crates/app` Tauri crate** (`TauriNotifier` over
  `tauri_plugin_notification::NotificationExt`) — hexagonally cleaner than `crates/sys` (a Tauri-based port
  impl belongs in the Tauri adapter; `crates/sys` stays pure-OS). Verified the plugin's Rust API against the
  official Tauri docs; invoked the `tauri-plugin-permissions` skill — **no capability added** (the ACL gates
  *webview* IPC; we call the plugin only from Rust, so least-privilege = no `notification:default`). Plugin
  registered via `.plugin(tauri_plugin_notification::init())`; `build_facade` now takes the `AppHandle` and is
  built **inside `.setup()`** (so the notifier can capture the handle), then `.notifier(TauriNotifier::new)`.
  `notifications_loop()` spawned alongside the other reactors. `plan/04` §1 port table updated to record the
  adapter change.
- **Wiring (single-source).** `notifier` added to `CorePorts` + `.notifier()` builder (Null-Object default
  `NoopNotifier`); `Facade` gains the notifier + the `AtomicBool` on/off + `notifications_loop`;
  `Supervisor::label_of`/`Registry::label_of` is the one focused label read. `RecordingNotifier` spy added to
  `core::testing`. No UI surfacing (D9 in-app toasts / D10 bell+unread stay **later**).
- **Gate green: `just lint && just test` → 234 (Rust 192 / UI 42)** — fmt, clippy `-D warnings`, tsc, ESLint,
  Prettier, **dep-direction** (core still framework-free — the `notify` *crate* lives only in the `crates/sys`
  file-watch adapter, never core) + **file-size** guards pass. +4 notify-domain mock-bus tests (crash toast,
  exhausted toast, disabled→silent, clean-stop→no toast). _Runtime "kill -9 → toast" is the user's `just dev`
  check._
- **Branching (user directive).** Slice 1 (D6/D7 live, `79de1cc`) pushed onto `feat/phase-6-file-watch`
  (**PR #9**). Slice 2 (D8) is on a **new stacked branch `feat/phase-6-notifications`** with a **stacked PR
  based on `feat/phase-6-file-watch`**. Strays never committed: `solo.yml`, `crates/solo.yml`, `processes.webp`.

### R7 open thread — driven-port ownership drift (logged for a future session)
- **The drift (user-flagged):** the "a bounded context owns its own port" rule is applied to the newer domains
  (`metrics`/`portscan`/`filewatch`/`notify` each own their port + `Noop`), but the **older driven ports still
  sit in the shared `core/ports/mod.rs` god-file** (`ProcessSpawner`/`PtyIo`/`ProcessControl` C2/C3,
  `Store`/`ProjectRepo`/`TrustRepo` C1, `LockReleaser` C6, `RuntimeState`/`OrphanControl` C2-orphans,
  `Summarizer` C4). Internal consistency drift, **not** a Solo-behavior divergence. Logged as **R7** in
  `plan/06` §7 (migrate each into its context, leaving only the cross-cutting `Clock` + the `CorePorts` bundle
  in `ports/`). Not actioned this session.

### D6/D7 file-watch went LIVE — notify OS adapter + dynamic re-watch (2026-06-20)
- **Slice 1 of this session (`79de1cc`, on `feat/phase-6-file-watch`/PR #9).** Implemented the `FileWatcher`
  port over a recursive `notify` watcher in **`crates/sys`** (`NotifyFileWatcher`, named for the capability
  like `metrics.rs`/`portscan.rs`): create/modify events forwarded as absolute paths to the core reactor on
  notify's own delivery thread (off the runtime), best-effort (an unwatchable root yields no events). `notify`
  8 added (`default-features = false`; inotify backend) + `tokio` (sync). **Closed the reactor's
  once-at-startup limitation:** it now re-syncs watch targets on every `ProjectOpened` (and on a lagged bus,
  to catch up), so a project opened after launch is watched too. Wired `build_facade .file_watcher(...)`.
  Running-only `file_restart` semantics preserved. +4 real-inotify integration tests (`crates/sys/tests/`) + 1
  reactor re-watch test (mock clock, deterministic ×5). _Runtime "edit watched file → restart" is the user's
  `just dev` check._

### Review of the D6/D7 file-watch core-policy slice — fixes applied (2026-06-20)
- **Independent review of the file-watch core-policy work, then every finding fixed; gate now 225
  (Rust 183 / UI 42)** (+1 reactor test). `just lint && just test` green; reactor tests 8×
  deterministic; dep-direction + file-size guards pass; `Cargo.lock` brotli/alloc pins unchanged.
- **File-watch now reloads a *running* command only (behavioral gap closed).** `Supervisor::restart`
  starts a stopped process, and `file_restart` called it unconditionally — so once the `notify` adapter
  lands, a watched change could **start a command the user stopped**, and on launch could start
  restored-but-resting commands (contradicting "restore never starts a process"). `file_restart` now
  no-ops unless the command `is_active()`, still delegating to `restart` for the cycle (trust gate +
  crash-reset reused — no reimplementation; the running-only decision lives in the one C2 method). New
  test `a_change_to_a_stopped_command_does_not_start_it`; the running-path reactor tests now start the
  process first so a "no restart" is attributable to the policy, not an inactive command. Gap recorded
  in `plan/05` §12 ("File-watch on a non-running command").
- **Comment discipline (CLAUDE.md §8).** Removed the lone `plan/05 §4` source citation (the only one in
  the source tree) and the `(D6/D7)`/`(D6)`/`(D7)` parity-row IDs from the filewatch doc comments —
  describe the behavior, not the plan rows. The C1–C8 **bounded-context** IDs are kept (glossary §71
  vocabulary, not phase/task numbers — user-confirmed this session).
- **Composition-root ordering.** `Facade::file_watch_loop()` is now spawned **after** `restore_projects()`
  in the setup hook, so the reactor's one-shot watch-target read sees the restored commands (moot under
  the Noop watcher; correct for when the adapter lands — dynamic re-watch on `ProjectOpened` is still the
  adapter session's job).
- **Reactor shutdown clarified, not changed.** The bus subscription is purely a shutdown sentinel
  (mirrors the self-healing reactor) — kept that idiom rather than introducing a divergent
  CancellationToken (avoids a second shutdown mechanism). The `supervisor/monitoring.rs` module doc was
  corrected: the file-watch reactor lives in `core/filewatch`, not "the monitoring domain"; only its C2
  accessors (`watch_targets`/`file_restart`) live in that file.

### D6/D7 file-watch restart — CORE POLICY ONLY (no OS adapter) (2026-06-20)
- **Scope (per the session task):** the pure, headless-testable file-watch-restart policy. The real
  `notify` adapter is a **separate next session** — this session uses a Fake watcher + the mock clock only.
  **Baseline confirmed green first: 213 (Rust 171 / UI 42); end 224 (Rust 182 / UI 42)** (+11 Rust).
  `just lint && just test` green; new mock-clock tests **10× deterministic** (no flakes). Branch
  **`feat/phase-6-file-watch`** off `main` (PR #8 merged into `main` as `6c76d18` before this session) →
  **new PR** (user directive this session). `globset` API confirmed via context7 (`*` crosses separators
  is the default `Glob` behaviour). Tauri `tauri-app-resources` skill consulted before the composition-root
  spawn.
- **New C5 `core/filewatch/` domain (mirrors `core/metrics`/`core/portscan`; a bounded context owns its
  own port).** `watcher.rs` = the `FileWatcher` port + `WatchHandle` (RAII: drop = stop watch) +
  `NoopFileWatcher`/`NoopWatchHandle` — **the `FileWatcher` stub was moved out of `ports/mod.rs`** into the
  domain (the recorded "a context owns its own port" decision; `Notifier`/`Summarizer` stubs stay in
  `ports/mod.rs`). `policy.rs` = the **pure** matcher (`globset`, relative to root, `*` crosses separators,
  **D7 default ignores** checked before the glob) + `compile` (empty/all-invalid globs → no watch).
  `reactor.rs` = the `Clock`-driven `WatchReactor`: consumes change events from the port, matches, **reuses
  `core/debounce::Debouncer`** (its first real consumer — added `Debouncer::due_at` so the reactor sleeps to
  the exact deadline, deterministic on the mock clock) to coalesce a save burst, and routes to the new
  `Supervisor::file_restart`.
- **Reuse, not reimplementation (per the task).** `Supervisor::file_restart` (in `supervisor/monitoring.rs`,
  the C2↔C5 surface) **delegates to the existing `Supervisor::restart`** and only publishes
  `DomainEvent::FileRestart` on success — so the trust gate (untrusted → fail-closed, no restart) and the
  crash-tracking reset come for free (a file-restart resets crash tracking like a user restart, independent
  of the 10/60s window). Eligibility = command-only + non-empty globs (via `Registry::watch_commands` →
  `Supervisor::watch_targets`), trusted-only is enforced at restart.
- **Single source threaded, not duplicated.** `restart_when_changed` flows `ProcessSpec` (already existed) →
  `Registration::{command,launched}` → `Registry`/`Managed` → `watch_commands()`/`watch_targets()`. New
  `DomainEvent::FileRestart` mirrored in `domain.ts` + the exhaustive `projection.ts` switch (no UI yet —
  Task 9 UI deferred, as instructed). `globset` added to `crates/core` (pure matching like `sha2`/`vte`;
  **dep-direction guard still green — core stays framework-free**; brotli/alloc `Cargo.lock` pins unchanged).
- **Composition (§16 Null-Object pattern).** `FileWatcher` wired into `CorePorts` with a `NoopFileWatcher`
  default + a `.file_watcher(...)` builder; `Facade::file_watch_loop()` added and **spawned in the
  composition root** alongside the other loops — **inert under the Noop default** (watches nothing) until
  next session swaps in the real `notify` adapter. `build_facade` is unchanged (keeps the default).
- **Divergence recorded:** `KNOWN-DIVERGENCES.md` **D-4** — the D7 default-ignore list
  (`.git`/`node_modules`/`target`/`dist`/`.venv`) is our gap-filling decision (`plan/05` §4 notes Solo's
  ignore list is undocumented); ignored paths never restart even if a glob would match.
- **Known limitation (next session):** the reactor establishes watches **once at startup** from the current
  watch targets; commands registered later (a project opened after launch) are not yet watched — dynamic
  re-watch on `ProjectOpened` lands with the `notify` adapter. With the Noop watcher this is moot.
- **Not done / next:** the **`notify` OS adapter** in `crates/sys` (implements `FileWatcher` over a recursive
  `notify` watcher off the runtime; wires D6/D7 **live** + dynamic re-watch; `build_facade` gains
  `.file_watcher(...)`), then **D8 notifications** (`Notifier` + `notify-rust`), the nightly soak gate, and
  the UI surfacing (CPU%/RSS, ports, restart/exhausted/not-ready/file-restart badges, Task 5/9 via
  `/impeccable`). **Next session should start with: the `notify` file-watch OS adapter in `crates/sys`.**
  Strays left untracked, **never committed**: `solo.yml`, `crates/solo.yml`, `processes.webp` (clean-room).

### Adversarial review of the OS-probe slice — fixes applied (2026-06-20)
- **Independent skeptical review of PR #8 (D1/D2/D3), then every finding fixed.** Gate **213 (Rust 171 /
  UI 42)**; `just lint && just test` green; monitoring mock-clock tests **40× deterministic**, dep-direction
  + file-size guards pass; `sysinfo` `memory()`=bytes and the brotli/alloc lock pins confirmed unchanged.
- **Read-model race closed (was the top bug).** The port scanner read `live_groups()`, did a slow OS read
  with no lock held, then wrote ports back — so a process that stopped mid-scan could have stale ports
  (and a spurious `PortsChanged`) resurrected on it, never cleared. `record_ports`/`set_ready` now thread
  the scanned **pgid**; `registry.set_ports`/`set_ready` apply **only while `entry.pgid == Some(pgid)`** under
  the one lock, so a late reading on an ended group is dropped. Same guard covers the readiness waiter. New
  test `a_monitoring_update_after_the_group_ends_is_dropped`.
- **OS reads moved off the runtime (CLAUDE.md §6/§8).** Both samplers + the waiter's poll now run the
  blocking `/proc`/`sysinfo` sweep via a new `supervision::run_blocking` (spawn_blocking + `resume_unwind`,
  so a probe panic still trips the supervised loop's panic-isolation and restarts it).
- **Exact process-group membership.** The `/proc` port probe now matches by **process-group id**
  (`/proc/<pid>/stat` pgrp) instead of a parent-subtree walk — simpler *and* catches a reparented
  (double-forked) descendant the subtree would miss. `sysinfo` metrics keep the subtree (the OS view doesn't
  expose the group there) with the doc softened to say so. The two probe-contract docs cross-reference their
  omit-dead vs keep-empty asymmetry.
- **Readiness is a closed enum** (`Readiness { Ungated, Waiting, Ready }`) replacing the `Option<bool>`
  tri-state, mirrored in `domain.ts` (the event stays `ready: bool` per the phase spec). Supervisor
  read-model accessors split into `supervisor/monitoring.rs` (supervisor.rs back under the 400-line smell).
- **Comment discipline:** removed 5 source citations the slice had introduced (`plan/04 §6`, `plan/05 §7`,
  `Phase 8`, `K4 precursor`) + a pre-existing `plan/05` citation in `ProjectGroup.tsx` (CLAUDE.md §8).

### OS-probe slice — D1 per-process CPU/mem + D2 port discovery (2026-06-20)
- **Scope:** the monitoring OS-probe slice. **Two gated commits, each start- and end-green** (`just lint &&
  just test`). Baseline confirmed **194 (Rust 154 / UI 40)** first; end **207 (Rust 166 / UI 41)**.
  Branch **`feat/phase-6-monitoring`** (cherry-picked from `main` after PR #7 merged — see below); commits
  **`e0fa32e` (D1)**, **`be1711a` (D2)**. **`crates/sys` created** this slice (the recorded user decision:
  no empty scaffolding earlier). Tauri `tauri-calling-frontend` consulted before the app event wiring;
  `sysinfo` API confirmed via context7 (0.33.1, `ProcessesToUpdate`/`ProcessRefreshKind::nothing().with_cpu()`).
- **D1 (matrix D1, v1 — `e0fa32e`):** per-process CPU% + RSS, aggregated over the process **group** (matrix
  D12, per-child breakdown, stays `later`). New **C5 metrics domain** `core/metrics/` (`probe.rs` =
  `MetricsProbe` + `ProcessMetrics` + `NoopMetricsProbe`; `sampler.rs` = `MetricsSampler`). Self-supervised,
  `Clock`-driven (~1 s), publishes `DomainEvent::MetricsTick`. Registry tracks each running group's leader
  pgid; `Supervisor::live_groups()`; `Facade::metrics_sampler_loop()` orchestrates C5 over C2 (C8, no context
  cycle). `crates/sys` `SysinfoMetricsProbe` over `sysinfo` (`default-features=false, features=["system"]` for
  size), subtree-by-parent aggregation, **per-core CPU%** (htop convention — documented; flip to total-machine
  if preferred). **Verify:** mock-clock + `FakeMetricsProbe` headless incl. **sampler self-restarts after a
  panic** (K4 precursor); real-`sysinfo` integration test (`crates/sys/tests/metrics.rs`) reads a live process
  and omits a dead group. Runtime "busy `yes` shows moving CPU/idle ~0" is the user's `just dev` check.
- **D2 (matrix D2, v1 — `be1711a`):** TCP port discovery on `ProcessView.ports`. New **C5 portscan domain**
  `core/portscan/` (`probe.rs` = `PortProbe` + `NoopPortProbe`; `scanner.rs` = `PortScanner`). The scanner
  (self-supervised, ~2 s) discovers each running group's listening ports, reflects them on `ProcessView.ports`,
  and emits `DomainEvent::PortsChanged` only on a real change (dedup); ports clear when the group ends.
  `Supervisor::record_ports` is the single mutation point. `crates/sys` `ProcPortProbe` reads `/proc` once per
  tick: process subtree (`/proc/<pid>/stat` ppid) → socket inodes (`/proc/<pid>/fd`) → `/proc/net/tcp{,6}`
  LISTEN entries; batched across groups. **Verify:** mock-clock scanner tests (discover-then-announce-once
  dedup; clear-on-stop); real-`/proc` integration test (`crates/sys/tests/portscan.rs`) **discovers a port the
  test process is actually listening on**. Runtime `python -m http.server` check is the user's.
- **Self-supervision extracted (DRY):** `core/supervision.rs::supervise()` runs a restartable loop under a
  panic-isolation boundary with `Clock`-driven exponential backoff; the metrics sampler and port scanner both
  use it instead of each owning the wrapper. Tested directly (`supervision_tests.rs`).
- **Architecture decisions this session (user directive — top source of truth §2; supersede prior docs):**
  1. **A bounded context owns its own port.** The metrics/portscan ports + data types live *in their domain
     module* (`core/metrics/probe.rs`, `core/portscan/probe.rs`), **not** in the shared `core/ports/mod.rs`.
     `CorePorts` imports each domain's port. Rationale: adding a new metric/probe is a change confined to its
     domain, never to a shared god-file. (The older driven ports — `LockReleaser`/`RuntimeState`/… — still sit
     in `ports/mod.rs`; migrating them is optional future cleanup, not required.)
  2. **Tests live in their own files**, not merged with the implementation (`#[cfg(test)] #[path =
     "x_tests.rs"] mod tests;` for private-item unit tests; `tests/` for adapter integration). This
     **reverses** the prior "tests stay inline" project decision (was CLAUDE.md §16 / `plan/06` §6 / this
     ledger). Applied to all new code this slice; existing inline tests are migrated opportunistically, not in
     a big-bang. Docs updated to match (see below).
  3. **Small single-purpose files**; design patterns where the trigger fires (Ports-&-Adapters with the
     domain-owned port; Null Object for the `Noop*` defaults; self-supervised reactor for the samplers).
- **Docs updated to match the decisions:** `ARCHITECTURE.md` (crate table adds `crates/sys`; tests-separated +
  domain-owned-port notes), `plan/06` §5.2 (port in its domain) + the inline-tests line, `CLAUDE.md` §15/§16
  (tests-separated). `plan/02` D1/D2 stay v1; D12 stays `later`.
- **Branch / PR (user directive this session):** the restart-policy work merged as **PR #7** before this slice,
  so D1/D2 were re-based onto `main` as **`feat/phase-6-monitoring`** and a fresh PR opened (see the PR link in
  the session summary). Strays left untracked, **never committed**: `solo.yml`, `crates/solo.yml`,
  `processes.webp` (Solo reference screenshot — clean-room).
- **D3 readiness DONE this slice (`4b4d930`):** `Facade::wait_for_port(id, port, timeout)` lives in the
  portscan domain (`waiter.rs`), reusing the `PortProbe`: it polls on the `Clock` until the port binds or
  times out, re-resolving the group each poll (a process that restarts mid-wait is probed on its new group;
  one that stops fails fast `NotRunning`). Readiness is a **dimension, not a `ProcStatus`** — `ProcessView.ready:
  Option<bool>` (None = no gate / Some(false) = Running-but-not-Ready / Some(true) = bound) + `ReadyStateChanged`;
  `Supervisor::set_ready` is the single mutation point and emits; `set_pgid(None)` clears it on stop. **No new
  port** (reuses `PortProbe`). The **production caller is the Phase-8 MCP `wait_for_bound_port` tool** — until
  then the capability + read-model surface are built and tested (mock-clock waiter tests: already-bound,
  late-bind, timeout, not-running), not yet driven in the GUI. Shared `portscan/test_support.rs` extracted so
  scanner + waiter tests don't duplicate setup (DRY); `FakePortProbe` made mutable for the late-bind test.
- **Not done / next:** D6/D7 file-watch (flesh out the `FileWatcher` port + a `notify` adapter, debounced,
  trusted-only, default ignores), D8 notifications (`Notifier` + `notify-rust`), the nightly soak gate, and the
  UI surfacing of CPU%/RSS + ports + the "restarting (k/N)"/RestartExhausted/not-ready badges (phase Task 5/9,
  via `/impeccable`). **Next session should start with: D6/D7 file-watch restarts.**

### Phase 6 begun — crash auto-restart policy (D4 + D11), the self-healing slice (2026-06-20)
- **Scope (user-chosen):** the **restart-policy slice first** — pure core, mock-clock-tested, **zero new
  deps/crates**, one gated commit. Baseline confirmed green first (**186 = Rust 146 / UI 40**); end
  **193 = Rust 153 / UI 40** (+7 Rust). Commit `90d51ac`. Tauri skill `tauri-calling-frontend` consulted
  before the one-line app wiring (new events flow through the existing `forward_events` emit bridge).
- **Architecture (user mandate: single trusted source, separate domain/module, no scatter, work on what's
  already defined).** The restart policy is **one cohesive C2 module** — `crates/core/src/supervisor/restart.rs`
  (plan/04 §3: "C2 owns restart policy"). It holds the **pure** `RestartWindow` (a sliding-window rate
  limiter driven by `Clock`-sourced instants, mirroring `Debouncer`), the shared `RestartPolicy`
  (per-process windows + a shutdown latch), and the `Supervisor` glue + the **reactor** (a thin event pump).
  - **Reuse, not duplication:** the restart *effect* calls the supervisor's existing `launch_actor`
    primitive (the one place a process is spawned) and the existing **trust gate**; the *eligibility* re-checks
    durable trust (untrusted never auto-restarts, fail-closed). **No** re-implemented spawn/trust logic.
  - **Worked on already-defined behavior:** threaded the existing `ProcessSpec.auto_restart` (single source)
    through `Registration` → `Registry`/`EntryInfo`; added the missing FSM edge `Crashed → RestartExhausted`
    to the existing `ProcStatus` contract; **closed B7's deferred "clears crash tracking" half** (a user
    stop/clean-exit/removal forgets the window; a user start/restart resets it).
  - **Reactor ownership (no leak):** the reactor holds a **`Weak<Supervisor>`** + a bus receiver, so it
    terminates when the facade drops instead of forming a keep-alive cycle (the bus's last `Sender` would
    otherwise never close). The composition root spawns the loop once via `tauri::async_runtime::spawn` in
    `.setup()`; `Facade` now holds `Arc<Supervisor>` and exposes `self_healing_loop()`.
  - **D11:** `Supervisor::shutdown` latches the policy closed first, so a crash during teardown is never
    auto-restarted. **D4:** 10 restarts in a 60 s sliding window → `RestartExhausted` + a `RestartExhausted`
    event (no hot-loop, no backoff — matching the documented gate).
- **Tests (honest, inline, shared fakes):** pure-window tests (restart-up-to-the-max-then-exhaust, age-out,
  forget-clears) in `restart.rs`; reactor end-to-end (`a_crashing_command_is_restarted_until_the_limit_then_exhausted`
  proves *exactly 10 then exhausted* on the mock clock), `shutdown_disables_auto_restart`,
  `an_untrusted_or_non_auto_restart_command_is_not_restarted`; the FSM-edge test in `process.rs`. Reused the
  supervisor harness (`Harness.sup` is now `Arc<Supervisor>`) + a single-source `auto_restart_spec` fixture.
- **Frontend single-source mirror:** the two new `DomainEvent` variants added to `domain.ts` and handled in
  `projection.ts`'s exhaustive switch (non-list-changing, like `TerminalBell` — the status delta already
  arrives via `ProcessStatusChanged`; the discrete events are the future notification/badge signals).
- **Crate placement decision (user-approved, for the *next* steps):** the OS-facing driven adapters (metrics
  probe `sysinfo`, port probe `/proc`, file watcher `notify`, notifier `notify-rust`) will land in a new
  **`crates/sys`** adapter — **not created this slice** (the restart policy is OS-agnostic core; an empty
  crate now would be dead scaffolding). It is created when step 2 (metrics) starts.
- **Not done (carried):** the OS-adapter steps D1/D2/D3/D6/D7/D8 + the nightly soak gate; runtime
  verification of auto-restart in the GUI (user, `just dev`). Strays left as-is (`solo.yml`, `crates/solo.yml`,
  `processes.webp` — clean-room: do **not** commit `processes.webp`).

### Adversarial review of the restart-policy slice — fixes applied (2026-06-20)
- **Two concurrency edges + two nits found and fixed; gate now `194 (Rust 154 / UI 40)`.**
- **Exhaust transition made atomic (race fix).** The exhaust path read the current status then
  transitioned non-atomically, so a user restart landing in that gap could be clobbered into
  `RestartExhausted` and fire a spurious "exhausted" notification. Replaced with a new
  `Registry::exhaust_if_crashed` that checks-and-transitions under one lock (mirrors `begin_launch`);
  only a still-`Crashed` process is held, and the `RestartExhausted` event fires exactly once on the
  real transition. New guarding test `exhaust_holds_only_a_crashed_process` (registry).
- **Shutdown reap is now a bounded loop (D11 race fix).** Under the multi-threaded Tauri runtime a
  crash whose auto-restart check slipped in just before the shutdown latch could spawn one last actor
  the single-pass reap missed (a potential orphan-on-quit). `shutdown` now reaps in passes until none
  remain; the latch caps new launches to a finite in-flight set, so it converges.
- **Bounded-state nit.** The exhaust path now `forget`s the window (a held-exhausted process keeps no
  lingering crash history); the `RestartPolicy` doc comment corrected to match (it had claimed an
  eviction path — `ProcessRemoved` — that is never emitted in v1).
- **Comment-discipline nit.** Dropped the `(D11)` matrix-row citation from the `shutdown` comment
  (CLAUDE.md §8 — it was the only such citation in `crates/`).
- **Verified:** `just lint` green (clippy `-D warnings`, dep-direction, file-size); `just test` green
  at **194**; the reactor + supervisor tests run 25× deterministically.

### Projects consolidated into a single trusted domain/module — backend + frontend (2026-06-20, later)
- **Why (user directive, top source of truth §2):** "fully refactor until we have a single trusted source
  'Projects' domain/module … project consumers are not going to define how projects should work. They are
  just consuming from projects domain." And: the icon must not be separate functionality — "name, icon, …
  should be DTO-like. No separate." Diagnosis (verified by reading, not assumed): the project **lifecycle**
  (open/restore) lived in `Facade`; the icon **policy** (allow-list + cap) in the Tauri adapter; the
  project↔process **join + visibility**, the **monogram**, and the **collapse-key** formats in the generic
  grouping module and the components; and the icon was fetched by a **second** IPC call (`project_icon`) + a
  `useProjectIcon` hook — consumers were defining how projects work.
- **Backend — one `core/projects/` module owns everything project (C1).** Split `projects.rs` into
  `projects/{registry,view,service}.rs` + `mod.rs`: `registry` (`Projects` over `ProjectRepo`), `view`
  (`ProjectView` — the display read-model), **`service` (`ProjectService` — the open/restore lifecycle +
  `ProjectLoad`/`LoadProjectError`, moved out of `Facade`)**. `Facade::load_project`/`restore_projects` are
  now 1-line delegations to a `ProjectService` it assembles from the contexts it owns; the Facade defines
  nothing about how a project opens.
- **Icon is resolved exactly like the name — a plain field of the read-model, no separate anything**
  (second user pass: "the icon is still separate … it's the same as the project name"). `ProjectView`
  carries `name: String` and `icon: Option<String>`, **both resolved in one place, `ProjectView::from_record`
  (`view.rs`)**: `display_name(record)` for the name, `render_icon(record)` for the icon (resolve the
  `solo.yml icon:` path → allow-list + size-cap → `data:` URL). `project_list` returns plain
  `Vec<ProjectView>` — there is **no** `WireProject` DTO, **no** `read_icon_data_url`/`icon_mime` adapter
  helper, **no** `core/projects/icon.rs`, **no** `project_icon` command, **no** `useProjectIcon` hook. The
  webview renders `project.icon` directly, just like `project.name`. **`base64` moved app → core** (a pure
  algorithm, like the existing `sha2`; dep-direction guard still green — core is framework-free). A live
  open arrives as a slimmed **`ProjectOpened { id }`** event (no display state on the event), which the
  store treats as a trigger to re-read the snapshot (the `mergeProject` delta-fold is gone).
- **Frontend — one `store/projects/` module** (`{useProjects, tree, view, index}.ts`): the store
  (read-model + open + notice), the project↔process **tree** projection (`groupByProject`/`runningCount`/
  `ProjectTree`), and the **view helpers** consumers reuse (`monogram`, `projectCollapseKey`,
  `kindCollapseKey`). `store/grouping.ts` keeps only process-kind grouping; `Sidebar`/`ProjectGroup`/`App`
  import from `@/store/projects` and only render. Added `isRunning` to `lib/status.ts` (kills the
  `"Running"` magic string in the running count).
- **Behavior change (user-directed): the sidebar now shows an opened project even with zero processes** (an
  empty node, "No commands yet"), so the user always sees what they opened. `groupByProject` no longer
  drops process-less projects; the test asserts the empty node. plan/05 §286 documents the grouped tree but
  not empty-project visibility, so this is a UI decision, not a Solo-behavior divergence.
- **Gate green: `just lint && just test` → 186 (Rust 146 / UI 40)** — fmt, clippy `-D warnings`, tsc,
  ESLint, Prettier, **dep-direction** (core framework-free *with* `base64`, like `sha2`) + **file-size**
  guards all pass. From the pre-refactor 186 (Rust 145 / UI 41): UI −1 (2 `mergeProject` fold tests → 1
  refetch-on-open test); Rust +1 (the icon-policy test folded into `view.rs`, which gained icon
  render/skip/oversize tests). **Honest test note:** the new `useProjects` refetch test surfaced — and now
  guards against — a re-subscribe churn when the caller passes an *unstable* error callback; production
  passes a stable `store.reportError` (a `useCallback`), like `useProcesses`.
- **Not done this session (the user's to verify, `just dev` restart):** on launch the sidebar shows opened
  projects (resting); opening a folder with a `solo.yml icon:` shows the icon rendered in-DTO; an opened
  folder with no commands shows an empty project node. Stray untracked `solo.yml` (root + `crates/`) and
  `processes.webp` (Solo reference screenshot — clean-room: do **not** commit) left in place.

### Projects became a first-class feature — project-grouped sidebar + read-model + restore (2026-06-20)
- **Why:** the user opened a folder, got a `solo.yml`, but **saw no project** in the sidebar. Root cause
  (traced, not assumed): the sidebar grouped only by **process kind** (Agents/Terminals/Commands) with **no
  project tier**, and `load_project` **dropped the `solo.yml` `name:`** (`projects.add(root, None, None)`),
  so there was no project identity to show. The pipeline (detect → register → `ProcessSpawned` → render)
  was sound — the gap was structural/presentational. Fixed end to end.
- **Core (C1) — project read-model, single-sourced.** `ProjectView { id, name, root, icon }` projects the
  durable `ProjectRecord` (name = `solo.yml name:` → folder fallback; icon resolved against root); projects
  stay **durable in SQLite** (no in-memory project state — corrected a first-draft design after the user
  flagged "we have sqlite"). `Projects::views()`, `Facade::projects_snapshot()` (CQRS query), and a new
  `DomainEvent::ProjectOpened` (delta) added; `load_project` now **persists the resolved name/icon** and
  announces the open. Commits `9b38a0f` (read-model + name), `ea69a73` (icon path).
- **A13 (project icon) pulled into v1 (user directive 2026-06-20).** `project_icon` Tauri command reads a
  project's icon into a capped (512 KiB), image-extension-only `data:` URL the avatar renders; monogram
  fallback otherwise. CSP already allows `img-src data:`; no asset-protocol widening (least-privilege).
  Commit `8252b1c`. `base64` (already transitive) declared directly — `Cargo.lock` +1 line, brotli pins
  untouched. plan/02 A13 → **v1**.
- **Session restore on launch (register-only).** The app re-registers every durable project's commands on
  startup so the sidebar **shows your projects across runs**, but **resting** — `Facade::restore_projects`
  shares `load_project`'s register path (`open_and_register`) and **skips `start_all`**, so launching never
  spawns a process. Fixes "absolutely nothing in the sidebar" on launch. Commit `caa8b35`. (Auto-start-on-
  launch deliberately **not** done — safe default; offer it as a follow-up if the user wants Solo-style resume.)
- **UI (via `/impeccable` + shadcn + tauri skills).** Project-grouped sidebar: each opened project is a
  collapsible node (Avatar monogram/icon + Title-type name + `running/total` count in mono + **per-project**
  bulk controls) over its **non-empty** kind subgroups (empty Agents/Terminals hidden — kills the prior
  noise). `groupByProject` **omits process-less projects** (so a stale durable project never shows as an
  empty node). Bulk moved from the global toolbar into each project header, scoped by id — **fixes the
  `processes[0].project` bug** (tracked review finding #1). New: `Avatar` primitive (radix-ui), `useProjectIcon`,
  per-project+kind collapse state. Commit `6ababf1`. Drove the design through `/impeccable craft` (shape brief
  confirmed by the user) against `DESIGN.md`; reused `Button`/`Collapsible`/`ProcessControls`/`ProcessRow`.
- **Gate green: `just lint && just test` → 186 (Rust 145 / UI 41).** clippy `-D warnings`, rustfmt, tsc,
  ESLint, Prettier, dep-direction + file-size guards all pass. New honest tests: core (ProjectView name/icon
  resolution, `load_project` persists name + emits `ProjectOpened`, `projects_snapshot`, restore-without-start),
  app (`icon_mime` allow-list), UI (`groupByProject`, `runningCount`, `mergeProject`, project-tier render).
- **Skills used (CLAUDE.md §5):** `tauri-calling-rust` (the `project_list`/`project_icon` commands),
  `shadcn` (Avatar composition, reuse primitives, `cn()`/semantic tokens), `/impeccable craft` (the sidebar
  design against `DESIGN.md`).
- **Open / not done this session:** **runtime verification is the user's** (a `just dev` restart so the
  Rust restore rebuilds): on launch the sidebar should now show opened projects (resting); opening a folder
  with a `solo.yml icon:` should show the icon. Stray untracked `solo.yml` (root + `crates/`) and
  `processes.webp` (a Solo reference screenshot — clean-room: do **not** commit) left in place. **A13 icon
  rendering not yet observed at runtime.** Plan file: `~/.claude/plans/jaunty-sauteeing-giraffe.md`.

### A10 command auto-detection BUILT (v1) + deferred review finished — fourth session (2026-06-19)
- **Scope:** built A10 (the immediate next work), then finished the STEP-4 adversarial review of the
  Phase-5 follow-up. Gated, one-concern commits; `just lint && just test` green at the start of and after
  every commit. **Baseline confirmed first:** 134 (Rust 104 / UI 30). **End: 174 (Rust 138 / UI 36).**
  Stray root `package-lock.json` left untouched; no `cargo update`; `Cargo.lock` unchanged (detection uses
  the existing `serde_norway`/`indexmap`; no new deps).
- **A10 architecture (user mandate: "single trusted source, no duplicates, no scattered code, keep
  architecture, discipline, clear separation").** A dedicated detection + writer domain in **C1**
  (`core/config/`), **Registry/Strategy**: a `Detector` trait with **one file per ecosystem**
  (`detect/{npm,cargo,go,procfile,make,just,compose}.rs`) registered once in `detect::DETECTORS`; adding
  an ecosystem is one file + one line, no giant `match`. Detectors are **pure** over a `FileSource`
  (`read(rel)`); `detect_in(root)` is the thin `std::fs` shell. Detection emits the core's **own**
  `SoloYml`/`ProcessSpec` (no parallel representation). The **writer** single-sources the file through the
  model: `SoloYml`/`ProcessSpec` gained `Serialize` + `skip_serializing_if` so defaults are omitted;
  `write::render` serializes via `serde_norway` + a hand-written plain-language header; `create_if_absent`
  is the thin shell (atomic `O_EXCL` — never rewrites an existing file). `Facade::load_project` calls
  `create_if_absent` when absent; `ProjectLoad` gained `created`, flowing once core → `project_load` →
  `api.ts` → `useProjects`. The friendly copy lives in **one** `noticeFor` helper (presentation), derived
  from the facts (`created`, count). Per plan/05 §9: dev/start/serve → `auto_start`, build/test offered
  unchecked; detected commands register **trust-gated** (auto-create never bypasses the gate — asserted).
- **A10 commits (gated):** `deee0bb` docs (plan/02 A10→v1, plan/05 §9 cross-ref) · `968adf2` detect
  scaffolding + npm · `81383c6` cargo/go/Procfile/Make/Just/Compose detectors · `551e40b` writer
  (Serialize + create_if_absent) · `f2b3a06` `load_project` wiring (`created`) · `28ccac2` UI friendly
  confirmation · `6ae1979` docs (full `solo.yml` reference in README + ARCHITECTURE/plan/06 Registry/
  Strategy rows now name `config::detect` as the first concrete use). New honest tests: 26 core detect +
  6 writer + 2 facade (Rust 104→138) and 6 UI (useProjects copy cases; UI 30→32).
- **STEP-4 adversarial review (REVIEW-PROMPT) of `d497241`+`45461d0`+`72b526e` — FINISHED.** Independent
  skeptical pass + personal verification.
  - **Re-verified sound (security):** the trust gate — `start`/`restart`/`start_all` all consult the
    **durable** `trust.is_trusted(...)?` (`supervisor.rs:160/186/268`, `bulk.rs:25`), **fail-closed** on a
    store error, never the cosmetic `ProcessView.requires_trust` flag — **A6 cannot be bypassed**. Dialog
    capability is least-privilege (`capabilities/default.json`: `dialog:allow-open`, not `dialog:default`);
    only `tauri_plugin_dialog::init()` is registered (`lib.rs:99`); **no `fs:` permission, no fs plugin** —
    `tauri-plugin-fs` is transitive-only, unreachable from the webview.
  - **Fixed (2 commits):** `b637b50` — `useTrust.trust`/`trustAll` mutated the review **synchronously**
    before `configTrust` resolved (fail-open UX: on a failed grant the command vanished / the dialog
    closed though trust never applied); now updated only in the `.then()`, with a new `useTrust.test.ts`
    (success-drops / failure-keeps / trustAll-after-all) (UI 32→36). `8f8c524` — `create_if_absent` was
    `exists()` + `fs::write` (TOCTOU); switched to atomic `OpenOptions::create_new` (`O_EXCL`).
  - **Rejected (with evidence):** the reviewer's "duplicate event-listener re-subscribe gap" — `fail` is
    `useCallback(..., [])` (`useProcesses.ts:40`), a **stable** identity, so `useTrust`'s subscribe effect
    never churns. The "noticeFor vs EmptyState duplication" nit — different concepts (post-open notice vs
    pre-open resting copy), not a real DRY breach.
  - **Recorded, not fixed (tracked below — pre-existing and/or out of A10's scope):** (1) `useProcesses`
    `projectId = processes[0]?.project` over an unordered `HashMap` snapshot — correct for the single
    loaded project (the only v1 flow), wrong only with multiple projects (→ Phase 11 project-switch);
    pre-existing (`f2642a0`). (2) `load_project` does blocking fs (`canonicalize`/read/write) on the async
    command thread — negligible on local fs, but should move off-thread per §8 (needs care: it also spawns
    actors). (3) trusting clears `requires_trust` via `refresh()` with no `ProcessTrusted` event — a
    `ProcessStatusChanged` arriving before the snapshot can briefly show stale trust; the clean fix is a
    `ProcessTrusted` `DomainEvent` (§5.6). (4) `project_load` doesn't validate the path string (trusted
    webview; the trust gate still blocks execution). (5) `auto_start_candidates` filters `Stopped` only,
    excluding `Crashed`/`RestartExhausted` (ties into the start-all-vs-start-auto open thread).
- **Stray root `solo.yml` (0-byte, untracked) is GONE.** It was present at session start (`git status`
  showed `?? solo.yml`); it is now absent. **Not removed by me** — no command this session targets the
  repo root (all detect/write tests use tempdirs). Cause undetermined; 0 bytes + untracked → nothing of
  value lost. **Not recreated** (per "surface, don't act unilaterally"). Root `package-lock.json` left
  untouched as instructed.


- **Bug fixed + committed (`72b526e` `fix: report an empty project load instead of doing nothing`).**
  Reported symptom: "selecting a project produces no UI change." Root cause (traced from code + the
  decisive fact that **no `solo.yml` exists anywhere to pick**): `Facade::load_project` → `config.open` →
  `load_or_empty` treats a missing/empty `solo.yml` as a **valid empty config** (plan/05 §3), so it
  registers zero processes, emits zero events, and returns `Ok` — the screen is unchanged and silent. Not
  a wiring bug: `register` emits `ProcessSpawned` (`supervisor.rs:121`), `forward_events` bridges the bus
  to `domain-event`, and `api.ts` `listen("domain-event")` mirrors it; the event path is proven by
  `load_project_starts_a_trusted_auto_start_command` (subscribes, receives `Running`). **Fix:**
  `Facade::load_project` now returns **`ProjectLoad { id, processes }`** (the declared-process count);
  `project_load` relays it; **`useProjects` shows an in-flow `EmptyState` notice** (naming the folder) when
  the count is zero — informational, NOT the red error banner and NOT a modal (an empty `solo.yml` is
  valid). New honest tests (fail without the fix): core `load_project_reports_the_process_count`
  (empty dir → 0, two commands → 2); UI `surfaces a notice when the folder declares no processes`. Gate
  **green before and after: 134 (Rust 104 / UI 30)**. Files: `core/facade.rs`, `core/lib.rs`,
  `app/src/commands.rs`, `ui/{domain.ts,api.ts,store/useProjects.ts,store/useProjects.test.ts,
  components/EmptyState.tsx,App.tsx}`.
- **RUNTIME OBSERVED (user, this session):** the user ran the app, clicked **Open project**, picked
  `crates`, and **saw the notice** — confirming the **picker → `project_load` → projection** chain works
  end to end at runtime (the previously-unobserved events-after-subscribe path). So project-load itself
  is runtime-verified; the inline trust path (A6) and the orphan dialog (B8) remain unobserved.
- **SCOPE DECISION (user — top source of truth, §2): pull matrix row A10 (command auto-detection) into
  v1.** The user rejected the jargon notice ("Add a solo.yml with a processes: map…") for a non-developer
  and directed: when a picked folder has **no `solo.yml`, auto-create one** whose contents are
  **auto-detected commands** (scan package.json scripts, Procfile, Makefile/justfile, Cargo, go.mod,
  docker-compose, … — mirroring Solo, plan/05 §9), then show a **friendly, plain-language confirmation**
  naming the file/folder. Architecture mandate (user, verbatim): "single trusted source, no duplicates,
  no scattered code, keep architecture, discipline, clear separation." **NOT YET BUILT** — design only:
  a dedicated detection+writer domain in C1 (`core/config/`), Registry/Strategy (one detector per
  ecosystem behind a `Detector` trait, registered once), single-sourced through the `SoloYml`/`ProcessSpec`
  model (writer serializes via the model + a hand-written header), `ProjectLoad` gains `created`. **TODO
  next session:** update `plan/02` (A10 → v1, this phase) + `plan/05 §9` cross-ref + this ledger.
- **STEP-4 adversarial review of the Phase-5 follow-up (`d497241`+`45461d0`) was STARTED, not finished.**
  Confirmed sound (re-verify, don't trust): the **trust gate** — `start`/`restart`/`start_all` all consult
  the **durable** trust repo (`is_trusted`), NOT the cosmetic `ProcessView.requires_trust` flag, and
  fail-closed; **A6 cannot be bypassed via the flag**. Dialog capability is **least-privilege**
  (`dialog:allow-open`, not `dialog:default`); `tauri-plugin-fs` is pulled in transitively but neither
  `init()`'d nor granted any `fs:` permission → no surface widening. **Open finding:**
  `useTrust.trust`/`trustAll` optimistically drop a command from the open review (and `trustAll` closes
  it) **before** `configTrust` resolves — on a (rare) trust failure the command vanishes from the dialog
  though trust didn't apply (should-fix/nit). Finish the full review next session.
- **Stray files (untracked, LEFT as-is):** root `package-lock.json` (prior user decision) and a new
  **0-byte root `solo.yml`** (appeared during testing; surfaced to the user, not acted on).

### Phase-5 follow-up — second feature session (2026-06-19): project-load UI + trust review
- **Scope:** the final two Phase-5 follow-up pieces, one gated single commit each (start- and end-green;
  `just lint && just test`). **Baseline confirmed first:** 120 (Rust 100 / UI 20). **End: 132 (Rust 103 /
  UI 29).** Stray root `package-lock.json` left untouched; no `cargo update`; `Cargo.lock` only gained the
  dialog-plugin subtree (brotli/alloc-stdlib pins intact). Skills used per CLAUDE.md §5: **tauri-plugins** +
  **context7** (`tauri-plugin-dialog` 2.7.1 crate / `@tauri-apps/plugin-dialog` JS / permission key
  `dialog:allow-open` — verified, not guessed; default GTK backend needs no new system lib), **/impeccable**
  (built from `DESIGN.md`; harness has no image-gen so direct-from-brief), **shadcn** (project is
  framework="Manual" / components=[] — primitives are hand-authored, so reuse `Button`/`Dialog`, don't re-add).
- **Commit `d497241` — project-load UI; demo retired.** Thin **`project_load(path)`** Tauri command (recipe
  §5.5) → `Facade::load_project`; registered in the handler; typed `projectLoad` wrapper in `api.ts`. Native
  folder picker via **`tauri-plugin-dialog`** (`open({ directory: true })`, wrapped as `openProjectDirectory`
  in `api.ts` so the IPC boundary stays in one place) + `tauri_plugin_dialog::init()` + capability
  `dialog:allow-open`. An "Open project" affordance in the **toolbar** (ghost) and as the **empty-state
  primary CTA** (the one azure action there). New **`useProjects`** store action (routes through `api.ts`;
  reports failures on the shared banner via a new `useProcesses.reportError`). **`crates/app/src/demo.rs`
  deleted** + its `demo::seed` call removed — launch with no project now shows the empty state. Tests:
  `useProjects.test.ts` (picks → loads; cancel is a no-op; failure routed) + App empty-state copy updated.
- **Commit `45461d0` — trust review (A6/A9).** **First-open trust UX decision = Option B**, cited to plan/05
  §4 ("Solo blocks untrusted starts and *shows* them; the yml-change dialog is for *changes*") and product.md
  ("modal as first thought" anti-pattern): untrusted commands surface **inline** in the sidebar (Start
  disabled + a **Trust** affordance that trusts directly) so a freshly loaded project is usable; the **dialog**
  is reserved for a `solo.yml` *change*. Core: **`ProcessView.requires_trust`** (computed in `Supervisor`
  from the registry's `trust_variant` + the trust repo; fail-closed on a store error), carried on
  **`ProcessSpawned`**; **`ConfigChanged` enriched** with `commands: Vec<TrustReviewCommand>` (name/command/
  working_dir/env of each touched-and-untrusted command) built by `ConfigEngine` (`sync.rs::pending_trust`);
  **`Facade::trust_command(project, name)`** resolves the spec via a new **`ConfigEngine::spec`** accessor,
  records trust, and clears the read-model flag (`Supervisor::mark_trusted` → `Registry::mark_variant_trusted`);
  new `TrustCommandError`. App: `config_trust` command + `configTrust` wrapper. UI: `requires_trust` mirrored
  in `domain.ts` + handled in the projection; `ProcessControls` disables Start + shows a Trust affordance when
  untrusted (reused in sidebar + terminal header); **`TrustDialog`** (reuses `Dialog`/`Button`; shows the diff
  + each command's detail in mono; "Trust all" the one azure primary, per-command/dismiss ghost — Spent-on-
  Status honored) driven by **`useTrust`** (subscribes `ConfigChanged{requires_trust}`; trust → `config_trust`
  then `store.refresh`). Tests: core (`requires_trust` flips on trust + start unblocks; `NotFound`;
  `pending_trust` carries detail), UI (`TrustDialog` component; sidebar blocks+trusts an untrusted command; an
  emitted `ConfigChanged` pops the dialog — the closest A9 runtime check available pre-watcher).
- **Architecture conformance:** every behaviour routes through the one `Facade` (`load_project`,
  `trust_command`); adapters/React hold no business logic; new command/event strings live once (`api.ts`);
  the `DomainEvent` union + TS mirror stay exhaustive (`ProcessSpawned`/`ConfigChanged` extended on both sides
  per §5.6); `TrustReviewCommand` defined once in `core::config::review` and mirrored once in `domain.ts`;
  the `Dialog`/`Button` primitives are reused, not re-rolled. File-size guard zero outliers; dep-guard green.

### Phase-5 follow-up — feature session (2026-06-19, after cleanup sign-off)
- **Scope:** the deferred Phase-5 follow-up. Cleanup R0–R6 was signed off (the session prompt directing
  this feature work is the sign-off). Worked in disciplined, gated, one-feature-per-commit increments;
  `just lint && just test` green at the start of and after every commit. **Baseline confirmed first:**
  107 (Rust 97 / UI 10). **End: 120 (Rust 100 / UI 20).** Stray root `package-lock.json` left untouched
  (user decision); no `cargo update`; `Cargo.lock` unchanged.
- **Task-6 testing — RESEARCHED; Playwright is the wrong tool for Tauri.** The session prompt named
  "Playwright via the webapp-testing skill," but: the `webapp-testing` skill is **not installed** (only the
  project-local `tauri-testing` skill exists), and `tauri-driver`/`WebKitWebDriver` are **not present**.
  Researched the ecosystem (official Tauri testing docs + the `tauri-testing` skill): Tauri on Linux renders
  in **WebKitGTK**, which exposes no CDP, so **Playwright cannot drive a Tauri app** ("Playwright flat-out
  doesn't work because Tauri uses WebKitGTK, not Chromium"). Tauri's official e2e is the **WebDriver protocol
  via `tauri-driver` + WebdriverIO/Selenium** — never Playwright. Sources: v2.tauri.app/develop/tests/(webdriver/),
  the WebKit-engine-mismatch writeup, tauri discussion #3768. **Decision (two layers):** (layer 1, built
  this session) component/integration tests via `vitest` + `jsdom` + `@testing-library/react` + the
  `@tauri-apps/api/mocks` `mockIPC` — fast, deterministic, CI-ready today, no system installs; (layer 2,
  recorded as a follow-up) the real-window e2e is **WebdriverIO + `tauri-driver` + `webkit2gtk-driver` (apt,
  sudo) + xvfb**, which the skill's reference CI workflow runs on ubuntu — wire it when the system dep is
  installed (offer the user `! sudo apt install webkit2gtk-driver xvfb`). **New dev-deps (UI, dev-only — no
  shipped-bundle impact):** `jsdom` 29.1.1, `@testing-library/react` 16.3.2.
- **Commit `d1ef290` — mockIPC dashboard test (Task 6, layer 1).** `crates/app/ui/src/App.test.tsx`
  (per-file `// @vitest-environment jsdom`, so the pure reducer tests stay on the fast node env). Renders
  `App` against a mocked backend and asserts the integration-level behaviour the pure tests can't: subtype
  **grouping**, per-row **`[data-status]`**, **FSM-derived control enable/disable**, **row selection**
  opening the terminal pane, and the **empty state**. The xterm-backed `useTerminal` hook is `vi.mock`-stubbed
  (jsdom can't measure the emulator surface; the real PTY/echo path is layer 2 + the recorded human-verified
  echo). UI 10 → 14.
- **Commit `482988b` — orphan dialog (B8 UI).** Core primitive **`Supervisor::kill_orphan(pgid)`**
  (`supervisor/reconcile.rs`): SIGKILL the group via `OrphanControl` + `RuntimeState::forget` — best-effort,
  with a direct test. Thin **`orphans_resolve(pgids)`** Tauri command routes to it (registered in the handler).
  New **`Dialog` primitive** (`components/ui/dialog.tsx`) hand-authored on the **unified `radix-ui` package**
  (matches the project's `Collapsible`/`Tooltip`/`Slot` pattern; avoids the redundant `@radix-ui/react-dialog`
  dep the shadcn CLI would pull — its `components.json` reads as "Manual"). App-level **`OrphanDialog`** +
  **`useOrphans`** store hook (subscribes to `OrphansFound`; Kill / Kill all / Leave). Per **DESIGN.md's
  Spent-on-Status rule**, killing stays **slate** (ghost/outline — no saturated red), and the non-destructive
  **Leave running** is the one azure primary + the Esc/backdrop default. Rust 97 → 98, UI 14 → 17.
- **Commit `d9416ed` — terminal title/bell → header.** Focused **`useTerminalChrome(id)`** hook subscribes
  the selected pane to the low-rate `TerminalTitleChanged`/`TerminalBell` events (kept off the
  high-throughput byte path `useTerminal` owns): renders the OSC title (falling back to the label) + a
  transient azure bell indicator. Test drives **real `domain-event` emissions** via
  `mockIPC(..., { shouldMockEvents: true })` + `emit`. UI 17 → 20.
- **Commit `47458ea` — `Facade::load_project(root)` core wiring (the heart of project-load).** Opens a
  project end to end: `projects.add` (durable `ProjectId` + canonical root) → `config.open` (load `solo.yml`,
  seed sync state) → register each `ProcessSpec` as a trust-gated command → **`reconcile_orphans()` AFTER
  registration** (so a leftover matching a `solo.yml` command is adopted, not mis-surfaced) → `start_all`
  (the trusted auto-start subset). Untrusted commands register visible-but-`Stopped` and never run until
  trusted — loading never bypasses the trust gate. New `LoadProjectError` (exported). Two tests (registers
  each declared command; starts a pre-trusted auto-start command). Rust 98 → 100. **`demo.rs` is NOT yet
  removed** — that happens with the driving command + file-picker (next).
- **Architecture conformance:** every behaviour routes through the one `Facade`/`Supervisor`; adapters/React
  hold no business logic; the `DomainEvent` union + TS mirror stay exhaustive; new strings live once
  (`orphans_resolve` in `api.ts`); the `Dialog` is a reused primitive. File-size guard zero outliers; dep-guard
  green; tests inline + honest.

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
- **Config→supervisor wiring — DONE end to end (core + UI).** `Facade::load_project` (`47458ea`) +
  the **project-load UI** (`d497241`): `project_load(path)` command → `load_project` (`projects.add` →
  `config.open` → `Supervisor::register` per spec → **`reconcile_orphans()` after registration** →
  `start_all`); a `tauri-plugin-dialog` folder picker + "Open project" affordance + `useProjects`;
  **`demo.rs` removed**. `orphans_resolve` (`482988b`) and **`config_trust`** (`45461d0`) are built;
  `project_switch` is a Phase-11 polish item (not v1-gating). **Remaining = runtime/manual confirmation**:
  a `just dev` run opening a real `solo.yml` and seeing its stack populate (not observed this session).
- **B8 orphan adoption — mechanism + UI + reconcile-call now all in place.** The mechanism (record/reconcile/
  adopt/surface/prune) + real adapters were done earlier; **this session added the B8 *dialog*** (`482988b`:
  `OrphanDialog` + `useOrphans` on `OrphansFound`, core `kill_orphan`, `orphans_resolve` command) and the
  **reconcile-on-launch call now lives inside `Facade::load_project`** (after registration), so it fires when
  a project loads. **The project-load UI now calls `load_project` (`d497241`)**, so the full chain (load →
  reconcile → `OrphansFound` → dialog) is wired end to end; only **runtime confirmation** (a `just dev` run
  with a leftover group) remains — not observed this session. B7's **"clears crash tracking"** half remains a
  Phase-6 item.
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

**A. (Phase 8 session 2) Fan out the remaining v1 MCP tools onto the skeleton.**
The skeleton is on branch **`feat/phase-8-mcp-skeleton`** (this session — review/merge it first; gate `just lint`
+ `just test` exit 0). The path **agent → `soloist-mcp` (rmcp stdio) → IPC/UDS → app → `Facade` → core** is proven
end-to-end with identity/scope + 5 read tools. Next, add the rest of the **v1 F-set** the same way — each tool is a
thin `crates/mcp` handler (clean-room `schemars` params) → **one new/existing `Facade` method** → an `IpcRequest`
the app-side `handle_request` routes; no domain logic in the handler. Order of attack:
  1. **F6 mutations + F7** — `start_process`/`stop_process`/`restart_process`/`rename_process`/`select_process`/
     `close_process` and `send_input` (text + raw bytes; optional `wait_ms` returns the rendered tail). These are
     the first **action** tools, so they bring in **F13**: the trust gate + effective scope are enforced **in the
     core** (the `Facade` method), not the handler — so a tool can't run an untrusted command or touch another
     project. Add wire variants to `IpcRequest`/`IpcResponse` + arms to `handle_request` + tools.
  2. **F11 `spawn_agent`** — routes to the **existing `Facade::launch_agent`** (do not reimplement). This is the
     **E7** unblock (a lead agent spawning a worker over MCP). Plus `list_agent_tools` → `Facade::agents().list_tools`.
  3. **F8 bulk** (`start/stop/restart_all_commands`, trusted-in-scope only), **F9 output** (`get_process_output`/
     `_raw`, `search_output`/`_raw`, `clear_output`, `flush_terminal_perf`, `get_process_ports`), **F10 services**
     (`services_list`, `wait_for_bound_port` → `Facade::wait_for_port`).
Keep slices small + gate-green; each is its own commit. Use the **official MCP docs** + `rmcp` via context7 (the
`mcp-builder` skill is **not installed**). Then the F1 **helper bundling** (`externalBin`/sidecar) is a Phase-12
packaging item — note it, don't pull it forward. **F2/F12/F14 stay `later`.** Recorded gaps to keep in mind: MCP
tool **param schemas are ours** (`plan/05` §12) — document each; the `ipc`-deps-`core` type-linkage may want an
`ipc` split in P10 if CLI size matters (`plan/06` §2).

**A-prior. Phase 7 is now `Verified`** (user-confirmed at runtime 2026-06-22 — agent idle FSM + native login
observed in the running app; PR #15 `b95dc6a` already merged + CI-green). **E6** stays `later` + OFF; **E7**
completes via F11 above + P9. Idle-heuristic thresholds/cues remain a recorded gap (`KNOWN-DIVERGENCES.md` D-5 /
plan/05 §12). (The **package-lock.json** gitignore question is still open — see Decisions.)

**B. (Phase 6, user-only — still owed)**
1. **FLIP PHASE 6 → `Verified`: run the Phase-6 runtime acceptance walk via `just dev` (user-only — desktop,
   host `DISPLAY=:0`).** All Phase-6 v1 code is complete (D1–D8, D11, D5, soak gate, UI surfacing). Observe,
   with evidence: (a) a trusted `auto_restart` command you `kill -9` → Crashed → Starting → Running on its own,
   and its terminal **keeps the pre-crash output with a `── restarted ──` banner before the new output** (D5),
   and a desktop **toast** fires (D8); (b) a command that crashes instantly and repeatedly stops at **exactly
   10/60s** → `RestartExhausted`, no hot-loop (D4); (c) a busy command (`yes >/dev/null`) shows **moving
   CPU%/RSS** while idle sits ~0 (D1); (d) a dev server (`python -m http.server`) lists its bound **port** on
   its row (D2); (e) **edit a watched file → one debounced restart** + banner, edit an ignored path → nothing
   (D6/D7); (f) killing the metrics sampler task → it self-restarts, app unaffected. Once observed, Phase 6 →
   `Verified`. Baseline: branch `feat/phase-6-restart-banner`, newest `e75adc8`; gate **Rust core 163 / sys 14 /
   pty 10 +soak 3 ignored / store 13 / UI 60**. (The D5 PR is **not pushed/opened yet** — see the top Decisions
   entry; push + open it, or fold into the Phase-6 wrap-up, per the user's call.) **Then Phase 7** (agents &
   idle detection; summarization OFF by default).

0. **Verify the project-grouped sidebar at runtime (user-only — restart `just dev` so the Rust restore
   rebuilds; the commits live on a dedicated branch, see Decisions).** Observe, with evidence: (a) on
   **launch**, previously-opened projects reappear in the sidebar — each a collapsible **project node**
   (icon/monogram + name + `running/total`) over its non-empty kind subgroups — **resting** (nothing
   auto-started); (b) **Open project** → a folder with a `solo.yml` → its project node + commands appear;
   (c) a project whose `solo.yml` sets `icon:` shows that **image** in the avatar (A13), else the monogram;
   (d) the **per-project** bulk controls (Start all / Restart running / Stop all) act only on that project;
   (e) empty Agents/Terminals subgroups are **hidden**. If a project shows but is empty or an icon is
   missing, report it. Baseline: gate **186 (Rust 145 / UI 41)**.
0a. **Confirm Phase 5 + A10 at runtime, then flip Phase 5 to `Verified` (user-only — needs a desktop
   `just dev`, host `DISPLAY=:0`).** Observe, with evidence: (a) launch with no project → empty state;
   **Open project** → pick a folder **with** a `solo.yml` → its stack populates; (b) **A10:** pick a folder
   **without** a `solo.yml` (e.g. a Node/Cargo/Procfile project) → a `solo.yml` is created and the friendly
   confirmation names the file/folder + the count → the detected commands appear trust-gated; (c) an
   untrusted command shows Start disabled + a **Trust** affordance → click Trust → it becomes startable
   (A6, first-open); (d) a leftover process group surfaces the **orphan dialog** (B8). **A9 end-to-end**
   (the trust dialog on a *live* `solo.yml` edit) is **gated on the Phase-6 file watcher** — emit-tested
   now, no runtime trigger until the watcher lands; verify during Phase 6. Once (a)–(d) are observed,
   Phase 5 → `Verified` (also flips the long-open Phase 1 in-GUI click, same run). Baseline: `git log`
   newest = `8f8c524`; gate **174 (Rust 138 / UI 36)**. Locked decisions hold (tests inline; 7 placeholder
   modules + 4 stub crates stay; **leave** the stray root `package-lock.json` — do not rm/gitignore/stage;
   the 0-byte root `solo.yml` is gone — not recreated, see Decisions).
0b. **Tracked review findings (from the STEP-4 review; address when their area is next touched, none v1-
   blocking):** (1) ~~`useProcesses.projectId = processes[0]?.project` is wrong for multiple loaded
   projects~~ — **FIXED 2026-06-20**: bulk ops are now **per-project** (scoped by id in each project header);
   the single-project `projectId` field is gone. (2) `load_project` runs blocking fs on the async command
   thread — move off-thread per
   §8 (careful: it also spawns actors). (3) trusting clears `requires_trust` via `refresh()` with no event
   — add a `ProcessTrusted` `DomainEvent` (§5.6) to kill the snapshot race. (4) `project_load` path not
   validated (trusted webview; gate still blocks exec). (5) `auto_start_candidates` skips
   `Crashed`/`RestartExhausted` — fold into the start-all-vs-start-auto open thread.
2. **Phase 6 is code-complete — nothing left to build for v1.** All rows landed: D4+D11 (`90d51ac`), D1/D2/D3
   OS-probe (PR #8), D6/D7 file-watch live (PR #9), D8 notifications (PR #10), soak gate + UI surfacing +
   metrics fix (PR #11), and **D5 restart banner** (`e75adc8`). The **A9** trust dialog now fires on a real
   `solo.yml` edit at runtime (the file watcher is live) — confirm it during the Phase-5/6 `just dev` walk.
   The only gate to `Verified` is the runtime acceptance walk (item 1).
2-os. **Runtime-verify the OS probes (user, `just dev`).** With evidence: a busy command (`yes >/dev/null`)
   shows **moving CPU%/RSS** while an idle one sits ~0; a dev server (`python -m http.server`) lists its bound
   **port** on its row/`ProcessView.ports`; killing the metrics sampler task → it **self-restarts**, app
   unaffected. (`wait_for_port`/readiness has no GUI trigger until the Phase-8 MCP `wait_for_bound_port` tool;
   it is covered by mock-clock tests now.) The CPU%/RSS + port UI surfacing is a later `/impeccable` step.
2a. **Runtime-verify auto-restart (user, `just dev`):** an `auto_restart: true` trusted command that you
   `kill -9` should go Crashed → Starting → Running on its own; one that crashes instantly and repeatedly
   should stop at exactly 10 restarts within 60 s and show `RestartExhausted` (no hot-loop). Desktop
   notifications for these arrive with D8 (not built yet).
3. **Task 6 layer 2 — real-window e2e (recorded follow-up, needs a system dep).** Layer 1 (mockIPC component
   tests) is done. The real-window/PTY-echo e2e is **WebdriverIO + `tauri-driver` + `webkit2gtk-driver`** — NOT
   Playwright (WebKitGTK exposes no CDP; researched 2026-06-19). Install: `cargo install tauri-driver --locked`
   + `! sudo apt install webkit2gtk-driver xvfb`, then an `e2e/` WebdriverIO harness (the `tauri-testing` skill's
   reference `wdio.conf.js`) + a CI job (its reference workflow runs on ubuntu). Offer the sudo step to the user.
4. **Also fold in (small, non-gating):** the toolbar **"Start all"** start-all-vs-start-auto split (open
   thread); generate the `.impeccable/design.json` sidecar once components stabilise; consider lazy-loading
   xterm to trim the 167 KB-gzip bundle (§6, measure in Phase 12); refine `useTerminal` so a resting↔active
   status flip doesn't re-create the xterm (re-attach/replay — correct but mildly janky).
5. **Do not pull deferred `later` rows into v1** (A5/A8/A10/A12/A13, B9, C8 webgl). The live `notify` watcher
   is now **Phase 6 work** (item 2), no longer "deferred".
