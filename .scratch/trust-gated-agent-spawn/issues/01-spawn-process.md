# PRD-01 — O9 `spawn_process`: trust-gated arbitrary command spawn over MCP (PR #168)

Status: ready-for-agent
Blocked by: none

- **Severity:** P2 (parity row O9, v1 scope — currently entirely unimplemented)
- **Parity rows:** `plan/02-feature-parity-matrix.md` O9 (+ the O13 clause that this ticket resolves)
- **Area:** `crates/core/src/facade/scoped_process.rs`, `crates/core/src/facade/scoped.rs`,
  `crates/core/src/facade/mailbox.rs`, `crates/ipc/src/protocol/request.rs`, `crates/ipc/src/error.rs`,
  `crates/ipc/src/error/conversions.rs`, `crates/app/src/ipc_server/dispatch.rs`,
  `crates/mcp/src/args.rs`, `crates/mcp/src/tools/agent.rs`
- **Evidence:** VERIFIED in code (research session, 2026-08-14). `spawn_process` has zero
  occurrences under `crates/` on both `main` and the `feat/agent-mailbox-coordination` HEAD.
- **Depends on:** nothing. Ships independently of the mailbox work.

## Problem

An MCP-connected agent can spawn a *known agent tool* as a worker (`spawn_agent`), and the local
user can open a *fixed shell* from the UI (`Facade::create_terminal`). Neither lets an agent start
a command — so an agent that needs the project's dev server or test runner running has no way to
start one that is not already registered from `solo.yml`.

O9 closes that with `spawn_process`: create + start a command process in the caller's own project,
**gated by the same trust rule a manual command start gets**, enforced in the core so every adapter
inherits it.

## Decisions already made (do not re-litigate)

1. **No O13 onboarding leg.** `spawn_process` exposes **no** `prompt` and **no**
   `include_agent_instructions` parameter, and calls neither `queue_onboarding` nor
   `queue_spawned_task`. Owner-decided 2026-08-14 on the following mechanical grounds — a spawned
   command is a `Command`, and the whole O13 contract is agent-shaped:
   - `mailbox_identity` requires `view.kind == ProcessKind::Agent`
     (`crates/core/src/facade/mailbox.rs:372-378`), so a spawned command could never *retrieve or
     acknowledge* anything queued for it — and retrieval + acknowledgement **is** the O13 delivery
     contract.
   - `agent_roster` filters the same way (`crates/core/src/facade/mailbox.rs:106`).
   - `idle.track(id, kind)` is called from exactly one place — `crates/core/src/facade.rs:462`,
     inside `launch_agent` — and takes `AgentKind` (`crates/core/src/agents/tool.rs:15-26`), a
     closed provider enum with no member for "an arbitrary command". No `AgentActivityChanged` is
     ever published for a spawned command.
   - The mailbox reactor only sees tracked agents
     (`crates/core/src/coordination/mailbox/reactor.rs:94`, `:113-123`).

   So `queue_onboarding` on a spawned command would be a **silent no-op that also leaks**: the entry
   lands in `state.onboarding` / `state.wake_attempts`
   (`crates/core/src/coordination/mailbox/onboarding.rs:23-27`) and is only ever removed by
   `mark_wake_submitted` (`:67-74`, unreachable) or `remove_process` on `ProcessRemoved`
   (`reactor.rs:103`). And if it somehow *did* fire, `try_submit_turn` would type Soloist
   coordination prose into the stdin of `npm run dev`.

2. **Lineage IS recorded** for a spawned process, even though the mailbox is not. That is what makes
   it visible in the orchestration tree and what extends the delegation-depth rule to it (below).

3. **Scope comes from the session. There is no project parameter.** Adding one would be a security
   regression that reintroduces the hole F13 closed.

## Four load-bearing warnings

These are the ones that would otherwise **ship green**. Read them before writing code.

### (a) The raw-vs-resolved `working_dir` hash trap

`ProcessSpec::variant_hash` (`crates/core/src/config/model.rs:133-151`) digests exactly three
things: `command`, the **raw** `Option<PathBuf>` `working_dir` as written in `solo.yml` (with a
`0`/`1` presence discriminant), and the sorted `env` map. The process *name* is deliberately
excluded (`config/model.rs:130-132`).

**It hashes `self.working_dir`, NOT `resolved_working_dir(root)`** (`config/model.rs:158`). An
implementer who resolves the caller's `working_dir` against the project root before hashing produces
a digest that can never equal a trusted variant's, so `spawn_process` refuses **100% of calls** — and
that failure is indistinguishable from correct refuse-by-default behaviour, so it would ship.

**Rule:** build the `ProcessSpec` with the caller's `working_dir` **verbatim** (`None` when omitted,
meaning the project root), hash *that*, and let `Registration::command` do the resolution afterwards.

Because `env` is hashed too, accepting a caller-supplied `env` is safe by construction: any env the
user has not already approved yields a different variant and is refused. That is the reason the
parameter can exist at all.

### (b) The gate must precede `register`, not `start`

`Supervisor::register` (`crates/core/src/supervisor.rs:180-232`) publishes
`DomainEvent::ProcessSpawned` at `:222` and inserts a registry row **before** `start` ever consults
`guard_trust` (`crates/core/src/supervisor.rs:252-263`). So a "register first, let `start` refuse"
implementation leaves a **permanent ghost process in the sidebar for every refused spawn**.

The precedent is explicit — `plan/05-solo-reference-and-sources.md:407` (Worker spawn depth row):

> "The gate lives in the core, before the launch, so a refusal spawns and records nothing."

and `plan/orchestrator/orch-04-deferred-coordination-tools.md:46-49`:

> "a `spawn_process` must run **in the caller's effective project scope** and the spawned command
> variant must be **trusted there**, else it is refused — the same guarantee a manual command start
> gets, enforced in the core for every adapter."

Test 5.2 below asserts the snapshot is unchanged specifically to catch this. An error-only assertion
would pass against the buggy ordering.

### (c) `ProcessKind::Command` is forced, not chosen

The parity row title ("arbitrary *terminal* over MCP") and F11's grouping will push an implementer
toward `ProcessKind::Terminal`. It cannot be:

- `Registration::launched` hardcodes `trust_variant: None`
  (`crates/core/src/supervisor/registration.rs:106`) — a `Terminal` is ungated **by construction**,
  so there would be nothing for the gate to key on at restart.
- B10's owner decision (`plan/05-solo-reference-and-sources.md:470`, clause (a)) reserved `Terminal`
  for the fixed `exec ${SHELL:-/bin/sh}` constant precisely so "no surface can turn 'open a terminal'
  into arbitrary code execution".
- `Registration::command` is the **only** constructor carrying
  `trust_variant: Some(spec.variant_hash())` (`crates/core/src/supervisor/registration.rs:79`).

### (d) `supervisor.start` re-runs the gate — keep it

`guard_trust` runs again inside `start` on the stored `trust_variant`. That is intentional defence in
depth **and** it is what makes a later trust revocation refuse a *restart* of a spawned process. Do
not "optimize" it away because the façade already checked.

## Fix approach — ordered, file by file

Each step names the `plan/06-codebase-blueprint-and-cleanup.md` §5 recipe it satisfies.

### Step 0 — record the decision first (`orch-04` Task 1: "Record the decision before coding")

`plan/05-solo-reference-and-sources.md` §12 — add one row for `spawn_process` (O9) covering:
(a) the trust treatment below; (b) `ProcessKind::Command` and why; (c) scope from the session, no
project parameter; (d) label numbering; (e) the O13 resolution from "Decisions already made" §1.
Cross-reference `plan/05:470` clause (c), which currently says `spawn_process` "stays deferred to
`orch-04`" — update it in the same pass.

### Step 1 — core behaviour (`plan/06` §5.1, + §5.3 step 3 "add the Facade behavior first")

**`crates/core/src/facade/scoped.rs`** (197 lines) — add `SpawnProcessError` next to
`SpawnAgentError` at `:127`:

```rust
pub enum SpawnProcessError {
    NoProjectScope,                                 // no project selected/bound/singular
    WorkerMayNotSpawn,                              // one-level delegation depth
    UnknownProject,                                 // scope resolved to a project no longer open
    InvalidCommand(crate::config::InvalidCommand),  // #[from]
    Untrusted,                                      // variant not trusted in this project
    Store(StoreError),                              // #[from]
    Supervisor(SupervisorError),                    // #[from]
}
```

A distinct type rather than reusing `ScopedActionError` (`scoped.rs:105`), mirroring how
`SpawnAgentError` is distinct — `ScopedActionError` has no `WorkerMayNotSpawn`/`UnknownProject`/
`InvalidCommand`, and its `From<SupervisorError>` impl (`scoped.rs:143`) folds `Untrusted`
differently.

**`crates/core/src/facade/scoped_process.rs`** (303 lines → ~375, under the ~400 smell) — add
`ScopedFacade::spawn_process` next to `spawn_agent_request`, because they share the depth gate:

```rust
pub fn spawn_process(&self, request: SpawnProcessRequest) -> Result<ProcessId, SpawnProcessError>
```

**Sync, not async.** `spawn_agent_request` is sync (`scoped_process.rs:126`) and `supervisor.start`
spawns the actor into the ambient runtime. (`orch-04:99` sketches an `async fn` on `Facade` — that
sketch is stale on both counts; see "Doc corrections" below.)

Body, in order:

1. `let project = self.inner.effective_project(self.session).ok_or(NoProjectScope)?;`
2. Worker-depth gate — **extract** `crates/core/src/facade/scoped_process.rs:139-148` into a shared
   private helper (e.g. `fn caller_is_spawned_worker(&self) -> bool`) called by **both**
   `spawn_agent_request` and `spawn_process`. Two copies of a security gate is exactly the CLAUDE.md
   §15 "editing the same thing in two files" signal. The existing gate checks
   `lineage.parent_of(caller).is_some()` for both the bound process and the peer-group-resolved
   `home_process()`.
3. `crate::config::check_command(&label, &spec)` (`crates/core/src/config/model.rs:98`) — the one
   place the blank-name/blank-command invariant lives.
4. `let root = self.inner.project_root(project)?.ok_or(UnknownProject)?;`
5. **`if !self.inner.trust.is_trusted(project, &spec)? { return Err(SpawnProcessError::Untrusted); }`**
   — `crates/core/src/trust.rs:51`. `TrustStore` is a private field of `Facade`
   (`crates/core/src/facade.rs:129`), reachable from `scoped_process.rs` because it is a child module
   of `facade`; `crates/core/src/facade/mailbox.rs` already reaches `self.inner.mailbox` the same
   way. A `StoreError` propagates as a refusal — fail-closed, matching `supervisor.rs:239`.
6. Register + start (Step 2).
7. Lineage: `if let Some(lead) = self.inner.identity.origin(self.session).process() { self.inner.lineage.record(id, lead); }`
   — verbatim from `scoped_process.rs:183-185`.
8. **No** `idle.track`, **no** `queue_onboarding`, **no** `queue_spawned_task`.

**`crates/core/src/facade/mailbox.rs`** — add next to `SpawnAgentRequest` at `:19`:

```rust
pub struct SpawnProcessRequest {
    pub command: String,
    pub working_dir: Option<PathBuf>,   // RAW — see warning (a)
    pub env: BTreeMap<String, String>,
    pub label: Option<String>,
}
```

`Serialize + Deserialize` so `crates/ipc` reuses it verbatim (no DTO drift). If placing it in
`mailbox.rs` reads wrong — it has nothing to do with messaging — `crates/core/src/facade/scoped.rs`
is the acceptable alternative. The hard rule is that `crates/ipc` reuses the core type rather than
declaring a parallel DTO.

Re-export both new types from `crates/core/src/facade.rs:95-99`.

### Step 2 — registration shape (`plan/06` §5.1 step 1: logic in the owning context, C2)

Use **`Registration::command(project, &root, &label, &spec).numbered()`**
(`crates/core/src/supervisor/registration.rs:61-86`, `:118`). Construct the `ProcessSpec` with
`auto_start: false`, `auto_restart: false`, `restart_when_changed: vec![]` — none of the three
affects the variant hash.

Three things fall out for free; do not re-implement them:

- `spec.resolved_working_dir(root)` (`registration.rs:74`) clamps an absolute or `..`-climbing
  `working_dir` back inside the project root (`config/model.rs:173`).
- `auto_start`/`auto_restart`/`restart_when_changed` come from the spec.
- `.numbered()` gives `Labelling::NumberedIfTaken`, resolved inside `Registry::add` under the insert
  guard (`crates/core/src/supervisor/registry.rs:112-115`), so two concurrent spawns cannot claim
  one name.

**Label:** optional caller-supplied `label`; default to the command's first whitespace-separated
token. Solo documents no schema here — clean-room choice, record it in `plan/05` §12.

**Orphan-identity collision — already analysed, not an open risk.** `Registration::launched`'s
comment (`registration.rs:97-98`) says launched processes substitute `working_dir` for
`project_root` "so a leftover never matches a configured command", and `Registration::command` uses
the real root — so a `spawn_process` entry shares the configured-command identity namespace.
Checking the actual matcher: `Registry::identity` (`registry.rs:294-301`) reports
`entry.view.label`, which `Registry::add` has already rewritten to the **post-`numbered()`** label at
`:113`; `Registry::find_resting_match` (`registry.rs:306-322`) requires an exact three-way match on
`project_root` **and** label **and** `launch.command`, against a process whose status is `Stopped`;
`Supervisor::reconcile_orphans` (`crates/core/src/supervisor/reconcile.rs:23-35`) is the only caller.
Because `numbered()` guarantees the label is unique across every kind in the project at registration
time, a spawned entry can never collide with a live `solo.yml` command's label. The only orphan it
can attract is a leftover group of *the identical command line under the identical label in the same
project root* — which is what adoption is for. Test 5.9 locks this in.

### Step 3 — wire protocol (`plan/06` §5.3 step 1)

**`crates/ipc/src/protocol/request.rs`** — new variant after `SpawnAgent` (`:55`):

```rust
SpawnProcess {
    command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    working_dir: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label: Option<String>,
},
```

**`crates/ipc/src/protocol/response.rs`** — **no new variant.** Reuse `IpcResponse::Spawned(ProcessId)`
(`:57`); there is no initial message.

**`crates/ipc/src/error.rs`** — add **one** variant; the other four already exist
(`NoProjectScope` `:40`, `UnknownProject` `:25`, `Untrusted` `:157`, `WorkerMayNotSpawn` `:164`):

```rust
/// A spawn named a command line or label that cannot be run.
#[error("{0}")]
InvalidCommand(String),
```

Add it to the caller-fixable arm of `IpcError::is_request_error` (`crates/ipc/src/error.rs:193`, arm
list ending `:242`) — a refused blank command is fixable by the caller and must reach the model as a
tool result, not a protocol error. (`crates/mcp/src/tools/reply.rs:40-45` is what consumes that
classification: `is_request_error()` → `Ok(refusal(app))` → `CallToolResult::structured_error`.)

Also reword `WorkerMayNotSpawn`'s message, which currently reads *"a worker agent cannot spawn
agents"* — it now also covers processes. One string, two places: `crates/ipc/src/error.rs:167` and
its core twin `crates/core/src/facade/scoped.rs:133`. That cross-language pair is the sanctioned
duplication; do not add a third.

**`crates/ipc/src/error/conversions.rs`** — `impl From<SpawnProcessError> for IpcError` beside `:46`,
mapping all seven variants; `Store`/`Supervisor` → `IpcError::Internal(err.to_string())` per the
`LaunchAgentError` precedent at `:40-41`.

### Step 4 — app dispatch (`plan/06` §5.3 step 1: one arm, one façade call)

**`crates/app/src/ipc_server/dispatch.rs`** — one arm after `:199`:

```rust
IpcRequest::SpawnProcess { command, working_dir, env, label } => facade
    .scoped(session)
    .spawn_process(SpawnProcessRequest { command, working_dir, env, label })
    .map(IpcResponse::Spawned)
    .map_err(IpcError::from),
```

Import `SpawnProcessRequest` at `dispatch.rs:16-19`. No domain logic; the file's header (`:8-11`)
states why it is one flat exhaustive match.

### Step 5 — MCP tool (`plan/06` §5.3)

**`crates/mcp/src/args.rs`** — `SpawnProcessArg` after `SpawnAgentArg` (`:63-75`), deriving
`Deserialize + schemars::JsonSchema`. Doc comments on each field become the clean-room schema
descriptions. State plainly in the `command` doc that the command must **already be trusted** in the
project.

**`crates/mcp/src/tools/agent.rs`** — a second `#[tool]` in the existing `agent_router` block. It
belongs here, not in `tools/process.rs`: `process.rs` acts on processes that already exist,
`agent.rs` is the spawn category, and `plan/05:250` groups Solo's `spawn_process` under
Agent/Terminal. Handler shape mirrors `agent.rs:18-50` verbatim: destructure, build one
`IpcRequest`, match `Ok(IpcResponse::Spawned(p))` → `structured(...)`, `Ok(_)` → `unexpected()`,
`Err` → `app_error(&err)`.

**No change to `crates/mcp/src/server.rs`** — the "Agents" `ToolGroup` (`:229-233`) already composes
`agent_router`, and `served_tool_count` (`:136`) derives from the router. `plan/06` §5.3 step 2 is
satisfied by construction.

## Practical consequence to record in `plan/05` §12

Trust is only writable today by `Facade::trust_command` / `trust_reviewed_command`
(`crates/core/src/facade.rs:388, 401`), which resolve a spec **by name out of the loaded
`solo.yml`**. There is no API to trust an ad-hoc command line. So `spawn_process`'s usable set after
this ticket is exactly *"command variants the user has already approved in this project"* — in
practice, `solo.yml` commands. An agent cannot invent a command line and have it run.

That is orch-04 Task 1 implemented literally, and it must be written down, because "arbitrary
terminal over MCP" in the row title reads much wider than what ships. **PRD-02 is the ticket that
widens it**, deliberately and with a user approval step.

## Test plan (must fail before, pass after)

Placement per CLAUDE.md §16. `crates/core/src/facade/scoped_tests.rs` is already 750 lines — do
**not** grow it. Attach a new file by adding at the end of
`crates/core/src/facade/scoped_process.rs`:

```rust
#[cfg(test)]
#[path = "scoped_process_tests.rs"]
mod tests;
```

Fixture: `crates/core/src/testing/fixtures.rs:53` `facade_with_agent_tool()` gives a façade + a sole
project (so an unbound session resolves scope). Trust a variant with the public accessor
`Facade::trust()` (`crates/core/src/facade.rs:293`) → `TrustStore::trust`
(`crates/core/src/trust.rs:56`). Tests reaching `start` need `#[tokio::test]` —
`start` spawns the actor (`supervisor.rs:270`); `FakeSpawner::exits_on_terminate` stands in.

Every case asserts an **observable outcome**, never a call shape. Since all the code is new, "it
doesn't compile yet" is **not** proof — for each row, break the fix, watch it redden, restore.

### `crates/core/src/facade/scoped_process_tests.rs`

| # | Test | Asserts (observable) | Reddens when |
|---|---|---|---|
| 5.1 | `a_trusted_command_spawns_and_starts_in_the_session_project` | `Ok(id)`; `facade.process_view(id)` shows `project == scope`, `kind == ProcessKind::Command`, `requires_trust == false`, status ≠ `Stopped` | trust check inverted; or `Registration::launched`/`ProcessKind::Terminal` is used (both `requires_trust` and `kind` flip) |
| 5.2 | `an_untrusted_command_is_refused_and_registers_nothing` | `Err(SpawnProcessError::Untrusted)` **and** `facade.snapshot()` unchanged **and** no `ProcessSpawned` event | delete the trust check → the spawn succeeds; **or move the check after `register` → the error is right but the snapshot grows.** The second assertion is the only thing that catches warning (b); an error-only assertion passes against the bug |
| 5.3 | `a_variant_trusted_in_another_project_is_refused` | trust the spec in project B, session scoped to A → `Untrusted` | the gate passes the wrong `ProjectId` to `is_trusted` |
| 5.4 | `spawn_process_without_a_project_in_scope_is_refused` | `NoProjectScope`, nothing registered | scope resolution dropped |
| 5.5 | `a_spawned_worker_may_not_spawn_a_process` | mirror `crates/core/src/facade/scoped_tests.rs:243-258`: spawn a worker via `spawn_agent`, bind a session to it, call `spawn_process` → `WorkerMayNotSpawn`, nothing registered | the shared depth helper is not called from `spawn_process` |
| 5.6 | `a_process_spawned_via_spawn_process_may_not_itself_spawn` | bind a session to the id from 5.1, call `spawn_process` → `WorkerMayNotSpawn` | the `lineage.record` call is omitted — proves the depth rule closes over the new surface |
| 5.7 | `the_trust_variant_matches_a_configured_commands_variant` | seed a `solo.yml` command, trust it via `Facade::trust_command`, then `spawn_process` with byte-identical `command`/`working_dir`/`env` → `Ok` | **warning (a).** Switch the hash input to `resolved_working_dir(root)` and *only this test* reddens — 5.1–5.6 all still pass because they trust the same shape they spawn |
| 5.8 | `a_process_spawned_by_a_lead_nests_under_it` | `facade.orchestration_snapshot(project)` contains an `AgentNode` for the new id with `parent == Some(lead)` and `kind == ProcessKind::Command` | `lineage.record` omitted |
| 5.9 | `a_spawned_process_does_not_match_a_configured_commands_orphan_identity` | register a `solo.yml` command "Web"; `spawn_process` the same command line with `label: Some("Web")`; assert the new label is `"Web 2"` and `find_resting_match(root, "Web", cmd)` still resolves to the **configured** process | `.numbered()` dropped — the Step-2 adoption analysis stops holding |
| 5.10 | `a_blank_command_is_refused` | `InvalidCommand(BlankCommand)`, nothing registered | `check_command` not called |
| 5.11 | `a_spawned_process_is_queued_no_onboarding_briefing` | the mailbox holds no pending onboarding for the new id | someone adds a `queue_onboarding` call — the inverse test that encodes decision §1 |

### Adapter / wire

| File | Test |
|---|---|
| `crates/ipc/src/protocol_tests.rs` | `IpcRequest::SpawnProcess` round-trips, including the omitted-optionals shape (`working_dir`/`env`/`label` absent). Reddens on a serde tagging slip — the same class of bug the P8 review already caught once on `IpcResponse` |
| `crates/app/src/ipc_server/dispatch_tests.rs` | mirror `:448`: the arm reaches `spawn_process` and returns `IpcResponse::Spawned`; a refusal maps to `IpcError::Untrusted`. Reddens if the arm routes to `Facade` instead of `scoped(session)` |
| `crates/mcp/src/server_tests.rs:109` | add `"spawn_process"` under the `// tools/agent.rs` comment in `EXPECTED_TOOL_SURFACE`; the served-surface assertion at `:262-268` and the count at `:285` follow automatically. Reddens if the `#[tool]` is missing or lands in a router that is not composed |
| `crates/mcp/src/server_tests.rs` | handler test mirroring `:949`: the tool threads its args into one `IpcRequest::SpawnProcess` and surfaces the process id |
| `crates/pty/tests/orchestration.rs` (extends `:102`) | real-PTY E2E: a bound lead trusts a variant, spawns it, the child actually runs and appears under the lead in `orchestration_snapshot`; an untrusted variant is refused and nothing appears |

## Acceptance — Done when

- [ ] A trusted command variant, spawned by a session scoped to that project, **creates and starts**
      a `ProcessKind::Command` process bound with `SOLOIST_PROCESS_ID` and nested under its lead in
      `orchestration_snapshot`.
- [ ] An **untrusted** variant is refused **and nothing is registered** — no registry row, no
      `ProcessSpawned` event, no ghost row in the sidebar.
- [ ] A variant trusted only in **another** project is refused; a session with **no** project in
      scope is refused. There is no project parameter on any surface.
- [ ] A caller that is itself a spawned worker is refused with `WorkerMayNotSpawn`, and a process
      created by `spawn_process` likewise cannot spawn.
- [ ] The variant hash written by the gate is computed from the **raw** `working_dir`, proven by test
      5.7 passing against a `solo.yml` command trusted through the existing UI path.
- [ ] `spawn_process` exposes no `prompt` and no `include_agent_instructions`, and queues no
      onboarding briefing (test 5.11).
- [ ] The MCP tool surface guard at `crates/mcp/src/server_tests.rs:109` lists `spawn_process`.
- [ ] Every test above has been **observed failing** against the unfixed behaviour (break the fix,
      watch it redden, restore) — a test never seen red is unproven.
- [ ] `just lint` and `just test` exit 0.
- [ ] `PROGRESS.md` updated per CLAUDE.md §10.

### Doc corrections that are part of this ticket

- [ ] `plan/05-solo-reference-and-sources.md` §12 — the new O9 row (Step 0).
- [ ] `plan/05-solo-reference-and-sources.md:402` — O13 row: replace "extending it to
      `spawn_process` stays partial until future O9 lands" with the resolution from §1.
- [ ] `plan/05-solo-reference-and-sources.md:470` clause (c) — "an MCP `spawn_process` (O9) … stays
      deferred to `orch-04`" is now false.
- [ ] `plan/02-feature-parity-matrix.md` O9 row — **restore the base Verify gate.** The row on `main`
      reads:

      `| O9 | spawn_process (arbitrary terminal over MCP) with its trust treatment | ✅ name / ❓ trust | orch-04 | v1 | Trusted spawn works; untrusted / cross-project refused |`

      The current HEAD weakened it with exactly two hedges — `— still future work` in Scope and a
      `When implemented:` prefix on Verify. Drop both.
- [ ] `plan/02-feature-parity-matrix.md` O13 row — resolve "Repeat for `spawn_process` when O9 lands".
- [ ] `plan/orchestrator/README.md:92, :96` — the same two rows carry the same hedges.
- [ ] `plan/orchestrator/orch-04-deferred-coordination-tools.md` — `:18-24` ("Current split" / "Note
      on O13's independence"), `:74` ("only when O9 is implemented"), `:128`, `:148-149`. **Also
      `:99`**, whose Interfaces sketch is stale twice over: it puts the method on `Facade` with a
      caller-supplied `scope` (CLAUDE.md §16 says a session-scoped action goes on `ScopedFacade`,
      **never** `Facade`, and §2's hierarchy puts CLAUDE.md above the phase file), and it marks the
      method `async` (the `spawn_agent_request` precedent is sync). Fix the phase file rather than
      following it, and note the divergence in the PR so a reviewer does not read it as the
      implementer ignoring the plan.

## Size estimate

**550–750 inserted lines** — roughly 250 production, 350–450 tests, 40 docs. No schema change, no
migration. Self-contained; ships alone.

## Out of scope

- Any way to trust a command line the user has **not** already approved — that is PRD-02.
- Any UI. This ticket adds no Tauri command and no frontend surface.
- The O13 onboarding leg (decided closed, §1). If it is ever reopened it requires relaxing the two
  `kind == Agent` checks, adding a provider-neutral idle strategy, and idle-tracking non-agents —
  a larger change than O9 itself.
