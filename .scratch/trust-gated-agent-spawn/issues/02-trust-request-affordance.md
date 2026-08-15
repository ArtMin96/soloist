# PRD-02 — Trust-request affordance: an agent asks, the user approves, with provenance and revoke (PR #169)

Status: ready-for-agent
Blocked by: 01

- **Severity:** P2 (new capability, owner-decided 2026-08-14; deliberately widens what an agent can
  cause to run, so the security work below is part of the feature, not a follow-up)
- **Area:** new `crates/core/src/trustrequest/`, `crates/core/src/events.rs`,
  `crates/core/src/ids.rs`, `crates/core/src/facade/scoped.rs`, `crates/core/src/facade.rs`,
  `crates/core/src/ports.rs`, `crates/store/src/migrate.rs`, `crates/store/src/trust.rs`,
  `crates/ipc/**`, `crates/app/src/ipc_server/dispatch.rs`, `crates/app/src/commands/`,
  `crates/mcp/src/args.rs`, `crates/mcp/src/tools/process.rs`, plus the UI half
- **Evidence:** VERIFIED in code (research session, 2026-08-14).
- **Depends on:** PRD-01 (`spawn_process`). PRD-01's refusal is the primary thing this unblocks.

## Problem

After PRD-01, `spawn_process` is gated on `TrustStore::is_trusted`, and trust is only writable by
`Facade::trust_command` / `trust_reviewed_command` (`crates/core/src/facade.rs:388, 401`), both of
which resolve a spec **by name out of the loaded `solo.yml`**. There is no API to trust an ad-hoc
command line. So an agent's usable set is exactly "command variants the user already approved" — and
an agent blocked by that has **no recourse**. The same dead end already exists today for
`start_process` on an untrusted `solo.yml` command (`ScopedActionError::Untrusted`,
`crates/core/src/facade/scoped.rs:117`).

This ticket adds the missing step: an agent **requests** approval for a command variant, the user
sees exactly what would run and approves or denies, and the agent learns the outcome.

## Decisions already made (do not re-litigate)

All three were put to the owner on 2026-08-14 and decided.

1. **Design B — a separate `request_command_trust` tool**, not an implicit request inside
   `spawn_process`. Reasons: (a) `spawn_process` stays a pure gated action with an unambiguous
   refusal — one behavior per command; (b) it generalises to `start_process` on an untrusted
   `solo.yml` command; (c) `reason` is the whole security mitigation and must be a **required**
   parameter, which is awkward to bolt onto one branch of `spawn_process`.

2. **Persistent grant + provenance migration + revoke — all three, as prerequisites.** A one-shot
   consuming grant was investigated and rejected on mechanical grounds: `Supervisor` holds
   `trust: Arc<dyn TrustRepo>` **directly** (`crates/core/src/supervisor.rs:79`) and `guard_trust`
   (`:252-263`) calls `self.trust.is_trusted(project, variant)` on **every** start/restart —
   including auto-start, crash auto-restart (`:153`), and file-watch restart. A consuming grant needs
   either a new consuming `TrustRepo` method (changing the port for every implementer including
   `FakeTrustRepo`) or a second collaborator inside `Supervisor`; either way a second trust concept
   lands in the hot gate, and **a file-watch restart silently eating the user's single grant is a bug
   nobody would diagnose.** So the grant is an ordinary durable `TrustRepo` row — which makes
   provenance and revoke prerequisites, not enhancements (see "Why provenance and revoke are not
   optional" below).

3. **Reachable by non-Agent bound processes.** This is what makes the feature generalise to
   `start_process`, and it is what forces `trust_request_status` to exist as the authoritative
   channel — verified: `coordination_owner` (`crates/core/src/facade/coordination.rs:235-243`)
   requires only a bound process and imposes **no kind gate**, while `mailbox_identity`
   (`crates/core/src/facade/mailbox.rs:372-378`) additionally requires
   `kind == ProcessKind::Agent`. A non-Agent requester therefore has no inbox at all.

## The pending-request aggregate

**Ephemeral, in memory.** A request is meaningful only while its requester is alive and this app run
continues. `plan/05-solo-reference-and-sources.md:470` clause (e) records that nothing survives an
app restart because closing the app stops all processes — so a request that outlived its run would
be an approval prompt for a command nobody is asking for any more, attributable to a dead process.
That is worse than losing it.

This is the split CLAUDE.md §8 already draws, and the one the mailbox uses: **the request is
ephemeral, the grant it produces is durable.**

**Reuse `TrustReviewCommand` for the payload — do not invent a parallel type.**
`crates/core/src/configchange.rs:52-60` already defines *"enough of the spec for the UI to show what
will run — command, working directory, and environment — before the user trusts it"*, carrying
`name`, `variant_hash`, `command`, `working_dir` (documented as the **raw** `solo.yml` value), and
`env`. Built by `TrustReviewCommand::from_spec` (`crates/core/src/config/review.rs:9-20`). A second
review type would be the DRY violation CLAUDE.md §15 names.

**Home.** A new `crates/core/src/trustrequest/` module beside `crates/core/src/trust.rs`. `trust.rs`
owns the *granted* half of the security concern at the crate root (not under a context directory);
this owns the *pending* half. Its vocabulary types go in the shared-kernel style, for the reason
`configchange.rs:5-11` states verbatim about `TrustReviewCommand`: `DomainEvent` carries them, so if
the owning context held them the bus and that context would import each other.

```rust
id_newtype!(TrustRequestId);  // crates/core/src/ids.rs — per-run monotonic,
                              // the AgentMessageId precedent at ids.rs:118-123

pub struct TrustRequest {
    pub id: TrustRequestId,
    pub project: ProjectId,
    pub requested_by: ProcessId,     // from coordination_owner — attribution
    pub review: TrustReviewCommand,  // reused; carries the pinned variant_hash
    pub reason: String,              // agent-supplied, REQUIRED, attacker-controlled
    pub expires_unix_millis: u64,    // Clock::now_unix_millis + TTL
}
```

**Dedupe key: `(project, variant_hash)`** — the same key the durable grant uses
(`crates/store/src/trust.rs:3`, `PRIMARY KEY (project_id, variant_hash)`). A second request for a
variant already pending returns the existing `TrustRequestId` rather than enqueuing a duplicate, so N
agents asking for the same command produce **one** prompt. The requester is recorded for attribution
but is deliberately **not** part of the key — otherwise a loop of agents could generate one prompt
each for the identical command.

## Bounds (CLAUDE.md §8 — a ceiling on everything)

Structure from the mailbox, expiry from leases.

| Bound | Value | Precedent |
|---|---|---|
| `MAX_TRUST_REQUEST_REASON_BYTES` | 4 KiB | `MAX_AGENT_MESSAGE_BYTES = 16 * 1024` (`crates/core/src/coordination/mailbox/vocabulary.rs:7`); smaller because a reason is one sentence, not a payload |
| `MAX_PENDING_TRUST_REQUESTS_PER_PROJECT` | 16 | `MAX_PENDING_MESSAGES_PER_PROJECT = 1024` (`vocabulary.rs:11`); **far** smaller because every entry costs a *human decision*, not memory — a queue of 1,024 approval prompts is the denial-of-service |
| `MAX_PENDING_TRUST_REQUESTS` (process-wide) | 64 | `MAX_PENDING_AGENT_MESSAGES = 4096` (`vocabulary.rs:13`) |
| `TRUST_REQUEST_TTL` | 10 min | `DEFAULT_LEASE_TTL = 5 * 60` (`crates/core/src/coordination/lease.rs:26`); longer because a human must notice it, short enough that a stale prompt does not sit overnight |

**Overflow → refuse, never evict.** The mailbox contract (`MailboxCapacityError`,
`vocabulary.rs:87-95` — "a queue ceiling that refused an enqueue without dropping an existing
message"). Here it is a security property, not tidiness: evicting to make room would let a flood of
requests silently displace one the user was about to read.

**Expiry: lazily on read**, as leases do (`crates/core/src/coordination/lease.rs:4-6` — "TTL expiry
(applied lazily on the next read)"). No timer task, no new supervised loop.

**Requester dies before approval → drop the request, via the `LockReleaser` port, NOT the event
bus.** The two owner-close precedents are not equivalent and the difference matters here. The mailbox
cleans up by riding `DomainEvent::ProcessRemoved` through its reactor
(`crates/core/src/coordination/mailbox/reactor.rs:103`), and that path can lag — the same loop
handles `Err(RecvError::Lagged(_))` with a snapshot reconcile (`reactor.rs:104`). Leases instead use
the `LockReleaser` port (`crates/core/src/ports.rs:279-282`), which the supervisor calls
**deterministically** whenever a process reaches a terminal state, adapted at
`crates/core/src/coordination/releaser.rs`. **For a security aggregate, take the deterministic
hook.** `CompositeLockReleaser` (`crates/core/src/ports.rs:293-315`) already exists to fan the single
supervisor hook out to several releasers, so this is **one more element in a `Vec` at the composition
root** (`crates/app/src/lib.rs::build_facade`), not a new mechanism.

A dropped request must also announce itself, so a dialog already on screen for a dead requester
closes rather than inviting the user to approve on behalf of a process that no longer exists.

## The round trip, hop by hop

| # | Hop | Where |
|---|---|---|
| 1 | Agent calls `request_command_trust(command, working_dir?, env?, label?, reason)` | new `#[tool]` in `crates/mcp/src/tools/process.rs` (it now covers both spawn and start refusals); arg struct in `crates/mcp/src/args.rs` |
| 2 | One `IpcRequest::RequestCommandTrust { … }` | new variant in `crates/ipc/src/protocol/request.rs` |
| 3 | One dispatch arm → one façade call | `crates/app/src/ipc_server/dispatch.rs`, shape per `:178-199` |
| 4 | `ScopedFacade::request_command_trust` | new. `project` from `coordination_scope()` (`crates/core/src/facade/scoped.rs:87-91`), `requested_by` from `coordination_owner()` (`scoped.rs:95-99`) — **the caller cannot assert either**. Builds the `ProcessSpec` with the caller's **raw** `working_dir` (see the hash trap below), runs `check_command` (`crates/core/src/config/model.rs:98`), short-circuits to `Granted` if `TrustStore::is_trusted` already says yes, else records the pending request |
| 5 | Core publishes `DomainEvent::TrustRequested { project, request }` | new variant in `crates/core/src/events.rs`, beside `ConfigChanged` (`:120-126`) which already carries `Vec<TrustReviewCommand>` for exactly this dialog |
| 6 | UI renders the approval dialog | `crates/app/ui/src/store/projection.ts` (its `ConfigChanged` case at `:66` is the precedent) → a component modelled on `crates/app/ui/src/components/TrustDialog.tsx` (136 lines) |
| 7 | User approves or denies | `Facade::approve_trust_request(id)` / `deny_trust_request(id)` — **on `Facade`, never `ScopedFacade`.** `crates/core/src/facade/attention.rs:4-7` states the rule outright: local-user state lives on `Facade` because "a session reaching the core over MCP is another agent, not the person at the keyboard". One Tauri command each, per `plan/06` §5.5 |
| 8 | Approval re-verifies the pin, then grants | resolve the request, **re-compute the variant hash from the stored spec and compare to `review.variant_hash`**, then `TrustStore::trust` (`crates/core/src/trust.rs:56`) + `Supervisor::mark_trusted` (`crates/core/src/supervisor.rs:247`) so the read-model `requires_trust` flag clears |
| 9 | Core publishes `DomainEvent::TrustRequestResolved { project, request, granted }` and removes the pending entry | `crates/core/src/events.rs` |
| 10 | The agent learns the outcome | two channels — below |

### How the agent learns: push AND poll — both are required

- **Push (best-effort):** on resolution, **if the requester is an Agent**, enqueue one message of a
  new `AgentMessageKind::TrustDecision` (`crates/core/src/coordination/mailbox/vocabulary.rs:19-24`,
  mirrored in `crates/app/ui/src/domain.ts`). Delivery follows the existing idle-gated wake path
  unchanged (`crates/core/src/coordination/mailbox/reactor.rs:94-99`). **Best-effort by
  construction:** a full mailbox or a non-Agent requester must never roll back or block the durable
  grant — the rule O16 already applies to completion notices (`plan/05:404`).
- **Poll (authoritative):** a `trust_request_status(request_id)` read tool returning
  `pending | granted | denied | expired`. This is the channel **every** requester has (decision §3),
  and the fallback when no wake arrived.

`request_command_trust` itself returns a **normal success** (`reply::structured`), not an error —
`{ request_id, state: "pending" | "granted" }` — because recording the request succeeded. `granted`
short-circuits when the variant was already trusted, so the agent never waits for a decision nobody
needs to make.

**Bonus, small and worth doing:** `crates/mcp/src/tools/reply.rs:57-60, :61-77` shows `refusal()`
lifts a structured detail's fields beside the discriminator. So `IpcError::Untrusted`
(`crates/ipc/src/error.rs:157`) can be enriched to carry the `variant_hash` it refused — then a
`spawn_process`/`start_process` refusal tells the agent exactly what to request instead of leaving it
to guess.

## Load-bearing warnings

### The hash must be pinned and re-verified at grant time

The `variant_hash` is computed at request time from `(command, raw working_dir, env)`
(`crates/core/src/config/model.rs:133-151`). **The hash shown to the user and the hash written by
`TrustStore::trust` must come from the same stored spec value, re-verified at grant time.** This is
`trust_reviewed_command`'s existing pattern (`crates/core/src/facade.rs:401-417`, with
`ChangedSinceReview` at `:411`) — reuse it, do not reinvent it.

If the displayed hash and the written hash derive from different values, **approval can be widened
after display**, which is the entire attack this feature must not enable.

### The raw-vs-resolved `working_dir` trap applies here too, and is worse

`ProcessSpec::variant_hash` hashes the **raw** `Option<PathBuf>`, not `resolved_working_dir(root)`
(`crates/core/src/config/model.rs:158`). In PRD-01 getting this wrong means refusing everything —
annoying but safe. **Here it means the dialog displays one command and authorizes a different
variant.** Build the spec with the caller's `working_dir` verbatim, and derive both the displayed
`TrustReviewCommand` and the granted hash from that one value. Test 5.9 is the guard.

## Why provenance and revoke are not optional

Verified against the current schema:

1. **Provenance.** `crates/store/src/trust.rs:3` and `crates/store/src/migrate.rs:41-55`: the `trust`
   table is `(project_id, variant_hash)` and **nothing else**. An agent-requested grant is today
   **indistinguishable** from one the user authored, and a review surface could only list bare
   hashes — useless. Add nullable `requested_by`, `reason`, `granted_at_unix_millis` via
   `ALTER TABLE ADD COLUMN` in a new `if version < 22` block. `SCHEMA_VERSION` is **21** at
   `crates/store/src/migrate.rs:12`; the `column_exists` helper and the `version < 21` block at
   `:352` are the precedent. NULL = user-authored, so existing rows stay correct without a table
   rebuild.
2. **Revocation.** `TrustStore::untrust` exists (`crates/core/src/trust.rs:61`) and
   `crates/core/src/facade.rs:387` says plainly *"Untrusting is not yet exposed"* — confirmed: no
   `untrust`/`revoke` path exists in `crates/app`, `crates/mcp`, `crates/httpapi`, or `crates/cli`.
   **If agents can cause grants, the user must be able to take them back.** Needs a
   `TrustRepo::list_grants(project)` method (none exists), a `Facade::revoke_command_trust`, and a
   UI list.

## Fix approach — ordered

`plan/06` §5 recipe per step.

1. **Record the decision first** (`plan/05` §12) — the aggregate, bounds, TTL, push+poll channels,
   persistent-grant rationale, and the provenance/revoke prerequisites. `orch-04` Task 1's "record
   before coding" rule.
2. **Shared-kernel vocabulary** (`plan/06` §5.6 sibling): `TrustRequest`, `TrustRequestState`,
   `TrustRequestId` in `crates/core/src/ids.rs`, the four caps. Reuses `TrustReviewCommand`.
3. **Ephemeral aggregate** (`plan/06` §5.1): `crates/core/src/trustrequest/` — state behind a
   `Mutex`, dedupe, bounds, lazy expiry.
4. **Releaser adapter + composition** (`plan/06` §5.2): a `LockReleaser` impl that drops a closing
   process's pending requests, added to the `CompositeLockReleaser` vec in
   `crates/app/src/lib.rs::build_facade` — the **only** place adapters are chosen.
5. **Two `DomainEvent` variants** (`plan/06` §5.6): `TrustRequested`, `TrustRequestResolved`;
   mirrored in `crates/app/ui/src/domain.ts` and handled in `projection.ts`'s exhaustive switch.
6. **Store: migration 22 + `TrustRepo` methods** — `set_trusted_with_provenance`, `list_grants`;
   implement in `crates/store/src/trust.rs`; update `FakeTrustRepo` in `crates/core/src/testing`.
7. **`ScopedFacade::request_command_trust` + `trust_request_status`** (`plan/06` §5.1).
8. **`Facade::approve_trust_request` / `deny_trust_request` / `pending_trust_requests` /
   `revoke_command_trust` / `list_trusted_commands`** — local-user authority only.
9. **ipc variants + conversions + dispatch arms** (`plan/06` §5.3 step 1). **Approve/deny must NOT
   appear on the IPC surface** — that is the boundary the whole design rests on.
10. **MCP args + 2 tools** (`plan/06` §5.3); add both names to `EXPECTED_TOOL_SURFACE`
    (`crates/mcp/src/server_tests.rs:109`).
11. **Tauri commands** (`plan/06` §5.5): approve, deny, list pending, list grants, revoke — thin
    `#[tauri::command]`s registered in the `invoke_handler!` list, typed wrappers in
    `crates/app/ui/src/api.ts`.
12. **UI** (`plan/06` §5.7) — **MUST go through the `/impeccable` skill per CLAUDE.md §5.**
    `PRODUCT.md` + `DESIGN.md` are the design source of truth; never hand-roll UI. The approval
    dialog and the trusted-grant list are both new surfaces and both need it.

## Test plan (must fail before, pass after)

New files per CLAUDE.md §16. Observable outcomes, never call shapes. For each row: break the fix,
watch it redden, restore.

### `crates/core/src/trustrequest/state_tests.rs` (aggregate unit, `MockClock`)

| # | Test | Asserts | Reddens when |
|---|---|---|---|
| 5.1 | `a_request_for_an_already_trusted_variant_short_circuits_to_granted` | outcome is `Granted`, nothing pending | the pre-check is dropped → a pointless prompt appears |
| 5.2 | `two_agents_requesting_one_variant_produce_one_pending_request` | both get the same `TrustRequestId`; pending count is 1 | dedupe keyed on `(project, variant, requester)` instead of `(project, variant)` |
| 5.3 | `the_project_ceiling_refuses_without_dropping_a_queued_request` | request 17 is refused **and** all 16 originals still readable | an evicting ring buffer is used instead of refuse-on-full |
| 5.4 | `an_expired_request_reads_back_as_expired_and_frees_its_slot` | drive `MockClock` past the TTL; status is `Expired`, a new request succeeds | lazy expiry omitted — the only test that fails, since nothing else advances the clock |
| 5.5 | `an_oversized_reason_is_refused` | > 4 KiB → refused, nothing pending | the reason cap is dropped |

### `crates/core/src/trustrequest/releaser_tests.rs`

| 5.6 | `a_closing_requesters_pending_request_is_dropped` | call `release_all(process)`; the request is gone and a resolution is announced | the releaser is not registered in `CompositeLockReleaser`, **or** cleanup was wired to `ProcessRemoved` instead — worth saying in the test name, because the event-bus version passes a happy-path test and fails only under `RecvError::Lagged` |

### `crates/core/src/facade/trustrequest_tests.rs` (round trip against fakes)

| # | Test | Asserts | Reddens when |
|---|---|---|---|
| 5.7 | `approving_a_request_makes_the_variant_startable` | after approve, `TrustStore::is_trusted` is true **and** the process's `requires_trust` read-model flag is false | `Supervisor::mark_trusted` omitted — trust is granted but the UI still shows the command as untrusted, which an `is_trusted` assertion alone would not catch |
| 5.8 | `approving_a_request_whose_spec_changed_since_display_is_refused` | mutate the stored spec, approve → `ChangedSinceReview`-equivalent; **nothing is trusted** | grant-time hash re-verification dropped — the core of the security argument |
| 5.9 | `the_displayed_hash_and_the_granted_hash_come_from_one_raw_spec` | request with a relative `working_dir`; the emitted `review.variant_hash` equals `spec.variant_hash()` of the **unresolved** spec, and a `solo.yml` command with the identical raw shape is startable after approval | the raw-vs-resolved bug — approval displays one command and authorizes another |
| 5.10 | `denying_a_request_trusts_nothing_and_resolves_it` | `is_trusted` false, request gone, `granted: false` announced | deny falls through to the grant path |
| 5.11 | `a_scoped_caller_cannot_approve_its_own_request` | `compile_fail` doc test — `ScopedFacade` exposes no approve method, matching the existing `compile_fail` block at `crates/core/src/facade/scoped.rs:33-61` | someone adds `approve_trust_request` to `ScopedFacade`; the compiler catches it, which is the point |
| 5.12 | `a_requester_cannot_assert_its_project_or_identity` | bind to a process in project A, request; the pending entry's `project` and `requested_by` are A's regardless of arguments | a project/requester parameter is added to the tool |
| 5.13 | `an_agent_requester_receives_the_decision_in_its_mailbox` | after approval, `agent_message_list` for the requester holds one `TrustDecision`; acknowledging removes it | the push leg is dropped |
| 5.14 | `a_non_agent_requester_still_learns_the_outcome_by_status` | bind to a `Command`-kind process, request, approve; the mailbox is empty **and** `trust_request_status` reports `Granted` | the design was built mailbox-only — the test that encodes decision §3 |
| 5.15 | `a_full_mailbox_cannot_undo_a_granted_trust` | fill the requester's inbox to `MAX_PENDING_MESSAGES_PER_RECIPIENT`, approve; `is_trusted` true and status `Granted` | notification is put inside the grant path instead of after it |

### `crates/store/src/trust_tests.rs`

| 5.16 | `a_grant_records_its_requester_and_reason` | write with provenance, reopen the store, read it back | the migration adds columns but the insert never populates them |
| 5.17 | `an_existing_grant_survives_the_migration_as_user_authored` | seed a v21 row, migrate, assert `is_trusted` still true and provenance is `None` | the migration rebuilds the table and drops rows |
| 5.18 | `revoking_a_grant_makes_the_variant_untrusted_again` | `list_grants` → revoke → gone, `is_trusted` false | revoke is exposed but not wired to the repo |

### Adapters

- `crates/ipc/src/protocol_tests.rs` — round-trip the two new requests and their responses.
- `crates/app/src/ipc_server/dispatch_tests.rs` — both arms route to `scoped(session)`; **approve and
  deny are absent from the IPC surface** (a negative assertion worth writing, since it is the
  boundary the security argument rests on).
- `crates/mcp/src/server_tests.rs:109` — add `"request_command_trust"` and `"trust_request_status"`
  to `EXPECTED_TOOL_SURFACE`; the count assertion at `:285` follows.
- UI component tests for the dialog and the grant list (see acceptance below).

## Acceptance — Done when

### Core behaviour
- [ ] An agent (or any bound process) can request approval for a command variant; the request is
      recorded ephemerally, deduped on `(project, variant_hash)`, and announced as
      `DomainEvent::TrustRequested`.
- [ ] A request for an already-trusted variant short-circuits to `granted` without prompting.
- [ ] Approving writes durable trust **and** clears the `requires_trust` read-model flag, so the
      command becomes startable without a restart.
- [ ] Approving a request whose spec changed since it was displayed is **refused and trusts
      nothing**.
- [ ] The hash displayed to the user and the hash written on approval derive from the **same raw
      spec value** (test 5.9).
- [ ] Denying resolves the request and trusts nothing.
- [ ] Every bound requester learns the outcome: Agents via a `TrustDecision` mailbox message,
      everyone via `trust_request_status`. A full mailbox or a departed requester **cannot** roll
      back a granted trust.
- [ ] A closing requester's pending request is dropped through the **`LockReleaser`** hook, and the
      drop is announced.
- [ ] All four bounds hold; overflow **refuses** without dropping a queued request; a request past
      its TTL reads back `Expired` and frees its slot.
- [ ] Approve and deny exist **only** on `Facade` — not on `ScopedFacade`, not on the IPC surface,
      not as an MCP tool.

### Provenance and revoke (prerequisites, not follow-ups)
- [ ] Migration 22 adds nullable `requested_by`, `reason`, `granted_at_unix_millis` to `trust`; a
      pre-existing v21 row survives and reads back as user-authored.
- [ ] A grant created by an approval records its requester and reason.
- [ ] The user can **list** every trusted command variant in a project with its provenance, and
      **revoke** any of them; a revoked variant is refused again on the next start.

### Approval-fatigue mitigations (UI acceptance criteria)
- [ ] The `reason` string is rendered as a **quotation attributed to the named requesting process** —
      never as Soloist's own prose. It is agent-supplied text that may be prompt-injected or simply
      confused.
- [ ] The `reason` is rendered as **plain text**: no markdown, no HTML, no link auto-linking.
- [ ] The **command line is the most prominent element** of the dialog; the reason is context, not
      the headline. Approving must be impossible without the command line being on screen — the
      existing `TrustDialog` contract ("Review what it runs, then trust it",
      `crates/app/ui/src/components/TrustDialog.tsx:43-44`), not a new promise.
- [ ] The approve control is **not auto-focused**, and deny is the **low-friction** path.
- [ ] The requesting process's label **and** id are shown, so the user knows who is asking.
- [ ] A request whose requester died closes its dialog rather than inviting approval on behalf of a
      dead process.
- [ ] Both new UI surfaces (approval dialog, trusted-grant list) were built **through the
      `/impeccable` skill** per CLAUDE.md §5, against `PRODUCT.md` + `DESIGN.md`. Contrast ≥ 4.5:1,
      OKLCH, reduced-motion fallbacks.

### Gates
- [ ] Every test above has been **observed failing** against the unfixed behaviour.
- [ ] `just lint` and `just test` exit 0; the dependency-direction guard is green.
- [ ] `PROGRESS.md` updated per CLAUDE.md §10; the decisions recorded in `plan/05` §12.

## Size estimate

| Work | Estimate |
|---|---|
| Ephemeral aggregate + bounds + TTL + releaser | 280–330 |
| Vocabulary, ids, 2 `DomainEvent` variants | 90–110 |
| `ScopedFacade` (request + status) & `Facade` (approve/deny/list/revoke) | 210–250 |
| ipc variants + conversions + dispatch arms | 100–120 |
| MCP args + 2 tools | 60–80 |
| Tauri commands + `invoke_handler` registration | 60–80 |
| **Durable provenance: migration 22 + `TrustRepo` methods + impls + `FakeTrustRepo`** | **160–200** |
| Tests (core + store + adapters) | 650–850 |
| **Core subtotal** | **1,600–2,000** |
| UI half — `domain.ts`/`projection.ts` mirrors (~90), pending-request store/hook (~80), approval dialog modelled on the 136-line `TrustDialog.tsx` (~200–250), trusted-grant list + revoke (~200), `api.ts` wrappers (~40), component tests (~350–450) | **800–1,200** |

Splitting the core further (e.g. provenance/revoke as its own PR) is possible but **not
recommended**: revoke is a prerequisite of the security argument, and landing agent-caused grants
without a way to take them back is the one sequencing to avoid.

## Risk on record

This makes "agent asks, user clicks yes" the path by which arbitrary commands become runnable.
**Approval fatigue is the real failure mode, and no amount of core hygiene fixes it** — the
mitigations that matter are the UI acceptance criteria above.

Worth saying in the docs and the UI copy rather than glossing: "an agent can ask for arbitrary code
execution and the user clicks yes" is a meaningfully different security posture from "the user
approves the commands in their own `solo.yml`". It should be presented as a new capability, not as an
extension of existing trust.

What a malicious or confused agent still **cannot** do: assert its own identity or project (both
derived from the authenticated session); reach another project (no project parameter); grant its own
request (approval is `Facade`-only); flood the user (16 per project / 64 global, refuse-not-evict);
leave a request behind after it dies (deterministic `LockReleaser` hook); or widen an approval after
display (hash re-verified at grant).

## Out of scope

- `spawn_process` itself — PRD-01.
- Any HTTP/CLI surface for requesting or approving trust. The loopback API is the local user's
  authority and has no session; approval is a desktop-UI action.
- Project-level trust (`is_project_trusted` / `set_project_trusted`,
  `crates/core/src/ports.rs:262-267`) — a separate, coarser gate that this ticket does not touch.
