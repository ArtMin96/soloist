# PRD-09 — Security hardening + Solo-fidelity + small correctness fixes

Status: done
Blocked by: none

- **Severity:** P2/P3 (defense-in-depth + doc fidelity + small UX correctness)
- **Area:** config/pty (`working_dir`), `crates/app/src/peer_cred.rs`, `crates/core/src/config`
  (trust hash), `crates/sys/src/agents.rs`, `crates/httpapi`/`crates/cli` (minor), plan docs
- **Evidence:** mostly AGENT-reported hardening; group into one cleanup session. Verify each before
  coding.

## Items

### H1 — `working_dir` project-root containment (D2) — P2
`resolved_working_dir = project_root.join(dir)` (`config/model.rs:119`), set verbatim into the PTY
cwd (`pty/src/lib.rs:82`). `working_dir: /etc` (absolute replaces root) or `../../etc` escapes the
project. Mitigated (trust-gated **and** `working_dir` is in the variant hash → new untrusted
variant on edit), so not an unauthenticated escape — but Solo v0.9.3 requires containment and
there's no recorded decision. **Fix:** add a `canonicalize`/`starts_with(root)` (or `components()`
`..`-rejecting) guard; refuse or clamp a `working_dir` that escapes. Record the fidelity item.

### H2 — Peer uid assertion (D6) — P3
`peer_cred.rs` reads `SO_PEERCRED` pid→pgid for scoping but doesn't assert peer uid == app uid;
confinement rests on the `0700` data dir. **Fix:** add a cheap `uid == getuid()` check as
defense-in-depth (fail-closed on mismatch).

### H3 — Trust-hash vs doc reconciliation (D7) — P2 (doc)
`variant_hash` covers command+working_dir+env only (matches CLAUDE.md §3 D-1), but `plan/05 §4/§12`
lists `auto_start`/`auto_restart`/`restart_when_changed` as re-trust triggers. No code-exec
escalation (a relaunch still runs the same trusted command). **Fix:** reconcile the docs —
either widen the hash to match `plan/05`, or amend `plan/05`/`KNOWN-DIVERGENCES.md` to the narrower
locked list. Owner decides; likely a doc fix.

### H4 — Agent "installed" detection PATH parity (C7) — P2
`runs_version_ok` probes with the inherited PATH (`sys/agents.rs:59-66`) while launch uses
`$SHELL -lc <command>` (`pty/lib.rs:79-86`). An agent installed via nvm/asdf/volta is mis-badged.
**Fix:** probe through the same login-shell env used for launch, so detection matches reality.

### H5 — HTTP/CLI minor (A3–A6) — P2/P3
- **A3:** no rate limit on `POST /projects/:id/spawn-agent` — add a simple per-interval cap (ties
  to PRD-06's local-caller threat).
- **A4:** CLI default-port fallback can address a foreign server if the runtime file is missing —
  verify server identity (`/health` version) before mutating, or refuse when the runtime file is
  absent.
- **A5:** `status` handler aggregates in the adapter (`routes.rs:56-72`) — move the `running` tally
  behind a façade read (route-to-facade discipline).
- **A6:** CLI `render_table` writes unescaped process labels (`command.rs:216-236`) — sanitize/
  escape control bytes so a crafted process name can't spoof the `soloist status` table.

## Test plan
- **H1:** a `working_dir` of `/etc` or `../../x` is refused/clamped (core test); an in-project
  relative dir still resolves.
- **H2:** a connection from a different uid is refused (adapter test with a fake peer cred).
- **H4:** detection uses the login-shell PATH (test via the shellenv seam already used elsewhere).
- **H5:** A3 rate-limit test; A5 façade-read test; A6 escaping test (a label with `\x1b[2J` renders
  inert).

## Acceptance
- Each item fixed with a test, or explicitly recorded as an accepted divergence in
  `plan/05`/`KNOWN-DIVERGENCES.md`. `just test` + `just lint` green.

## Out of scope
The core read-authorization decision (PRD-06). The blocking-store work (PRD-08).

## Comments

**Done — 2026-07-14 (impl commit `aedd202`, docs/ledger commit follows). All headless-verified.**
Each finding was re-checked against the code before coding; all fixed test-first (red→green).

- **H1** — `ProcessSpec::resolved_working_dir` now folds the dir onto the root lexically and
  clamps any escape (absolute path, or `..` above root) back to the root; a relative dir that
  stays inside (even via `..`) still resolves. Kept infallible to avoid rippling a `Result`
  through `Registration::command`'s ~30 call sites. Core test
  `working_dir_is_contained_within_the_root`. Fidelity recorded in `plan/05` §12.
- **H2** — `peer_cred::peer_pgid` reads the peer uid and **drops** the connection
  (`PermissionDenied`) when it is not Soloist's own uid; pure `peer_uid_permitted` unit-tested,
  fail-closed drop wired in `handle_connection`.
- **H3** — doc-only: `plan/05` §4 now records the narrow re-trust set and points at
  `KNOWN-DIVERGENCES.md` D-1 (already settled). No code change (widening the hash would break
  the CLAUDE.md §3 locked variant).
- **H4** — `CommandVersionProbe` probes `$SHELL -ilc <PROBE_SCRIPT> soloist-detect <command>`
  with the command as the shell positional `$1` (never interpolated → no word-split, no
  injection; `exec` for clean reap). Timeout raised to 3s (login-shell headroom, matching the
  env capture). Unit tests (`probe_command`, positional-not-shell-text) + real-OS integration
  test (deterministic via a stub `$SHELL`).
- **A3** — `ApiState` carries a fixed-window `SpawnRateLimiter`; `spawn_agent` returns **429**
  past the cap. Unit tests (cap + window rollover) + integration burst test.
- **A4** — `Client::from_runtime` returns `Result` and **refuses** ("Soloist is not running")
  when the runtime file is absent instead of probing `DEFAULT_PORT` with an empty token. Pure
  `from_runtime_opt` unit-tested.
- **A5** — `/status` routes to `Facade::status_summary` (`StatusSummary`), computing the
  `running` tally once in core; the adapter-local `Status` struct is gone. Core + route tests.
- **A6** — `render_table` sanitizes control bytes in the process label (`sanitize_cell` →
  `char::is_control()`), rendering a crafted name inert. Unit test.

**Code review** (adversarial subagent): cleared H1/H2/A3/A4/A5/A6/H3; found and I fixed an H4
command-quoting gap (unquoted interpolation → mis-detection + latent injection for a
spaced/metacharacter command — the positional-arg design above closes it) and one PRD tag in a
test comment. Accepted (documented) tradeoffs: the per-tool `-ilc` startup cost (bounded 3s,
cached 600s, off-runtime) and grandchild reaping on a hung probe (now `exec`-mitigated, and
consistent with the existing `shellenv` capture).

**Gates:** `just lint` exit 0 (fmt, clippy `-D warnings`, tsc, eslint, prettier, dep-direction;
file-size advisory only). `just test` — Rust **952 passed / 0 failed**, UI **306 passed / 61
files**. (Both verified against a tree containing only the ticket-09 changes — an unrelated,
pre-existing `__DIAG__` instrumentation in `useTerminal.ts`, the owner's live terminal-desync
debugging, was left untouched and excluded from the commit.)

**Next frontier: 10** (`ready-for-agent`; `Blocked by: 06`, which is `done` → unblocked). **07**
stays `Blocked by: 02` (`needs-human-verify`, not `done`).
