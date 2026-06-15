# PROGRESS.md — Soloist State Ledger

> **This file is the shared memory across sessions.** Git history complements it, but this ledger is
> where a session reads what's done and what's next. **Read it at the start of every session** (per
> `CLAUDE.md` §1) and **update it at the end of every session** (per `CLAUDE.md` §10–§11). Keep it
> factual and evidence-backed — never mark `Verified` what you didn't verify.

---

## Current state

- **Overall:** **Phase 3 (Process Supervisor, C2) — `In progress`. The B1–B7 core slice + the Phase-2
  runtime deferrals (A2, A6) are code-complete and headless-verified; B8 (orphan adoption) and Task 4
  (output capture/log ring) are deferred to the next Phase-3 session** (the user chose "Core first" to
  give B8 dedicated test time). Built on the Phase 1 actor + Phase 2 C1: a real `Supervisor` (registry,
  mailbox-driven actor with `Stop`/`Restart`, status FSM, graceful SIGTERM→5s→SIGKILL on the **process
  group**, exit classification, panic isolation), the **trust gate enforced in core** on
  start/restart/bulk, bulk ops (start_all/stop_all/restart_running), the stop→lock-release hook, and
  login-shell execution (`$SHELL -lc`) honoring `working_dir`/`env`/`auto_start`. **Phase 3 PR
  reviewed via `REVIEW-PROMPT.md` and all findings applied** (shutdown wired to the Tauri exit
  event; atomic start-claim; passwd shell fallback; comment/doc nits). `just lint && just test`
  green: **77 tests** (core 56 / pty-integration 5 / store 10 / UI 6).
- **Active phase:** **Phase 3 (Process Supervisor).** Status `In progress` (B1–B7 done-pending-verify;
  B8 + output capture remain).
- **Phase 2:** `Done — pending verify` — its runtime deferrals A2/A6 are now closed in Phase 3.
- **Phase 1:** still `Done — pending verify` — its one open step is the **manual in-GUI Start/Stop
  click** (`just dev`); the demo now routes through the real supervisor (an ungated terminal running
  `sleep 60`), so the click-through path is unchanged and still valid to confirm.
- **Last session:** 2026-06-15 — built Phase 3 / context C2 (B1–B7 + A2/A6); details under "Decisions / changes".

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
| 3 | Process supervisor (3 subtypes, status FSM, orphans) | **In progress** | **B1–B7 + A2/A6 done — pending verify.** `Supervisor` (C2) on the Phase-1 actor: mailbox actor (`Stop`/`Restart`), status FSM (+`Crashed→Starting` edge), graceful SIGTERM→5s→SIGKILL on the **pgroup**, exit classification (+`exit_code` on the status event), panic isolation; **trust gate in core** on start/restart/start_all (A6); login-shell `$SHELL -lc` honoring `working_dir`/`env`/`auto_start` (A2/B5); bulk start_all/stop_all/restart_running (B4); stop→lock-release hook (B7, `LockReleaser` port, `NoopLockReleaser` until C6/Phase 9). **Evidence:** core 56 (14 supervisor incl. FSM-restart + shutdown-reaps tests), pty 5 (real-OS: pgroup reap, grandchild reap, trap-TERM→SIGKILL escalation, cwd/env-honored-via-exit-code, **start/stop 50 → zero leaked PIDs**). `just lint && just test` green (77). **Reviewed + fixed** (see Decisions): shutdown now wired to the Tauri `ExitRequested` event; start path made race-free; shell resolution gains the passwd fallback. **Remaining (next session):** B8 orphan adoption (runtime-state file port + reconcile adopt/surface/prune), Task 4 output capture/log ring (5,000-line bound), and the "clears crash tracking" half of B7 (Phase-6 restart policy). |
| 4 | PTY & terminal I/O (rendered+raw, input, resize, OSC) | Not started | |
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
- **Phase 3 remaining (next session):** **B8 orphan adoption** — needs a new runtime-state-file port
  (persist `{project_root,name,command,pgid}`; NOT SQLite, per plan/04 §7) + a liveness/adopt mechanism,
  and `reconcile_orphans()` classifying adopt (project+name+command match) / surface (`OrphansFound`) /
  prune; **Task 4 output capture** — add an output channel to `Spawned` + a bounded `Ring<LogLine>`
  (5,000-line cap, plan/04 §8) + per-process log fan-out, drained by the actor (the dashboard/MCP
  consume it later); and B7's **"clears crash tracking"** half (Phase-6 restart policy).
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

1. Run `just lint && just test` first to confirm the Phase 3 baseline is still green (**77 tests**:
   core 56 / pty 5 / store 10 / UI 6).
2. **Finish Phase 3 — B8 orphan adoption + Task 4 output capture.** Re-read
   `plan/phases/phase-03-process-supervisor.md` Tasks 4 & 9 and parity row **B8**, plus plan/04 §7
   (runtime-state file, NOT SQLite) and §8 (bounded ring). Suggested order:
   - **B8:** add a `RuntimeState`/`OrphanStore` port (persist `{project_root, name, command, pgid}` to a
     small file in the data dir) + a liveness/adopt mechanism (the spawner adapter or a new port: is the
     pgid alive? signal it). `Supervisor::reconcile_orphans()` → pure classify (adopt when
     project+name+command match a known config command / surface `OrphansFound` / prune dead), with the
     adopted process re-registered as `Running` tracking the existing pgid (poll liveness on the
     `Clock`). The Kill/KillAll/Leave dialog is Phase 5 UI — emit the decision event only.
   - **Task 4:** add `output: mpsc::Receiver<Vec<u8>>` to `Spawned` (bounded — backpressure), have the
     `pty` adapter pipe stdout+stderr through a reader task, and have the actor drain it into a bounded
     `Ring<LogLine>` (new small `ring.rs`, 5,000-line cap) + a per-process log broadcast
     (`subscribe_logs`). Add the "buffer bound" test from the phase test plan.
   - Then flip Phase 3 to `Done — pending verify` once every B1–B8 row + acceptance + test plan is green.
3. **Wire "open project → register commands → start_all"** (connect `ConfigEngine::open` to
   `Supervisor::register` per command) — either in the B8 session or Phase 5.
4. **Still open from Phase 1 (independent, user-only):** confirm the in-GUI Start/Stop click via
   `just dev` (the demo terminal `sleep 60` goes `stopped → starting → running`, then `stopping →
   stopped`), then flip the Phase 1 row to `Verified`. Cannot be automated headlessly before Phase 5 Playwright.
5. **Do not pull deferred `later` rows into v1** (A5/A8/A10/A12/A13, B9 "resume last session") and **do
   not** build the live `notify` watcher before Phase 6 — all recorded above with rationale.
