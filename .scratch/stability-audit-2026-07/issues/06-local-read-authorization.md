# PRD-06 — Close the local read-disclosure surface (HTTP unauth reads + MCP cross-project reads)

Status: done
Blocked by: none

- **Severity:** P1 security (local information disclosure of another user's / another project's
  process output, which can contain secrets/tokens)
- **Area:** `crates/httpapi` (routes/auth/cors), `crates/app/src/ipc_server.rs`,
  `crates/core/src/facade` read surfaces
- **Evidence:** A1 (HTTP unauth reads) VERIFIED; D1 (MCP cross-project reads) AGENT-reported +
  corroborated by the security audit; A2 (Host validation) AGENT.
- **NOTE — needs an owner decision first (see Open question).**

## Problem
Three overlapping local-read exposures:
1. **A1 (VERIFIED):** the HTTP read routes are completely **unauthenticated**
   (`crates/httpapi/src/routes.rs:26-37` — no auth layer; only mutations get `require_local_auth`).
   The mutation gate compares against the compile-time constant `"1"` (`crates/ipc/src/http.rs:28`),
   so it is CSRF protection, not authentication. On the multi-user Ubuntu target (D2) any local
   process/UID can `GET /processes/:id/output` and read another user's process logs.
2. **D1:** the MCP read tools take a bare process id with **no scope check**
   (`ipc_server.rs` `GetProcessRawOutput`/`GetProcessOutput`/`Search*`/`ListProcesses`), so an
   agent bound to project A can read the **full raw scrollback of project B's** agents/commands.
   Documented as "open by design" (D-6) — but D-6's rationale covers action tools, not the
   disclosure risk of cross-project raw-output reads.
3. **A2:** no `Host`-header validation → a DNS-rebinding page can make same-origin reads (CORS
   never applies) and set the constant auth header freely.

The scheme choice (header vs Solo v0.9.3's rotating bearer token) is a **recorded owner decision**
— do NOT relitigate it. The defects are: reads have zero gate, there's no per-user boundary on
HTTP, and MCP reads cross the project-isolation boundary.

## Decisions (owner-confirmed 2026-07-13 — implement these, not options)
- **HTTP:** add a **per-launch random token required on ALL routes** (reads + mutations), written
  to a `0700` discovery file the CLI reads. Upgrades mutations from CSRF-only to real auth and
  closes the multi-user read exposure. Plus a `Host`-header guard (A2).
- **MCP reads:** **scope reads to the caller's project.** Refuse/redact `Get*Output`/`Search*`/
  `GetProcessStatus`/`GetProcessPorts` for out-of-scope processes; keep `list_processes`
  cross-project but redact out-of-scope rows to identity (name/status), no output.

## Fix approach
- **HTTP reads (per-launch token):** replace the constant `"1"` with a per-launch random token,
  written to a `0700` discovery file (extend the existing runtime file the CLI already reads).
  Require it on the **whole** router — move reads behind the same auth layer as mutations, now
  keyed on the real token. Update the CLI + UI clients to send it. Add a `Host`-header guard
  (reject `Host` ≠ `127.0.0.1`/`localhost[:port]`) to kill the DNS-rebinding path (A2). Use a
  timing-safe comparison now that the value is a secret.
- **MCP reads (project-scoped):** in `ipc_server.rs`, resolve the caller's `effective_project`
  (already available for write tools) and refuse/redact `Get*Output`/`Search*`/`GetProcessStatus`/
  `GetProcessPorts` for a process outside it; route through a scoped façade read so the rule lives
  in core (one behavior, many frontends), mirroring how writes already scope. `list_processes`
  stays cross-project but redacts out-of-scope rows to identity only.

## Test plan (must fail before, pass after)
- **HTTP:** every read route returns 401 without the token (closes audit test-gap B1/B2 —
  currently **no** read has an auth test); a foreign/absent `Host` is rejected; the token is
  per-launch and not the constant `"1"`.
- **MCP:** an agent scoped to project A gets a refusal/redaction on `get_process_raw_output` for a
  project-B process (core-level test like the existing `OutOfScope` write tests); in-scope reads
  still succeed.
- **Trust-gate HTTP test (audit gap B1):** while here, add the missing HTTP 403 test — POST
  `start` on an untrusted command returns 403.

## Acceptance
- HTTP: every route (read + mutation) requires the per-launch token; no route is reachable
  without it; the token is a per-launch secret in a `0700` file, compared timing-safely; a
  foreign/absent `Host` is rejected. MCP: an out-of-scope process's output/search/status/ports is
  refused or redacted; in-scope reads still work; `list_processes` shows out-of-scope rows as
  identity only. `just test` + `just lint` green.
- This changes the recorded HTTP scheme (constant header → per-launch token) — update `plan/05`
  and `KNOWN-DIVERGENCES.md` to reflect the new decision (supersedes the old `X-Soloist-Local-Auth`
  constant-header note).

## Out of scope
Rotating the token mid-session / bearer-refresh (Solo v0.9.3's fuller scheme) — a per-launch token
is sufficient here. Rate limiting (PRD-09).

## Comments

**Done — 2026-07-14, impl commit `4c63170` (branch `fix/stability-audit-2026-07`).**

What changed:
- **HTTP (A1/A2, per-launch token):** `ipc::http` mints a fresh 32-byte hex token per launch
  (`generate_token`, `getrandom`), written into the runtime file (`http-api.json`) which is now
  created **owner-only `0600`** inside the already-`0700` data dir. `HttpRuntime` carries `{ port,
  token }`. The token is required on the **whole** router (reads + mutations) via a `require_token`
  middleware comparing in constant time (`subtle`); a `require_local_host` middleware rejects a
  non-loopback `Host` with 403 (A2). CORS + the Host guard share one `host::host_is_loopback` rule.
  `LOCAL_AUTH_VALUE` (`"1"`) is gone. The CLI reads the token from the runtime file and sends it on
  every request; `serve()` fails closed if OS randomness is unavailable.
- **MCP (D1, project-scoped reads):** `get_process_output`/`_raw`/`search*`/`get_process_status`/
  `get_process_ports` (and `wait_for_bound_port`) now route through scoped wrappers in
  `core::facade::scoped` that `require_in_scope` → refuse an out-of-scope process (`OutOfScope`);
  `list_processes` uses `snapshot_scoped`, redacting out-of-scope rows to identity via
  `ProcessView::redacted_identity`. The unscoped accessors stay for the local UI and the (now
  token-authed) HTTP API.

**Owner-decision note:** the ticket says a "0700 discovery file"; realized as a **`0600`** file
(canonical owner-only file mode; `0700` would set a meaningless execute bit on a JSON secret) inside
the already-`0700` data dir — same "unreadable to other UIDs" boundary. Recorded in `KNOWN-DIVERGENCES.md`
D-17. Also scoped `wait_for_bound_port` (not in the ticket's explicit list but the same "ports"
disclosure class the acceptance says to refuse) after code-review flagged the residual.

Tests (red-before/green-after): ipc `generate_token` freshness/length + runtime round-trip + `0600`
mode; httpapi every-read-401-without-token, wrong-token-401, foreign/absent-Host-403, and the missing
**B1 trust-gate 403**; the real `soloist-cli` binary round-trips the token end to end
(`cli/tests/shell.rs`); core `read_tools_enforce_scope` (every scoped read refuses out-of-scope),
`snapshot_scoped_redacts_out_of_scope_rows_to_identity`, `redacted_identity_*`; adapter
`the_read_tools_refuse_an_out_of_scope_process_but_list_stays_cross_project` and
`wait_for_bound_port_on_an_out_of_scope_process_is_refused`.

Gates: **`just lint` exit 0** (fmt, clippy `-D warnings`, tsc, eslint, prettier, dep-direction;
file-size advisory only). **`just test`: Rust 932 passed / 0 failed, UI 306 passed.** `/code-review`
(Standards + Spec) ran clean — no hard violations, spec faithfully implemented; the two acted-on
findings (scope `wait_for_bound_port`; drop `(D2)` doc-citations) are folded into `4c63170`.
