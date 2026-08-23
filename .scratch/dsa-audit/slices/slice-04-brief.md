# Implementation Brief — Slice 4 — DSA Audit Trio

**Current phase (from PROGRESS.md):** DSA AUDIT SLICE 3 (S27-F1 + S25-F1) — `Done — pending verify`, UNCOMMITTED. Next: slice 4 (this session) through slice 10.

**Architecture constraints in force:**
- Hexagonal layering: core is pure, OS/UI/MCP/HTTP/CLI/SQLite/PTY are adapters behind ports.
- One behavior, many frontends: core command routed to from every adapter; never reimplement per adapter.
- Errors as values: typed errors (thiserror) at boundaries; no unwrap/expect/panic in long-running tasks.
- Single-source of truth: Rust enum in core; TS mirror in one domain.ts; no magic strings/numbers.
- Bounded everything: caps on buffers/channels/retries; no unbounded thing is left unfixed.
- Comment and naming discipline: public doc comments only; no phase numbers, plan citations, or restating code in names.
- Test discipline: test behavior, not call shape; every new/changed test must FAIL against unfixed code first.

---

## Work Item A — S08-F2: Scratchpad blank-name validation on `rename` and `transfer`

**Finding verifier status:** (Awaiting verify-B result; proceeding with finding as stated in audit.)

### The defect

`Scratchpads::rename` (crates/core/src/coordination/scratchpad.rs:226-237) does not validate the `to` parameter before passing it to the repo. A blank target name is accepted, stored, and returned as a valid rename.

**Current code (lines 226-237):**
```rust
    /// Renames the scratchpad `from` to `to` in `project` (the durable id is unchanged), returning
    /// the renamed scratchpad. [`RenameError::NotFound`] if there is none, [`RenameError::NameTaken`]
    /// if `to` is already used in the project.
    pub fn rename(
        &self,
        project: ProjectId,
        from: &str,
        to: &str,
    ) -> Result<ScratchpadView, RenameError> {
        match self.repo.rename(project, from, to)? {
            RenameResult::Renamed(stored) => Ok(ScratchpadView::of(*stored)),
            RenameResult::NotFound => Err(RenameError::NotFound),
            RenameResult::NameTaken => Err(RenameError::NameTaken),
        }
    }
```

**Pattern to follow:** `crates/core/src/coordination/diagram.rs` (lines 27-32, 40-56, 216-228):
- Defines `fn validate_name(name: &str) -> Result<(), String>` (lines 27-32)
- Calls it in the main `validate` function (line 42)
- Calls it in `rename` before the repo call (line 222)
- Has `RenameError::Invalid(String)` variant (line 288)

### Changes required

#### 1. **crates/core/src/coordination/scratchpad.rs**

**Add the `validate_name` helper** (after line 48, before the `render` function):
```rust
fn validate_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        Err("name must not be blank".to_owned())
    } else {
        Ok(())
    }
}
```

**Update the `validate` function** (line 32) to call `validate_name` instead of inlining the check:
```rust
fn validate(name: &str, body: &str) -> Result<(), String> {
    let mut problems: Vec<String> = Vec::new();
    if let Err(problem) = validate_name(name) {
        problems.push(problem);
    }
    if body.len() > MAX_SCRATCHPAD_CONTENT_BYTES {
        problems.push(format!(
            "the content exceeds the {} KiB cap",
            MAX_SCRATCHPAD_CONTENT_BYTES / 1024
        ));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}
```

**Update `RenameError` enum** (lines 322-334) to add the `Invalid` variant:
```rust
#[derive(Debug, thiserror::Error)]
pub enum RenameError {
    /// The target name failed validation (blank name).
    #[error("scratchpad is not well-formed: {0}")]
    Invalid(String),
    /// No scratchpad exists under the source name in the project.
    #[error("no scratchpad under that name")]
    NotFound,
    /// The target name is already used by another scratchpad in the project.
    #[error("a scratchpad with that name already exists")]
    NameTaken,
    /// A durable read or write failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}
```

**Update `rename` method** (line 226) to validate `to` before the repo call:
```rust
    pub fn rename(
        &self,
        project: ProjectId,
        from: &str,
        to: &str,
    ) -> Result<ScratchpadView, RenameError> {
        validate_name(to).map_err(RenameError::Invalid)?;
        match self.repo.rename(project, from, to)? {
            RenameResult::Renamed(stored) => Ok(ScratchpadView::of(*stored)),
            RenameResult::NotFound => Err(RenameError::NotFound),
            RenameResult::NameTaken => Err(RenameError::NameTaken),
        }
    }
```

**Update `transfer` method** (line 250) to validate `name` parameter the same way:
```rust
    pub fn transfer(
        &self,
        from: ProjectId,
        name: &str,
        to: ProjectId,
    ) -> Result<ScratchpadTransfer, RenameError> {
        validate_name(name).map_err(RenameError::Invalid)?;
        match self.repo.transfer(from, name, to)? {
            TransferResult::Transferred(moved) => Ok(ScratchpadTransfer {
                scratchpad: ScratchpadView::of(moved.scratchpad),
                todos: moved.todos,
            }),
            TransferResult::NotFound => Err(RenameError::NotFound),
            TransferResult::NameTaken => Err(RenameError::NameTaken),
        }
    }
```

#### 2. **crates/core/src/facade/scratchpad.rs**

**Update `scratchpad_rename_in` method** (lines 116-132) to map the new `Invalid` variant:
```rust
    pub fn scratchpad_rename_in(
        &self,
        project: ProjectId,
        from: &str,
        to: &str,
    ) -> Result<ScratchpadView, CoordinationError> {
        self.emit_scratchpad(
            project,
            self.scratchpads
                .rename(project, from, to)
                .map_err(|err| match err {
                    RenameError::Invalid(message) => CoordinationError::InvalidScratchpad(message),
                    RenameError::NotFound => CoordinationError::UnknownScratchpad,
                    RenameError::NameTaken => CoordinationError::ScratchpadNameTaken,
                    RenameError::Store(err) => CoordinationError::Store(err),
                }),
        )
    }
```

**Update `scratchpad_transfer_in` method** (lines 143-170) similarly:
```rust
        let moved = self
            .scratchpads
            .transfer(from, name, to)
            .map_err(|err| match err {
                RenameError::Invalid(message) => CoordinationError::InvalidScratchpad(message),
                RenameError::NotFound => CoordinationError::UnknownScratchpad,
                RenameError::NameTaken => CoordinationError::ScratchpadNameTaken,
                RenameError::Store(err) => CoordinationError::Store(err),
            })?;
```

#### 3. **crates/core/src/coordination/scratchpad_tests.rs**

**Add test** (after line 204, `rename_reports_missing_and_taken`):
```rust
#[test]
fn rename_rejects_a_blank_target_name_without_moving_the_scratchpad() {
    let pads = scratchpads();
    pads.write(PROJECT, "old", body(), None).expect("create");

    assert!(matches!(
        pads.rename(PROJECT, "old", "   "),
        Err(RenameError::Invalid(message)) if message.contains("name")
    ));
    assert!(pads.read(PROJECT, "old").unwrap().is_some());
    assert!(pads.read(PROJECT, "   ").unwrap().is_none());
}
```

**Test naming convention:** follows `diagram_tests.rs` line 225: `rename_rejects_a_blank_target_name_without_moving_the_diagram`.

---

## Work Item B — S11-F1: Git `commit_template` missing `NotARepo` arm

**Finding verifier status:** (Awaiting verify-B; proceeding as stated.)

### The defect

`Git::configured_template` (crates/core/src/git/commit.rs:52-60) calls `self.repository.commit_template()` and returns its result directly without handling `GitError::NotARepo`. When the root is not a repository, the error propagates instead of returning `Ok(None)` as the aggregate's contract requires.

**Current code (lines 52-60):**
```rust
    /// The same template, for a caller that has already passed the gate. One place applies the
    /// ceiling and asks the port, so what is offered as a starting message and what an agent is
    /// asked to fill in can never be two different things.
    pub(super) fn configured_template(
        &self,
        project: ProjectId,
        root: &Path,
    ) -> Result<Option<String>, GitError> {
        let gate = self.gate(project);
        let _running = lock(&gate);
        self.repository.commit_template(root, COMMIT_TEMPLATE_LIMIT)
    }
```

**Pattern to follow:** `crates/core/src/git/branch.rs:33-41` — match on the result and handle `NotARepo`:
```rust
pub fn branches(&self, project: ProjectId, root: &Path) -> Result<Option<Branches>, GitError> {
    let gate = self.gate(project);
    let _running = lock(&gate);
    match self.repository.branches(root, BRANCH_PAGE_SIZE) {
        Ok(branches) => Ok(Some(branches)),
        Err(GitError::NotARepo) => Ok(None),
        Err(err) => Err(err),
    }
}
```

### Changes required

#### 1. **crates/core/src/git/commit.rs**

**Update `configured_template` method** (lines 52-60):
```rust
    pub(super) fn configured_template(
        &self,
        project: ProjectId,
        root: &Path,
    ) -> Result<Option<String>, GitError> {
        let gate = self.gate(project);
        let _running = lock(&gate);
        match self.repository.commit_template(root, COMMIT_TEMPLATE_LIMIT) {
            Ok(template) => Ok(template),
            Err(GitError::NotARepo) => Ok(None),
            Err(err) => Err(err),
        }
    }
```

#### 2. **crates/core/src/git/repository.rs**

**Extend the port trait doc comment** (lines 92-97) to enumerate all 18 methods and clarify that `commit_template` returns `Ok(None)` when the path is not a repository:

Add after the opening `pub trait GitRepository: Send + Sync {` line (after line 98):
```rust
    // The trait has the following methods (18 in total):
    // - status: repository working-tree status
    // - list_files: all paths the repository tracks
    // - diff: how a path differs at a target
    // - read_file: a working tree copy of a path
    // - log: one page of history
    // - stage: record a path in the index
    // - unstage: take a path out of the index
    // - discard: throw away unstaged changes
    // - stage_hunk: record one hunk in the index
    // - unstage_hunk: take one hunk out of the index
    // - discard_hunk: throw away one hunk of unstaged change
    // - commit_template: the initial message a new commit starts from
    // - commit: record the index as a commit
    // - branches: the branches a switcher can offer
    // - branch: create, switch, or delete a branch
    // - stash: move changes to/from the stash
    // - sync: exchange commits with a remote
    // - abort_merge: abandon an in-progress merge
    // 
    // For methods returning `Ok(None)` (template, status, etc.), that means the path is not a
    // repository, which is an ordinary state rather than a failure.
```

Or, more concisely, update just the `commit_template` method's doc comment (line 191-202) to clarify:
```rust
    /// The message a new commit starts from, as the repository's own configuration supplies it,
    /// or `None` where it supplies none, or when the path is not a repository.
    ///
    /// What comes back is what version control would have committed had the configured template
    /// been left exactly as it was found: the guidance lines it strips from an edited message are
    /// already gone, because a template's hints exist to be read and replaced, and a message box
    /// is not an editor session anybody would expect to prune them from by hand.
    ///
    /// `None` — rather than a failure — for a configuration that names nothing readable, for a
    /// template longer than `limit` (which is the core's ceiling), or for a path under no
    /// repository, which is an ordinary state rather than a fault.
    fn commit_template(&self, root: &Path, limit: usize) -> Result<Option<String>, GitError>;
```

#### 3. **crates/core/src/git/commit_tests.rs**

**Add test** (after line 182, `a_template_past_the_ceiling_the_core_sets_is_no_template_at_all`):
```rust
#[test]
fn a_template_read_for_a_non_repository_returns_none_not_an_error() {
    let repository = FakeGitRepository::reporting(git_status("main"))
        .refusing(GitError::NotARepo);
    let project = ProjectId::next();
    let git = git_trusting(repository, project);

    let offered = git
        .commit_template(project, Path::new(ROOT))
        .expect("a non-repo is not a fault");

    assert_eq!(offered, None);
}
```

**Test naming convention:** follows the pattern in commit_tests.rs of describing the observable outcome: `a_template_read_for_a_non_repository_returns_none_not_an_error`.

---

## Work Item C — S12-F2: Trust requests `resolve` and `withdraw_requests_of` verification

**Finding verifier status:** (Awaiting verify-B; proceeding as stated.)

### The context

`crates/core/src/trust/requests.rs` implements the pending-trust-request aggregate. Two methods are in scope:

1. **`resolve(id: TrustRequestId, outcome: TrustRequestState) -> Option<TrustRequest>`** (lines 219-236)
   - Removes a pending request by id and records the outcome (in the resolved ring).
   - Announces `TrustRequestResolved` event on the bus.
   - Returns the resolved request or `None` if it was no longer pending.

2. **`withdraw_requests_of(process: ProcessId)`** (lines 241-268)
   - Called when a process closes (via `LockReleaser::release_all`, crates/core/src/trust/releaser.rs).
   - Removes every pending request opened by that process.
   - Marks each `TrustRequestState::Withdrawn`.
   - Records receipts and announces `TrustRequestResolved` events.

### Current implementation — exact code

**`resolve` method (lines 219-236):**
```rust
    /// Removes `id` from the pending set with `outcome`, records the receipt a later poll reads,
    /// and announces the resolution. Returns the request that was resolved, or `None` when it was
    /// no longer pending.
    pub fn resolve(&self, id: TrustRequestId, outcome: TrustRequestState) -> Option<TrustRequest> {
        let resolved = {
            let mut state = lock(&self.state);
            let index = state
                .pending
                .iter()
                .position(|held| held.request.id == id)?;
            let held = state.pending.remove(index);
            record_receipt(&mut state, id, held.request.project, outcome);
            held.request
        };
        self.bus.publish(DomainEvent::TrustRequestResolved {
            project: resolved.project,
            id: resolved.id,
            state: outcome,
        });
        Some(resolved)
    }
```

**`withdraw_requests_of` method (lines 241-268):**
```rust
    /// Drops every request `process` opened, marking each [`TrustRequestState::Withdrawn`] and
    /// announcing it — so an approval prompt already on screen for a process that has closed goes
    /// away rather than inviting a grant on its behalf.
    pub(super) fn withdraw_requests_of(&self, process: ProcessId) {
        let withdrawn: Vec<_> = {
            let mut state = lock(&self.state);
            let (leaving, staying): (Vec<_>, Vec<_>) = std::mem::take(&mut state.pending)
                .into_iter()
                .partition(|held| held.request.requested_by == process);
            state.pending = staying;
            for held in &leaving {
                record_receipt(
                    &mut state,
                    held.request.id,
                    held.request.project,
                    TrustRequestState::Withdrawn,
                );
            }
            leaving
                .into_iter()
                .map(|held| (held.request.project, held.request.id))
                .collect()
        };
        for (project, id) in withdrawn {
            self.bus.publish(DomainEvent::TrustRequestResolved {
                project,
                id,
                state: TrustRequestState::Withdrawn,
            });
        }
    }
```

### Helper function — "prune" role

**`prune_expired` function (lines 300-316):**
```rust
/// Moves every request past its expiry out of the pending set, recording each as
/// [`TrustRequestState::Expired`], and reports them so the caller can announce them after
/// unlocking.
fn prune_expired(state: &mut RequestState, now: u64) -> Vec<(ProjectId, TrustRequestId)> {
    let (aged, live): (Vec<_>, Vec<_>) = std::mem::take(&mut state.pending)
        .into_iter()
        .partition(|held| held.request.expires_unix_millis <= now);
    state.pending = live;
    for held in &aged {
        record_receipt(
            state,
            held.request.id,
            held.request.project,
            TrustRequestState::Expired,
        );
    }
    aged.into_iter()
        .map(|held| (held.request.project, held.request.id))
        .collect()
}
```

**Role of "prune":** Every read-like method (`status`, `pending`, `peek`, `record`) calls `prune_expired` before or while acquiring the lock, so requests that have aged past their TTL are removed and announced as `Expired` without needing a background timer. The `prune_expired` helper does the partition and recording; the caller (on the read method) announces the expired events after the lock is released.

### Existing tests

**Test file:** `crates/core/src/trust/requests_tests.rs` (lines 1-202)

Existing tests cover:
- `two_processes_requesting_one_variant_produce_one_pending_request` (line 46)
- `a_different_variant_from_the_same_process_is_its_own_request` (line 69)
- `the_project_ceiling_refuses_without_dropping_a_queued_request` (line 86)
- `the_global_ceiling_refuses_across_projects` (line 115)
- `an_expired_request_reads_back_as_expired_and_frees_its_slot` (line 143) — covers prune indirectly
- `an_oversized_reason_is_refused` (line 172)
- `a_status_read_cannot_see_another_projects_request` (line 186)
- `a_resolved_request_reads_back_its_outcome` (line 202) — covers resolve

**Coverage gap:** No direct test for `withdraw_requests_of`. No test explicitly verifying:
- A process closing via `LockReleaser::release_all` withdraws its requests.
- Multiple requests from one process are all withdrawn.
- Requests from other processes are unaffected.
- Announced events carry correct metadata.

### Changes required

#### 1. **crates/core/src/trust/requests_tests.rs**

**Add test** (after line 202):
```rust
#[test]
fn withdraw_requests_of_removes_all_requests_from_a_process_and_marks_them_withdrawn() {
    let project = ProjectId::from_raw(1);
    let leaving = ProcessId::next();
    let staying = ProcessId::next();
    let requests = requests(Arc::new(MockClock::new()));

    let leaving_1 = requests
        .record(submission(project, leaving, "npm run build"))
        .expect("record");
    let leaving_2 = requests
        .record(submission(project, leaving, "npm run test"))
        .expect("record");
    let staying_req = requests
        .record(submission(project, staying, "npm run lint"))
        .expect("record");

    requests.withdraw_requests_of(leaving);

    assert_eq!(
        requests.status(project, leaving_1),
        Some(TrustRequestState::Withdrawn),
        "a withdrawn request must read back with Withdrawn status"
    );
    assert_eq!(
        requests.status(project, leaving_2),
        Some(TrustRequestState::Withdrawn),
        "all requests from the departing process must be withdrawn"
    );
    assert_eq!(
        requests.status(project, staying_req),
        Some(TrustRequestState::Pending),
        "requests from other processes must not be affected"
    );
    assert!(
        requests.pending(project).is_empty(),
        "no pending requests remain after withdrawal"
    );
}
```

**Test naming convention:** follows existing test names in the file, describing the observable behavior: `withdraw_requests_of_removes_all_requests_from_a_process_and_marks_them_withdrawn`.

---

## Session protocol notes

From PROGRESS.md:
- **Current phase:** DSA AUDIT — slice 3 (S27-F1 + S25-F1) completed with gates green (`just lint`, `just test` both passing).
- **Active work:** Three audit findings (S08-F2, S11-F1, S12-F2) from the codebase discipline audit.
- **Architecture constraints:** All eight hold (hexagonal, one-behavior-many-frontends, errors-as-values, single-source, bounded, deterministic-shutdown, comment-discipline, test-discipline).
- **No locked decisions blocked or bent by these changes.**

**Required setup before implementation:**
1. Invoke `tauri-calling-rust` (IPC changes? No — these are domain core only.)
2. Invoke `testing-guidelines` (for test-first approach and assertion patterns).
3. No UI/UX changes here — core + tests only.

**Post-implementation gate:**
- `just lint` (Rust fmt, clippy -D warnings, tsc, ESLint, dependency-direction guard)
- `just test` (cargo test --workspace + vitest; must run in full, not scoped)
- Both must exit 0 and all tests must be shown to fail without the fix.
- `PROGRESS.md` updated with evidence.

---

## Summary of files to modify

| File | Lines | Change |
|------|-------|--------|
| `crates/core/src/coordination/scratchpad.rs` | +27-32 (validate_name), 32-47 (validate), 322-334 (RenameError), 226-237 (rename), 245-259 (transfer) | Add validate_name, Invalid variant, call validation, update transfer |
| `crates/core/src/facade/scratchpad.rs` | 116-132 (scratchpad_rename_in), 143-170 (scratchpad_transfer_in) | Add Invalid error mapping arm in both methods |
| `crates/core/src/coordination/scratchpad_tests.rs` | +after 204 | Add rename_rejects_a_blank_target_name_without_moving_the_scratchpad test |
| `crates/core/src/git/commit.rs` | 52-60 (configured_template) | Match on result, handle NotARepo => Ok(None) |
| `crates/core/src/git/repository.rs` | 191-202 (commit_template doc) or 92-97 (trait doc) | Clarify NotARepo behavior in doc comment |
| `crates/core/src/git/commit_tests.rs` | +after 182 | Add a_template_read_for_a_non_repository_returns_none_not_an_error test |
| `crates/core/src/trust/requests_tests.rs` | +after 202 | Add withdraw_requests_of_removes_all_requests_from_a_process_and_marks_them_withdrawn test |

**Note on "C1 row-type decision":** The user mentioned this decision must land alongside S08-F2. It is not yet resolved in the slice-04 instructions. Awaiting clarity from the verifier (verify-B) or user guidance before assuming a scope change.

---

# VERIFIER-NARROWED SCOPE (authoritative — from `dsa-audit-verify-B`)

This section supersedes every "Awaiting verify-B" note above. Verify-B was read in
full; these are its verdicts verbatim in substance.

## A — S08-F2 · **NARROWED**

Defect reproduced end to end by the verifier. Narrowed on two points:

1. "Wedged into a state it can never be written from again" is **overstated**. The
   blank-named row is still listed with its exact name, still readable, still
   renamable back using the name from `scratchpad_list`, still deletable. A
   nuisance state, not a trap. Do not write a test or a comment claiming a trap.
2. The `DocName` newtype spanning both aggregates plus both facades is
   **REJECTED** — speculative abstraction for one missing call, YAGNI.

**Surviving claim (implement exactly this):** the finding's own fallback — add
`validate_name(to)` + `RenameError::Invalid(String)` to
`crates/core/src/coordination/scratchpad.rs` and one arm to the facade's error
map in `crates/core/src/facade/scratchpad.rs`, mirroring
`crates/core/src/coordination/diagram.rs` exactly, plus the mirror regression
test. ~15 lines.

**Also note: S08-F1 was REJECTED on verification** (`ScratchpadView.rendered` is a
published tool contract). Do **not** touch `rendered`, `render()`, or
`ScratchpadView`'s shape.

**C1 gate.** The priority list requires the row-type decision to land alongside
this. Per cross-cutting pattern C1, S08 has **already declined** aggregate
unification of `Scratchpads`/`Diagrams`, citing `transfer`, template seeding and
the heading-skipping `gist` rule. The obligation here is to **record** that
decision (S15 and S16 both wait on the answer), not to perform a unification.

## B — S11-F1 · **NARROWED**

Seven reads translate `NotARepo` into "no repository here": `branch.rs:38`,
`files.rs:32`, `files.rs:58`, `history.rs:49`, `diff.rs:68`,
`message_change.rs:104`, `status.rs:342`. `commit.rs::configured_template` calls
`self.repository.commit_template(root, COMMIT_TEMPLATE_LIMIT)` with **no**
`NotARepo` arm. The adapter manufactures that error deliberately
(`crates/git/src/lib.rs`), and the null-object port
(`crates/core/src/git/repository.rs:343`) also answers `NotARepo` — so a core
built without a git adapter errors on this one read while every sibling answers
`None`.

Narrowed on two points:

1. The end-to-end UI claim is **overstated**. `useCommitTemplate` returns only
   `value` from `useRepositoryRead`; the rejected promise is captured into an
   `error` the hook discards, so the user sees `null` either way. This is a
   **core-contract inconsistency, not a broken surface** — do not describe it as
   a user-visible bug.
2. The `reading` seam is **DRY-only and REJECTED as part of this change**; each
   site would still need `.flatten()` / `.unwrap_or_default()` decided per site,
   and `read` cannot use it cleanly. Optional tidy-up, judged separately. Do not
   build it.

**Surviving claim (implement exactly this):** add the missing
`Err(GitError::NotARepo) => Ok(None)` arm to `configured_template`, plus the two
tests named. Three lines, one behaviour fix.

**Plus (from the slice instruction):** extend the port doc at
`crates/core/src/git/repository.rs` to state the full **six-method** membership.

**C3 boundary — do not merge with S21.** S21-F1 is `Op → NotARepo` in the
*adapter* (manufacture, six copies, all currently correct). S11-F1 is
`NotARepo → Ok(None)` in the *core* (absorption, one copy missing — the live
bug). Different direction, crate, fix and blast radius. **S11 is the
authoritative owner.** The "test `commit_template` outside a repository" that
both lanes named is **one test, written once** — write it here.

## C — S12-F2 · **NARROWED**

> The earlier draft of this brief concluded "both functions are correctly
> implemented, coverage gap only". **That is wrong** and is corrected here.

Verifier read all six bodies in `crates/core/src/trust/requests.rs`: `record`,
`status`, `pending`, `peek` each take the lock, call
`prune_expired(&mut state, now)`, compute, release, then
`announce_resolved(expired)`. **`resolve` and `withdraw_requests_of` never
prune.** So denying or withdrawing a request already past its TTL files a
`Denied` / `Withdrawn` receipt and event where a read one instruction earlier
would have produced `Expired`. Also confirmed:
`crates/core/src/facade/trustrequest.rs::approve_trust_request` is safe only
because it `peek`s (which prunes) before `resolve`, and nothing in
`TrustRequests` states that dependency.

Narrowed on framing and materiality:

1. The security claim is **defence-in-depth only**. There is no current path that
   grants an expired request. Do not frame the fix or its tests as closing a
   security hole.
2. **Today's actual defect is a mislabelled receipt/event on two paths.** Small
   but real. That is what the tests must assert.
3. The `with_state` helper is **not** what fixes anything and is not required;
   `record`'s pre-lock hashing and post-unlock publish must stay outside any such
   closure anyway.

**Surviving claim (implement exactly this):** prune inside `resolve` and
`withdraw_requests_of`, plus two tests — resolve-after-expiry and
withdraw-after-expiry. Whether that arrives via `with_state` or two added lines
is an implementation choice, not the finding.
