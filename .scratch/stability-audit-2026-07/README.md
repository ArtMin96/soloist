# Stability & Security Audit — 2026-07-13

Full read-only review of Soloist for **bugs, security gaps, leaks, broken/half-wired features,
and test honesty**, requested by the owner ("hitting issues all day long… make sure everything
works as it should… no pretending"). Baseline: `main` @ `15bdd1a` (v0.5.0), after the
2026-07-13 stability-hardening sprint.

## Layout (local Markdown issue tracker — see `docs/agents/issue-tracker.md`)
- **`00-findings-log.md`** — the evidence ledger. Every finding with `file:line`, a concrete
  failure scenario, the contract it violates, and a status tag (`VERIFIED` = the main session
  re-checked the code; `AGENT` = reported, awaiting re-verification; `KNOWN-DEFERRED`).
- **`issues/NN-<slug>.md`** — one agent-ready ticket per **session-sized workstream** (owner's
  explicit ask: implement one per session). Each is self-contained: problem, contract reference,
  root cause, fix approach, test plan, acceptance, out-of-scope. Each carries a `Status:` (triage
  role) and a `Blocked by:` line near the top.
- **`README.md`** (this file) — index + locked decisions. There is no separate `spec.md`; each
  ticket is its own spec.

## What the audit found (headline)
- **The code is structurally sound.** The security core (trust gate in core, MCP project-scope
  isolation, parameterized SQL, atomic revision guards, lease correctness), the supervisor
  invariants (10/60s restart cap, SIGTERM→SIGKILL group reap, no zombies, bounded everything), and
  the Tauri surface (strict CSP, minimal capabilities, single-instance) all check out. No P0/P1
  trust bypass or wrong-project **write** is reachable.
- **The test suite is genuinely healthy — the "pretend tests" worry does not hold.** 1040 tests
  classified across every crate: **~98.7% exercise real behavior**, **0 over-mocked**, only 12
  trivial tautologies + 2 prose smokes. Core+MCP (the domain brain) is 99.2% real. The daily
  instability comes from **runtime bugs**, not fake tests hiding broken features.
- **The daily pain is a small set of real, fixable defects** — one P0 data-loss, two P1 runtime
  races (incl. the empty-agent-pane you hit), and a batch of half-wired settings. All below.

## Ticket index (priority order — one workstream per session)

| # | Workstream | Severity | Blocked by | Key findings |
|---|-----------|----------|-----------|--------------|
| [01](issues/01-soloyml-env-roundtrip.md) | Stop deleting `env:` on command edit | **P0** data loss | — | E1 (VERIFIED) |
| [02](issues/02-terminal-attach-empty-pane.md) | Empty new-agent pane race (your #1 symptom) | **P1** | — | C1 (VERIFIED), C6 |
| [03](issues/03-orphan-pid-reuse-safety.md) | Orphan PID/PGID-reuse kill safety | **P1** | — | C2 (VERIFIED), E9 |
| [04](issues/04-notification-toggles.md) | Notification toggles actually gate + bell path | **P1** wiring | — | E2, E3 |
| [05](issues/05-settings-integrity.md) | No decorative settings (summarizer/MCP-HTTP/sidebar) | **P1** wiring | — | E4, E5(V), E6 |
| [06](issues/06-local-read-authorization.md) | HTTP unauth reads + MCP cross-project reads | **P1** security | — | A1(V), D1, A2 |
| [07](issues/07-reconciliation-and-launch-races.md) | Finish reconciliation + actor launch races | P2 | 02 | E7, E8, C3, C4, C5 |
| [08](issues/08-store-off-runtime-bounded.md) | SQLite off the runtime + bounded payloads | P2 | — | D4(V), D3, load_project |
| [09](issues/09-hardening-and-fidelity.md) | working_dir/peer-uid/trust-hash/PATH/CLI hardening | P2/P3 | — | D2, D6, D7, C7, A3–A6 |
| [10](issues/10-test-coverage.md) | Close the real coverage holes | P2 tests | 06 | B1–B5 + holes |

`(V)` = re-verified against the code by the main session, not just agent-reported.

**Blocking rationale:** `07` waits on `02` (both touch `supervisor.rs` register/actor/launch — land
02's synchronous-channel fix first). `10` waits on `06` (its HTTP auth tests must target 06's new
per-launch-token scheme; 06 writes the core auth tests). Everything else is independent — the
numbering is the priority order.

## Owner decisions (locked 2026-07-13 — baked into the PRDs)
- **PRD-06 MCP reads:** scope cross-project reads to the caller's project (refuse/redact
  out-of-scope output; `list_processes` keeps identity-only rows for other projects).
- **PRD-06 HTTP reads:** per-launch random token required on ALL routes (reads + mutations) via a
  `0700` discovery file + a `Host`-header guard; supersedes the constant-`"1"` header — update
  `plan/05`.
- **PRD-05 summarizer UI:** hide/disable the opt-in (don't build the loop; E6 is `later`).
- **PRD-05 MCP/HTTP master toggles:** LIVE teardown/spin-up on toggle (no restart) — larger scope
  than startup-gating; may warrant its own session.

## How to work these — one command per session
Each new session (fresh context), on branch `fix/stability-audit-2026-07`, run:

```
/work-ticket
```

That project skill (`.claude/skills/work-ticket/`) does the whole per-session loop for you: runs
Soloist's start protocol (`CLAUDE.md §1`), picks the next open + unblocked ticket by the frontier
rule (lowest number whose `Blocked by:` are all done), claims it, implements it test-first via
`/implement` (tdd → gates → `/code-review` → commit), then flips its `Status:` and updates
`PROGRESS.md`. To work a specific one instead: `/work-ticket 04` (or a path). Then **clear context**
and run it again next session for the following ticket.

Frontier order it will follow: 01 → 02 → 03 (the P0 + the two daily-pain P1 runtime bugs) → 04 →
05 → 06 → 07 (after 02) / 08 / 09 / 10 (after 06). `02` and `08` finish as `needs-human-verify`
(they need a live-app check); `05` (live server teardown) and `06` carry the most design surface.
