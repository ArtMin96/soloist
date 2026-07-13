# Soloist stability & security audit — raw findings log (2026-07-13)

> Working scratch for a multi-agent audit. This is the **evidence ledger**; the PRDs
> (`10-*.md` … ) are the actionable output derived from it. A finding is only promoted to a
> PRD after it is **verified against the code by the main session** (not just asserted by an
> agent). Status tags: `VERIFIED` (main session re-checked the code), `AGENT` (reported by a
> sub-agent, not yet re-verified), `KNOWN-DEFERRED` (already tracked in PROGRESS.md).

Method: ~13 read-only audit agents fanned out across domains, each grounded in `plan/05`
(behavior contract) and `plan/04`/`06` (design) before judging, told what the 2026-07-13
stability sprint already fixed (so they verify rather than re-report), and required to give
`file:line` + a concrete failure scenario. The first batch was killed mid-run by a session
limit; a second consolidated batch re-ran the killed domains. Test-honesty and HTTP/CLI
completed in the first batch.

Baseline: branch `main` @ `15bdd1a` (v0.5.0), the stability-hardening sprint already merged.

---

## A. HTTP API + CLI (COMPLETE — first batch)

### A1 — HTTP read routes are completely unauthenticated + loopback is not a per-user boundary — `VERIFIED` (P1 security)
- **Code:** `crates/httpapi/src/routes.rs:26-37` — `read_routes()` (`/status`, `/processes`,
  `/processes/:id/ports`, `/processes/:id/output`, `/projects`, `/feedback`) has **no auth
  layer**; only `crate::mutations::router()` gets `require_local_auth`. The comment states
  reads are "open on loopback (no auth gate)".
- **Auth is a constant, not a secret:** the mutation gate compares the header against
  `LOCAL_AUTH_VALUE = "1"` (`crates/ipc/src/http.rs:28`), so it is CSRF protection (blocks
  cross-origin browser JS via a non-simple header + preflight), **not** authentication — any
  local process can send `X-Soloist-Local-Auth: 1`.
- **Contrast:** the MCP unix socket is defended by a `0700` data dir + `SO_PEERCRED` peer check
  (`crates/ipc/src/paths.rs`), i.e. a real per-owner boundary. The HTTP surface has no
  equivalent — the `127.0.0.1` bind is shared across every local UID.
- **Impact:** on the multi-user Ubuntu target (D2), a second local account (or any local
  process, e.g. a compromised dependency in another project) can `GET
  /processes/:id/output` to exfiltrate another user's process output/logs (dev-server secrets,
  tokens) with no credential, and drive every mutation (start/stop/restart, spawn-agent,
  `DELETE /projects/:id`).
- **Note:** the header-vs-bearer *scheme choice* is a recorded owner decision (`plan/05`;
  Solo v0.9.3 moved to a rotating bearer token + discovery file) — do NOT relitigate the
  scheme. The flagged defects are (a) reads have zero gate, (b) no peer-cred/UID guard, so the
  loopback bind is the only boundary and it is not per-user.

### A2 — No `Host`-header validation → DNS-rebinding read disclosure — `AGENT` (P2 security)
- CORS (`crates/httpapi/src/cors.rs`) inspects only `Origin`; nothing validates `Host`. A page
  on `evil.example:24678` that rebinds DNS to `127.0.0.1` makes *same-origin* fetches (CORS
  never applies) and can read `/processes`, `/processes/:id/output`, `/projects`, and set the
  constant auth header freely. The classic loopback defense (reject `Host` ≠ 127.0.0.1/
  localhost) is absent.

### A3 — No rate limiting on mutations, notably `spawn-agent` — `AGENT` (P2)
- `crates/httpapi/src/mutations.rs` — `POST /projects/:id/spawn-agent` routes to `launch_agent`
  with no quota; any local caller (see A1) can loop it to exhaust processes/PTYs/FDs.

### A4 — CLI default-port fallback can address a foreign server — `AGENT` (P2)
- `crates/cli/src/client.rs:49-52` falls back to `DEFAULT_PORT` 24678 when the runtime file is
  missing; if the server bound a fallback port and `write_runtime` failed, the CLI sends
  mutations (with the auth header) to whatever occupies 24678. No server-identity check before
  mutating.

### A5 — `status` handler aggregates in the adapter (route-to-facade) — `AGENT` (P2 discipline)
- `crates/httpapi/src/routes.rs:56-72` computes the `running` tally with a domain predicate in
  the handler instead of one façade read. Minor `plan/06 §16` violation.

### A6 — CLI table renders unescaped process labels — `AGENT` (P2)
- `crates/cli/src/command.rs:216-236` writes agent-settable `p.label` straight into stdout
  cells; embedded ANSI/OSC can spoof/corrupt the `soloist status` table.

**HTTP/CLI conformance PASSES (verified by agent):** every mutation is behind the gate (14
routes under one `route_layer`); bind is loopback-only on every attempt incl. fallback; port
collision degrades cleanly (tries 24678 + 16 offsets → OS port → disable, logged); CORS origin
match is exact (`evil-localhost.com`, `localhost.evil.com`, `Origin: null` all rejected);
preflight protects cross-origin mutations; no internal-detail leakage in responses; JSON bodies
capped by axum 2 MB default; data dir + socket `0700`; single-source wire contract in
`crates/ipc/src/http.rs`.

---

## B. Test-suite honesty (COMPLETE — first batch; UI + app/cli/httpapi + store/ipc/sys/pty)

**Verdict so far: the owner's "most tests are pretend" claim does NOT hold for the crates
audited.** 402 tests classified across three sub-audits; **395 REAL**, 9 tautological, 0
over-mocked, 0 implementation-coupled. The suite is unusually healthy. BUT there are real,
high-value **coverage holes** (category E), which matter more than the handful of weak tests.

### B-summary (audited crates)
| Area | Tests | Real | Tautological (B) | Over-mocked (D) |
|---|---|---|---|---|
| UI (`crates/app/ui`) | 163 | 160 | 3 | 0 |
| app + cli + httpapi | 109 | 108 | 1 | 0 |
| store + ipc + sys + pty | 130 | 125 | 5 | 0 |
| **core + mcp** | **638** | **633** | **3** | **0** (+2 C) |
| **TOTAL** | **1040** | **1026** | **12** | **0** (+2 C) |

**Bottom line the owner should hear plainly: the "too many pretend tests" belief does NOT hold.**
1040 tests classified across every crate — **~98.7% exercise real behavior**, 0 over-mocked, only
12 trivial tautologies + 2 prose-substring smoke tests. The core+mcp crates (the domain brain)
are the *strongest* part at 99.2% real, with faithful in-memory fakes (real revision guards /
ownership / upsert, not return-stubs) and a `MockClock` that genuinely advances. The daily
instability is **runtime bugs** (§C1/C2 etc.), **not** fake tests hiding broken features. The
real test weakness is **coverage holes** (below), which matter more than the weak tests.

### B1 — Trust gate is NEVER exercised over HTTP (no 403 test anywhere) — `AGENT` (biggest test gap)
- Every httpapi fixture registers an *ungated* terminal process, so `start` needs no trust. No
  test POSTs `start`/`restart` on an untrusted **command** and asserts 403. The
  `SupervisorError::Untrusted → 403` mapping (`mutations.rs:60`) and the CLI's "that command is
  not trusted" message (`client.rs:141`) are dead paths in the suite. Whether **core** enforces
  the gate is being checked by the 2nd-batch core-test agent.

### B2 — 8 of 14 mutation routes have no direct 401 test — `AGENT` (P2 test)
- Covered by the shared `route_layer` by construction, but a future route added to the *open*
  read router instead would only be caught for the 6 tested routes.

### B3 — No populated-DB migration-upgrade test — `AGENT` (P2 test)
- `migrate.rs` tests fresh-DB → v12 and idempotent re-run, but never upgrades a *populated*
  intermediate-version DB preserving rows. Low risk today (all steps `CREATE TABLE IF NOT
  EXISTS`); the first `ALTER TABLE` will land with no harness.

### B4 — IPC frame reader error paths untested — `AGENT` (P2 test)
- Oversized-prefix rejection is tested, but truncated-body-after-valid-prefix (`FrameError::Io`)
  and garbage/non-JSON payload (`FrameError::Codec`) are not.

### B5 — peer_cred fail-closed paths untested — `AGENT` (P2 test)
- `peer_cred.rs` has **no UID check at all** — `peer_pgid` reads `SO_PEERCRED` only for the
  peer's process *group* (project scoping); cross-user rejection is delegated to the `0700`
  socket dir. The `None` (unresolvable-peer) fail-closed path and the dropped-connection-on-
  unreadable-creds path are untested.

### The 9 tautological tests (delete/strengthen — low priority)
- UI: `lib/todo.test.ts:5` (copy-map echo), `store/timerPanel.test.ts:54` (self-comparison),
  `store/signalStore.test.ts:57` (constant-wiring echo).
- app/cli: `crates/cli/src/client_tests.rs:20` (`Display` pass-through echo).
- store/ipc: `kv_tests.rs:36`, `kv_tests.rs:43`, `project_settings_tests.rs:44`,
  `settings_tests.rs:93` (pure write-then-read echoes), `protocol_tests.rs:359` (fieldless-enum
  round-trip that can't catch a rename).

---

### Test coverage HOLES (real behavior with no/weak test — ranked; these are the actionable test work)
1. **CORE trust-gate IS tested** — `an_untrusted_command_cannot_run_by_any_path`
   (`supervisor.rs:692`) proves start/restart/start_all/resume all refuse an untrusted variant.
   The gap (B1) is **adapter-local**: no HTTP 403 integration test, no CLI "not trusted" test.
2. **MCP scope isolation IS tested** in core (`facade/scoped_tests.rs:110` `OutOfScope`,
   orchestration scoped, transfer `ForeignProject`, `ForeignProcess` binding). MCP crate only
   tests refusals *surface* — correct. So D1 (cross-project *read*) is a **design** question, not
   an untested one: reads are intentionally open; the tests match that intent.
3. **`Todos/Scratchpads::transfer` SUCCESS path untested** (only the `ForeignProject` refusal is)
   — the cross-project re-key that clears blockers + lock is unexercised. (Med)
4. **Populated hotkey-keymap serde round-trip untested** — only `from_str("{}")`; a remap + a
   disabled (`None`) entry + the `#[serde(rename="super")]` never round-trip. A serde regression
   would **silently reset every user's keybindings on reload**. (Med — user-visible)
5. **Config write-side 1 MB ceiling untested** — only the read-side limit is; the `write()` →
   `ConfigError::TooLarge` path (`config/sync.rs:192`) has no test. (Med)
6. **Port scanner + metrics sampler single-process only** — per-pgid attribution / per-process
   suppression never tested with 2+ concurrent groups; a cross-attribution bug would pass. (Low-Med)
7. **`facade/output` public reads untested at the façade** — `process_output` (default/cap
   counts), `search_output`, `process_raw_output`, `process_ports`, unknown-id `None`. (Low-Med)
8. Boundary ticks (restart-window strict-`<60s` edge; exact 5 s SIGKILL grace; ring 5000/256 KB
   constant wiring); config never rejects a LIST-form `processes:`; feedback exact-`MAX_LEN` +
   char-vs-byte; integration-file atomic replace / symlink→regular. (Low)

### The 5 non-real core+mcp tests (delete/strengthen — trivial)
- **B** `ids.rs:118` `from_raw_round_trips_a_wire_value` (newtype identity); `agents/lineage_tests.rs:7`
  `parent_of_returns_the_recorded_parent` (put-then-get); `coordination/kv_tests.rs:85`
  `list_returns_complex_json_value` (echo, dup of :51).
- **C** `support/guide_tests.rs:22` + `:43` (assert guide prose `contains("untrusted")`/`("revision")`
  — a reword breaks them though behavior is unchanged; catch only wholesale topic deletion).

## C. Supervisor + PTY + agent lifecycle (COMPLETE — 2nd batch)

Verified the four stability-sprint fixes are complete & gap-free. Strong conformance PASSES
(verified in code): crash cap 10/60s is a correct **sliding** window (attempts 1..=10, 11th
in-window exhausts, publishes once; a user start/restart clears it) — `restart.rs:62-76`;
graceful stop SIGTERM→5s→SIGKILL then reap in every branch — `actor.rs:422-437`; fresh process
group + `killpg` throughout (never bare PID); PTY tail drained after exit (EIO/0-len, 100 ms
grace); no zombies (dedicated reaper thread `wait()`s; adopted groups poll, never `wait()`);
bounded everything with named caps (output mpsc 1024, broadcast 256, input 256, mailbox 4,
forwarders 16, ring 5000 lines, scrollback 256 KB/proc + 16 MB global drop-oldest); double-start
atomically claimed; locks released on every terminal path. **MCP identity is intact** — the
actor injects `SOLOIST_PROCESS_ID` into `launch.env` (`actor.rs:161-163`) even though
`launch_agent` builds empty env, so the earlier "no env" worry is a non-defect.

### C1 — Freshly-launched agent shows an empty "not-started" pane (THE #1 daily bug) — `VERIFIED` (P1)
> Root cause is a two-part cross-boundary race (I re-checked every cited line):
- **(a) backend, lazy channel:** `terminals.open(id)` is called **only inside the actor task**
  (`actor.rs:177` — confirmed sole caller). `register`/`start`/`launch_actor` never pre-create
  it. So between `start()` returning and the actor being scheduled, `attach_pty` →
  `terminals.attach` returns `None` (`terminal.rs:209-215`), and `pty_attach` rejects the invoke
  with `"process has not started"` (`commands/mod.rs:252`).
- **(b) frontend, fragile retry:** `useTerminal.attach()` sets `attachedRef.current = true`
  **optimistically before** the async attach resolves (`useTerminal.ts:83`); on rejection it
  resets to `false` + `setState("not-started")` (`:164-168`). The **only** retry trigger is the
  effect keyed on `[process.status]` guarded by `!attachedRef.current` (`:281-283`). Interleaving:
  mount(`Starting`)→attach sets ref true→`Running` event fires the effect, sees ref still true→
  **skips**→the earlier attach rejects→ref false, state `"not-started"`→status never changes
  again→**no retry**. A live agent producing output renders the *"This process hasn't started
  yet. Press Start"* overlay; Start on a running process is a no-op → stuck until re-select
  (remount). Intermittent → "daily."
- **Contract violated:** "a state change with no event → permanently stale UI" (a live Running
  process shown as not-started).
- **Fix:** create the `TerminalChannel` synchronously in `register`/`launch_actor` before
  spawning the actor (so `attach_pty` never returns `None` for a registered process — makes the
  FE race moot); additionally drive the FE retry off attach-resolution/`state`, not
  `attachedRef` + status alone. → **own PRD (highest priority).**

### C2 — Recycled-PID kill on orphan reconciliation (Solo v0.9.3 bug class, present) — `VERIFIED` (P1)
> Confirmed: `OrphanRecord` (`ports/mod.rs:297-302`) persists only `{project_root, name,
> command, pgid}` — **no boot-id, no start-time**; `is_alive` (`pty/lib.rs:248-251`) is bare
> `killpg(pgid, None)`.
- On relaunch after a crash/force-quit + PID churn/reboot, an unrelated same-user process group
  can hold the recorded `pgid`. `reconcile_orphans` judges liveness purely by `is_alive(pgid)`.
  Two bad outcomes: (i) **surface/kill path** — `kill_orphan` SIGKILLs the bare recorded pgid
  (`reconcile.rs:70-73`) with no identity check; the user's "Kill" then SIGKILLs an unrelated
  group; (ii) **adopt path** — if the recycled group matches a resting registered command by
  `{project_root,name,command}`, it is adopted and later stop/shutdown SIGKILLs it.
- **Nuance (from the core-test audit):** the `{project_root,name,command}` match narrows the
  *adopt* risk, but the *kill-by-bare-pgid surface path is unguarded*, and two runs of the same
  command in the same project share identity. So the risk is real.
- **Fix:** stamp `OrphanRecord` with the group leader's start-time (`/proc/<pid>/stat` field 22)
  and/or boot id (`/proc/sys/kernel/random/boot_id`); require a match in `is_alive`/classify
  before adopting or offering to kill. → **own PRD.**

### C3 — Illegal FSM transitions silently dropped, untraced — `AGENT` (P2)
- `apply_transition` returns the old state and publishes **nothing** on `Err(_)`
  (`supervisor.rs:497-509`); no log, no event. A future regression attempting an illegal edge is
  invisible and can desync the registry/actor mirror with no UI event. → add `tracing::warn!` /
  diagnostic event on the `Err` arm. (Contract: "illegal transition = failure, traced.")

### C4 — `stop()` during the launch window is silently lost but returns `true` — `AGENT` (P2 race)
- `launch_actor` sets the actor handle **after** `begin_launch` moved the process to `Starting`,
  no await between (`supervisor.rs:438-456`). `stop()` treats `Starting` as active, tries
  `registry.mailbox(id)`; if the handle isn't set yet it sends nothing **but returns `true`**
  (`supervisor.rs:235-245`). A `stop`/`stack_stop` in that window is dropped while reporting
  success; the process keeps running. → set the handle inside the `begin_launch` critical
  section, or buffer a pending-stop flag.

### C5 — `shutdown()` can miss a process mid-launch → orphan on quit — `AGENT` (P2, same root as C4)
- `shutdown` reaps via `with_live_actor()` (handle-is-some) and breaks when a pass yields no
  joins (`supervisor.rs:402-416`). A `begin_launch`'d-but-not-yet-`set_handle` process is
  invisible; if its handle lands after the final pass, its child spawns after `shutdown` returns
  → leaks past quit. Violates deterministic-shutdown / no-orphans-on-quit. Narrow window.

### C6 — Initial/relaunch resize can be dropped; PTY left at 80×24 — `AGENT` (P2)
- The input pump drops `Write`/`Resize` while `current_io` is `None` (`actor.rs:397-403`), and
  `current_io` is set **after** `Running` is announced and cleared on every restart. A resize in
  those windows is discarded; every spawn/restart starts at `PtySize::default()` 80×24 (the
  actor's `launch.size` is never updated). The FE compensates via `ResizeObserver` + the status
  effect, so usually masked — but fragile (a dropped-and-never-retriggered resize leaves the
  agent mis-sized: gaps on the right/bottom). → set `current_io` before announcing `Running`;
  remember last-known size to re-apply on respawn. **Related to the empty-pane symptom class.**

### C7 — Agent "installed" detection uses a different PATH than launch — `AGENT` (P2)
- `runs_version_ok` probes with bare `Command::new(command).arg("--version")` (inherited PATH,
  `sys/agents.rs:59-66`) while launch runs `$SHELL -lc <command>` through the captured login-shell
  env (`pty/lib.rs:79-86`). An agent CLI installed only via nvm/asdf/volta is badged **not
  installed** yet launches fine (or vice-versa). → probe through the login shell for parity.

### C8 — Lagged reactor doesn't clear stale restart-windows — `AGENT` (P2 minor)
- On `RecvError::Lagged` the reactor only `rescan_crashed()` (`restart.rs:226-231`);
  `Stopped`/`Removed` clears are missed during the lag, so a stopped-then-restarted process can
  carry a stale crash count. Bounded (one entry/process), self-heals on next stop event.

## D. MCP + coordination + store security (COMPLETE — 2nd batch)

**Headline: no P0/P1 trust-bypass or wrong-project *write* is reachable.** The security core is
solid: trust gate enforced in `core` on every start/restart/resume path (`supervisor.rs:419`
`guard_trust`), MCP writes scope to `effective_project` in core, cross-project transfer doubly
authenticated, SQL fully parameterized (`?N` only), revision guards atomic under one connection
mutex, leases/locks correct (TTL clamp, owner-only release, launch reconcile), `solo.yml` 1 MiB
+ `deny_unknown_fields` + required `command`. The findings below are P1(one)/P2/hardening.

### D1 — Cross-project READ disclosure via unscoped MCP read tools — `AGENT` (candidate P1 security)
- **Code:** `crates/app/src/ipc_server.rs` read handlers take a bare `process` id with **no
  session/scope check** — `GetProcessRawOutput` (:234, full raw scrollback), `GetProcessOutput`
  (:230), `SearchOutput`/`SearchRawOutput` (:238/:246), `GetProcessStatus`, `GetProcessPorts`,
  `ListProcesses` (:169 → `facade.snapshot()` = every process in every project).
- **Impact:** an MCP agent bound to project A can enumerate and read the **full terminal
  scrollback of project B's agents/commands** — which can contain secrets, tokens, file
  contents typed into another agent's PTY. Same info-disclosure class as A1 (HTTP reads).
- **Status:** documented as intentional ("read tools open by design," `facade/scoped.rs:267`;
  D-6 authenticates only *action* tools) — **but D-6's rationale covers stop/restart/clear, not
  the disclosure risk of cross-project raw-output reads.** Needs an explicit owner decision:
  keep open (record why in `plan/05`/`KNOWN-DIVERGENCES.md`) or scope reads to the caller's
  project. Flagged P1 pending that decision because it crosses the project-isolation boundary.

### D2 — `working_dir` has no project-root containment — `AGENT` (P2 hardening / Solo fidelity)
- **Code:** `crates/core/src/config/model.rs:119` `resolved_working_dir = project_root.join(dir)`;
  `crates/pty/src/lib.rs:82` `builder.cwd(&spec.working_dir)` verbatim. `working_dir: /etc`
  (absolute replaces root) or `../../etc` escapes the project root. No `canonicalize` /
  `starts_with(root)` guard (containment exists only for file-watch globs, `filewatch/policy.rs`).
- **Mitigation:** trust-gated **and** `working_dir` is in the variant hash (`variant_hash`,
  model.rs:97), so an edit → new **untrusted** variant the user must re-approve (trust screen
  shows the dir). Not an unauthenticated escape.
- **Contract:** Solo v0.9.3 requires `working_dir` resolve inside the project; Soloist enforces
  nothing and there's **no recorded decision** declining it (`plan/05`, `KNOWN-DIVERGENCES.md`).
  → add a containment guard as defense-in-depth, or record the divergence.

### D3 — No per-payload/per-field size cap below the 8 MiB frame → unbounded `soloist.db` — `AGENT` (P2)
- **Code:** only ceiling on any coordination write is `MAX_FRAME = 8 MiB` (`crates/ipc/src/frame.rs:14`).
  `kv_set` (`store/src/kv.rs:15`, arbitrary JSON), `scratchpad_write`, todo docs, `timer_set`
  body — none length-validated, record counts unbounded. A misbehaving in-project agent grows
  the SQLite DB without bound. Contradicts `plan/05 §7` ("kv = small structured state, not
  logs") and CLAUDE.md §8 "bounded everything."

### D4 — Blocking `rusqlite` on the tokio runtime; no `busy_timeout` — `VERIFIED` (P2 contract deviation)
> Confirmed: `store/src/lib.rs:40` `conn: Mutex<Connection>` (a `std::sync` mutex); `configure()`
> (lib.rs:91-95) sets WAL + foreign_keys but **no `busy_timeout`**; store calls run inline.
- **Code:** `crates/store/src/lib.rs:39` single `Mutex<Connection>`; every store call runs
  `rusqlite` **inline** in async tasks (`ipc_server` `handle_request`, Tauri handlers), never via
  `spawn_blocking`. No `busy_timeout` pragma (lib.rs:46-95).
- **Contract:** CLAUDE.md §8 requires "spawn_blocking (no blocking calls on the tokio runtime)."
  Mitigated (single conn serializes, mutex never held across await, sections tiny) but a WAL
  commit `fsync` still blocks a runtime worker. This is the `load_project`-blocking class writ
  large. → move store calls off the runtime (spawn_blocking or a dedicated DB thread/pool).

### D5 — MCP scope-revert-on-reconnect is fail-closed, NOT a wrong-project write — `AGENT` (P3 informational)
- `crates/mcp/src/client.rs:104-113` re-binds on reconnect; `ipc_server.rs:125/141` opens a
  fresh session per connection, so a prior `select_project` is lost. But `select_project` is
  constrained to the caller's *home* project (`facade/session.rs:73`), and re-bind restores
  `effective_project` to that same home project — worst case a fail-closed `NoProjectScope` for
  an external caller that relied on the sole-project default. App-restart is moot (quitting kills
  the managed agent + its `soloist-mcp` child). Downgrades the earlier audit's P1 concern.

### D6 — Peer auth relies solely on the `0700` dir; no uid assertion — `AGENT` (P3 hardening)
- `crates/app/src/peer_cred.rs` reads `SO_PEERCRED` pid → `getpgid` but doesn't assert peer uid
  == app uid. Confinement rests on the `0700` data dir. A `uid == getuid()` check is cheap
  defense-in-depth. (pid→pgid TOCTOU is correctly fail-closed.)

### D7 — Trust hash excludes `auto_start`/`auto_restart`/`restart_when_changed` — `AGENT` (documented divergence, not a bypass)
- `variant_hash` (model.rs:97) covers command+working_dir+env only (matches CLAUDE.md §3 D-1's
  narrower list). An external edit flipping `auto_restart: true` on an already-trusted command
  keeps trust and gains auto-management — but every relaunch still runs the *same trusted
  command*. Diverges from `plan/05 §4/§12` (which lists these as re-trust triggers). No code-exec
  escalation. → reconcile the docs or widen the hash.

## E. UI + Tauri + feature-wiring (COMPLETE — 2nd batch)

**Conformance PASSES (verified in code):** all 74 `#[tauri::command]`s are thin, route to
`Facade`/`supervisor`, hold no domain `if`, do no direct fs/process work, and each is registered
+ has a matching `api.ts` invoke (no orphan/missing command); all UI IPC flows through `api.ts`
(no bypass); Tauri security is tight (strict CSP `default-src 'self'`, `connect-src` = ipc only,
`object-src 'none'`, `frame-ancestors 'none'`; minimal window-scoped capabilities; devtools/
agent-bridge/tokio-console opt-in, absent from default; single-instance plugin first; updater
pubkey + manual); core supervision flows wired end-to-end; notification reactor fires on crash/
restart-exhausted/permission/error; listener lifecycle correct everywhere (cancelled-flag +
unlisten, no StrictMode double-subscribe); `domain.ts` enums match Rust, discriminated-union
comparisons type-checked, `RESTART_LIMIT` a named const; scoped hotkeys really dispatched.

### E1 — Editing a shared command silently DELETES its `env:` from `solo.yml` — `VERIFIED` (P0 data loss)
> Full chain re-verified line-by-line by the main session:
- `ProjectCommandView` (`domain.ts:586-596`) and the editor's `CommandFields`/`buildSpec`
  (`components/project-settings/spec.ts:4-22`) have **no `env`** — "this surface neither shows
  nor edits a command's environment."
- Edit routes the env-less spec to core (`ProjectSettingsPane.tsx:81`) → `edit_shared_command`
  does `config.processes.insert(name, spec)` (`facade/commands.rs:190`), so `intended`'s spec has
  `env = {}`.
- `ProcessSpec.env` is `#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]`
  (`config/model.rs:74-75`) → empty env is omitted on serialize.
- The writer marks the entry `updated` (its spec differs) and **re-renders the body** via
  `entry_lines`, replacing the original lines (`config/edit.rs:93-101`); the omitted env means the
  committed `env:` block is **dropped**. The rename-verbatim safety (`edit.rs:84-88`) doesn't
  help — the env-less spec never equals the on-disk spec, so it always takes the re-render path.
- **Effect:** a user with `env: {…}` on a command who edits *any* field of it (toggle auto-start,
  change the command text, add a watch glob) — or renames it — silently loses the entire
  committed `env:` block on save. Reachable through normal UI; destroys version-controlled config.
- **Contract:** CLAUDE.md §3 "Never silently rewrite the user's `solo.yml`"; `phase-11a` "safe
  round-tripping (no silent rewrite)." Phase 11a is `Done — pending verify`, so this is a
  **pre-release defect**, not a shipped regression — but it is a P0 to fix before that verify.
- **Fix:** carry `env` through the read model + editor (round-trip it untouched even if not
  editable), OR have `edit_shared_command` **merge** onto the existing spec's env rather than
  replace. → **own PRD (P0).**

### E2 — Per-project "Crash & exit alerts" toggle is decorative — `AGENT` (P1 wiring)
- `crash_exit_alerts` is stored + displayed but **never consulted** by the reactor, which gates
  on a single global flag `if !self.enabled { continue }` (`notify/reactor.rs:60`) and fires crash
  toasts with no project-settings reference (`:77-84`). No consumer beyond its setter. Worse, the
  global `enabled` flag (`facade.rs:141` default true; setter `:177`) is **never called by any
  adapter** — so notifications can't be disabled anywhere at all.

### E3 — Per-project "Terminal alerts" toggle is decorative; no TerminalBell→notification path exists — `AGENT` (P1 wiring)
- `terminal_alerts`/`terminal_alerts_for()` feed only the display read model; the reactor's
  `compose` (`notify/reactor.rs:75-105`) handles only crash/exhausted/permission/error and
  **ignores `TerminalBell` entirely**. The switch gates nothing. (`TerminalBell` IS emitted by
  core and folded by the projection, but nothing turns it into a notification.)

### E4 — Auto-summarization opt-in is decorative — `AGENT` (P1 wiring; mitigated: E6 is later/OFF)
- `AgentsPanel.tsx:69-96` presents a functional summarizer tool+model opt-in persisted via
  `set_agent_settings`, but the `Summarizer` port is an **empty trait with no methods**
  (`ports/mod.rs:370`), isn't a `CorePorts` field, and is never invoked — no summarizer loop
  exists. Choosing a tool produces nothing. Mitigation: E6 is a `later`/OFF-by-default row, so
  most users never touch it — but the UI reads as functional.

### E5 — MCP/HTTP master integration toggles decorative — `VERIFIED` (P1, known-deferred, confirmed)
- `set_integration_settings` persists `mcp_enabled`/`http_api_enabled` (`commands/settings.rs:140`)
  but `lib.rs` spawns the MCP IPC server (`:291`) and HTTP server (`:305`) **unconditionally**
  under cfg features; `integration_settings` is never read outside its own command. (Matches the
  HTTP audit A1-adjacent + PROGRESS.md known-deferred.)

### E6 — A cluster of Sidebar settings are decorative — `AGENT` (P1/P2, known I7g gap)
- Of ten `Sidebar` fields only two take effect (`hide_empty_sections`, `show_settings_footer`).
  Persist-only, no consumer: `process_cpu_threshold`/`process_mem_threshold`/`project_cpu_threshold`/
  `project_mem_threshold` (`ProcessMeta.tsx:62-68` shows CPU/RSS/ports whenever Running, ignoring
  thresholds); `show_filter_input` (no filter input rendered); `project_open_in_editor`/
  `_in_terminal`/`_reveal_in_file_manager` (no such context actions exist). PROGRESS.md records
  I7g partial-Verify, but these are user-visible controls that do nothing.

### E7 — SignalsProvider has no reconcile/snapshot backstop → permanently stale idle badge — `AGENT` (P2, broader than the known reload gap)
- `SignalsProvider.tsx:14-27` subscribes to `onDomainEvent` only — no `onResync`, no focus
  refresh, no snapshot seed. On a `DOMAIN_RESYNC` (bus lag/drop) a dropped `AgentActivityChanged`
  leaves a **permanently stale idle badge** (edge-triggered; re-emitted only on the next
  transition). Metrics self-heal (~1 Hz), attempts on status change; activity does not. This is
  broader than the pre-flagged "webview-reload snapshot seed" — the lag/drop reconcile path is
  also uncovered. Also: a `ProcessRemoved` dropped during lag leaks that id's entries.

### E8 — `useLineage` and `useOrchestration` lack the `useReconcile` backstop — `AGENT` (P2)
- Both re-read on specific event types but neither calls `useReconcile` (no `onResync`, no focus).
  On a dropped delta the sidebar worker-nesting and the orchestration board (todos/timers/
  scratchpads) stay stale until the next relevant event. `useProcesses`/`useProjects` got the
  backstop in the sprint; these two didn't.

### E9 — `useOrphans` swallows the resolve rejection → user misinformed a leftover was reaped — `AGENT` (P2)
- `useOrphans.ts:41,49` `void orphansResolve(...).catch(() => {})`. A failed orphan SIGKILL is
  invisible and the row disappears optimistically regardless of outcome — the user believes a
  still-running leftover group was killed. (Ties to C2.)

### E10 — Assorted P2/minor — `AGENT`
- Orchestration snapshot carries `leases`+`kv` but `useOrchestration.ts:119-127` drops both — no
  UI surface for raw lease (G6)/kv (G10) state (orchestrator track still in progress).
- `useOrchestration` has no generation guard on same-project refreshes (rAF-coalesced; a slow
  frame-N snapshot can briefly show a stale board).
- SIGTERM to the app runs no in-process reap — only window-close/tray-Quit run
  `supervisor().shutdown()` (`lib.rs:394-404`); SIGTERM has no handler (mitigated by next-launch
  orphan reconciliation, but see C2 for why that path is itself risky).
- Stale comment `useGlobalHotkeys.ts:6-9` (doc-accuracy only).

## F. Core + MCP test honesty — see the §B summary table + coverage-holes list above (99.2% real).

---

## Cross-cutting notes (main session)

- **Empty-xterm-on-new-agent (user's #1 daily symptom):** the *wiring* is structurally intact —
  `launch_agent` (`facade.rs:298`) registers + starts an `Agent`-kind process, `register`
  publishes `ProcessSpawned` (`supervisor.rs:185`), the projection folds it into the list
  (`projection.ts:14`), the pane mounts when the process appears and `pty_attach`
  (`commands/mod.rs:242`) replays raw scrollback then streams live bytes. So it is a **runtime**
  defect, not a missing wire — most likely PTY-size/attach-timing or agent-CLI-on-spawn behavior.
  Needs a **live reproduction** to root-cause (consistent with PROGRESS.md: phases 5/6/8 never
  had runtime acceptance walks). → its own PRD (reproduce-first).
- **Agent spawn injects no env** (`facade.rs:319-326`, `env: BTreeMap::new()`) — deliberate (no
  credential injection) but means **no `SOLOIST_PROCESS_ID`**; MCP identity must therefore rely
  entirely on peer-pgid over the socket. Verify the 2nd-batch MCP agent confirms binding works
  for a launched agent whose only identity is its process group.
