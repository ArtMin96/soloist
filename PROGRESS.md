# PROGRESS.md — Soloist State Ledger

> **This file is the shared memory across sessions.** Git history complements it, but this ledger is
> where a session reads what's done and what's next. **Read it at the start of every session** (per
> `CLAUDE.md` §1) and **update it at the end of every session** (per `CLAUDE.md` §10–§11). Keep it
> factual and evidence-backed — never mark `Verified` what you didn't verify.

---

## Current state

- **Overall:** **Phase 1 (Walking skeleton) code-complete — pending one manual GUI check.** The
  hexagonal spine is real and proven end to end by automated tests (`just lint && just test` green); the
  only unautomated acceptance step is the in-GUI Start/Stop click (Playwright deferred to Phase 5).
- **Active phase:** **Phase 1 (Walking skeleton).** Status `Done — pending verify`.
- **Last session:** 2026-06-14 — built Phase 1 end to end (details under "Decisions / changes").

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
| 2 | Config & projects (real `solo.yml`, trust, sync, detect) | Not started | |
| 3 | Process supervisor (3 subtypes, status FSM, orphans) | Not started | Highest-risk; budget extra test time |
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
- **`KNOWN-DIVERGENCES.md`** not created yet (introduced in Phase 13; start it the moment a real
  Solo-behavior divergence is implemented — not needed for the build/toolchain decisions above).
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

1. **Confirm Phase 1's one open acceptance step, then mark it `Verified`:** run `just dev`, click *Start
   demo process* (a real `sleep 60` should appear and go `starting → running`), click *Stop* (it should
   go `stopping → stopped`); the rest of Phase 1's acceptance is already green via automated tests. After
   confirming, flip the Phase 1 row to `Verified` here.
2. **Then begin Phase 2 — Config & projects.** Read `plan/phases/phase-02-config-and-projects.md` end to
   end and re-read its parity rows (A1–A12). Grow the `crates/store` repositories (trust, projects) and
   the `crates/core` `config`/`projects`/`trust` contexts on the skeleton built this phase. Keep the
   discipline bar (CLAUDE.md §15): single-source, no magic strings, real tests, small files.
3. Run `just lint && just test` first to confirm the Phase 1 baseline is still green.
