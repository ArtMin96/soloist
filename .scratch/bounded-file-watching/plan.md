# Plan: bounded, gitignore-aware project file watching

**Scope:** full — 4 coupled changes across `core`, `sys`, the composition root and the UI; a shared
port must be reshaped before anything else can land, and the leaf work then fans out.

**Grounding (files actually read, whole definitions):**
`CLAUDE.md`, `ARCHITECTURE.md`, `plan/06` §1–§5.2,
`crates/core/src/filewatch/{watcher,policy,status,reactor}.rs`, `crates/core/src/watch.rs`,
`crates/core/src/git/{watch,watched,status}.rs` (`status.rs` §status/refresh/forget),
`crates/core/src/projects/config_watch.rs`, `crates/core/src/testing/filewatch.rs`,
`crates/core/src/events.rs` (`WatchRefusalChanged`), `crates/core/src/vcs.rs`,
`crates/core/src/composition.rs` + `crates/app/src/lib.rs` (`build_facade`, loop spawn order),
`crates/sys/src/{filewatch,proc}.rs` + `crates/sys/Cargo.toml`,
`crates/app/ui/src/{domain.ts,store/projection.ts,store/watchContext.ts,store/useWatchRefusals.ts,components/sidebar/WatchRefusedNotice.tsx}`,
`scripts/check-core-cycles.sh`, `justfile`, the research brief `watch-research.md`.

**External API verification (no invented signatures):**
- `notify` 8.2.0 read from source at
  `~/.cargo/registry/src/index.crates.io-*/notify-8.2.0/src/inotify.rs`:
  - per-path `is_recursive` flag in `watches: HashMap<PathBuf, (WatchDescriptor, WatchMask, bool, bool)>`
    (line 39–40) — **recursive and non-recursive registrations coexist on one watcher**.
  - `add_watch_by_event` (line 61) auto-adds a newly created directory **only when its parent is
    registered recursively** — so per-directory registration means writing our own add-on-create.
  - `add_single_watch` (line 419) re-registers an already-watched path with `MASK_ADD` and no error
    → `watch()` is **idempotent** and costs no second inotify watch.
  - `remove_watch` (line 473) returns `Err(WatchNotFound)` for an unregistered path → `unwatch` must
    be best-effort in our port.
  - `EventLoopMsg::RemoveWatch` (line 174) calls `remove_watch(path, false)` → `unwatch` drops the
    subtree only when the path was registered recursively.
  - event mapping (line 255–330): `CREATE`+`ISDIR` → `EventKind::Create(CreateKind::Folder)`;
    `MOVED_TO` → `EventKind::Modify(ModifyKind::Name(RenameMode::To))` with **no directory flag**;
    `DELETE`/`DELETE_SELF` → `EventKind::Remove(RemoveKind::Folder|File)`.
    Consequence: a directory moved in is indistinguishable from a file moved in, so the core must
    treat every appearance as a candidate and let the scanner (which stats) resolve it.
- `ignore` crate `WalkBuilder` verified against docs.rs (methods + documented defaults):
  `hidden(bool)` default **true**, `git_ignore` true, `git_global` true, `git_exclude` true,
  `ignore(bool)` true, `parents(bool)` true, `require_git(bool)` true, `standard_filters(bool)`,
  `filter_entry<P: Fn(&DirEntry) -> bool + Send + Sync + 'static>`, `max_depth(Option<usize>)`,
  `follow_links(bool)` default false, `same_file_system(bool)`, `overrides(Override)`,
  `build() -> Walk` yielding `Result<DirEntry, Error>`.
  Not currently in `Cargo.lock` — a genuinely new dependency (see Risks).
- `ignore` is **not** in the workspace today; `globset`, `walkdir`, `memchr`, `regex-automata`,
  `bstr`, `log` already are, so its marginal cost is mostly `crossbeam-*`.

---

## Contract

- [ ] Opening this repository (`/home/dell/Projects/soloist`) as a project registers **≤ 1,000**
      inotify watches for it, down from 58,179. Measured with
      `for f in /proc/$(pgrep -f 'soloist$' | head -1)/fdinfo/*; do grep -c '^inotify' "$f"; done | paste -sd+ | bc`.
- [ ] Three projects open simultaneously all get watches; none is refused because another took the
      budget. Verified by opening this repo + two others and checking each project's notice is absent.
- [ ] Soloist holds **one** inotify instance for file watching regardless of how many projects are
      open (was ~3–4 per project). Verified by counting `anon_inode:inotify` entries in
      `/proc/<pid>/fd`.
- [ ] Editing a tracked file anywhere in a non-ignored subdirectory still refreshes the git rail
      (`DomainEvent::GitStatusChanged`) and still fires `restart_when_changed`.
- [ ] Creating a **new** directory and immediately writing a file into it fires the matching
      `restart_when_changed` command — the create-then-populate race is closed, not merely narrowed.
- [ ] Deleting a watched directory releases its registrations: a start/open/close/open cycle of N
      projects ends at the same inotify watch count.
- [ ] A `restart_when_changed` glob whose prefix directory is **gitignored**
      (e.g. `dist/config.json`) still restarts its command — the gitignore filter must not silently
      shrink the restart contract.
- [ ] A project whose non-ignored tree exceeds its share of the watch budget is **degraded, not
      refused**: its `.git` state, `.git/refs`, its root directory and its glob-prefix directories
      stay watched, the UI says the live git status is limited, and the app keeps working.
- [ ] A watch refused once and later grantable is re-established on the next resync — for the git
      status reactor as well as the restart reactor (today it is permanently stuck).
- [ ] `just lint` and `just test` green, including `scripts/check-core-deps.sh` and
      `scripts/check-core-cycles.sh`.

**Must not change:**
- `solo.yml` schema and `restart_when_changed` glob semantics (`*` crosses separators).
- The debounce windows and their observable coalescing (`QUIET` 300 ms restarts / 100 ms git,
  `MAX_POSTPONE` 1 s), the retry-once-then-stop rule, and the `is_lock` suppression.
- `Facade` method signatures other than the watch loops; every existing Tauri command.
- Public `DomainEvent` variants other than the watch one being renamed here.
- No polling is introduced anywhere, in normal or degraded mode.

**Out of scope (do not build):**
- Raising `fs.inotify.max_user_watches` or advising the user to (the notice already names it).
- `git fsmonitor` integration (research brief §1: it relocates the cost and its IPC is git-internal).
- fanotify (research brief §2: `FAN_MARK_FILESYSTEM` needs `CAP_SYS_ADMIN`).
- Upgrading `notify` to v9.
- A user-triggered "refresh status now" command. There is none today —
  `Facade::git_status` serves `Git::status`, which returns the **cached** status when present
  (`crates/core/src/git/status.rs:317`) — and adding one is a separate UX change. Flagged in
  Open Questions because degraded mode has no manual escape hatch.

---

## Design decisions (the questions asked, answered)

### Where the watch-set policy lives, and how core stays free of the filesystem

Two things are separated:

1. **Enumeration (I/O)** is a new driven port, `WatchScanner`, declared in
   `crates/core/src/filewatch/scan.rs` and implemented in `crates/sys/src/watchscan.rs` over the
   `ignore` crate. It answers one question: *given this root, these always-ignored directory names,
   whether to honour the repository's own ignore rules, and this ceiling — what paths are there?*
   It makes no decision.
2. **Policy (pure)** lives in a new core module `crates/core/src/watchset/plan.rs`: given the scan
   results, the glob prefixes, the repository-state directories and the remaining budget, it
   produces the exact registration plan and the resulting `WatchLimit`. No clock, no I/O,
   exhaustively unit-testable — the same shape as `filewatch/policy.rs`.

`crates/core` gains no filesystem dependency: `scan.rs` is a trait plus value types, and
`NoopWatchScanner` (which reports just the root) is the default in `CorePorts`, so a build without
the real adapter watches each project's root and repository state and nothing deeper — which is
exactly the degraded mode, not a crash.

### How both call sites share it without duplicating the behaviour

Today three components each open their own watcher: `WatchReactor` (recursive root),
`GitStatusWatchReactor` via `git::watched::Watches` (`.git` + `.git/refs` + recursive root), and
`ConfigWatchReactor` (non-recursive root). That is the duplication.

**One owner replaces all three registrations:** `ProjectWatchSet` in `crates/core/src/watchset/`,
driven by its own supervised loop (`Facade::watch_set_loop`, spawned by the composition root before
the three reactors). It owns the single `WatchSession`, the per-project registration sets, the
budget, the incremental maintenance, and — because it is the only thing that knows what was
actually registered — the `WatchStatus` reporting for **both** purposes.

The three reactors become pure consumers: each is handed a `broadcast::Receiver<PathBuf>` and keeps
its existing matching + debouncing untouched. None of them names the watch set's module (they take
a tokio receiver), which is what keeps the module graph acyclic — see Risks.

### The port's new shape

The current port returns one handle per call, and `crates/sys/src/filewatch.rs:81` builds a fresh
`RecommendedWatcher` per handle. That is the ~4-instances-per-project ceiling. Replaced by a
session:

```rust
// crates/core/src/filewatch/watcher.rs
pub trait FileWatcher: Send + Sync {
    fn open(
        &self,
        changes: mpsc::Sender<FileChange>,
        dropped: Arc<AtomicU64>,
    ) -> Result<Arc<dyn WatchSession>, WatchError>;

    /// The most watches the backend will give, when it will say. `None` means unknown and the
    /// caller assumes a conservative default.
    fn capacity(&self) -> Option<usize>;
}

pub trait WatchSession: Send + Sync {
    fn watch_dir(&self, dir: &Path) -> Result<(), WatchError>;
    fn watch_tree(&self, root: &Path) -> Result<(), WatchError>;
    fn unwatch(&self, path: &Path);
}
```

`&self` (not `&mut self`) because registration runs on the blocking pool via
`crate::supervision::run_blocking`, which needs `'static + Send`; the adapter wraps its
`RecommendedWatcher` in a `Mutex`. Dropping the session stops every watch.

### How the budget is accounted and enforced

- `NotifyFileWatcher::capacity()` reads `/proc/sys/fs/inotify/max_user_watches` (same style as the
  existing `/proc` reads in `crates/sys/src/proc.rs`; no new dependency).
- `watchset::budget::Budget` (pure) holds `total = capacity / BUDGET_FRACTION`
  (`BUDGET_FRACTION = 2` — Soloist may hold half the machine's budget) or `ASSUMED_CAPACITY = 8_192`
  (the kernel default) when capacity is unknown, and tracks what is registered.
- **Per-project share** = `total / open_projects.max(1)`, recomputed on every resync. A project
  already holding more than its new share is re-planned against it (which will degrade it); a
  project inside its share is never disturbed — so opening a second project cannot silently degrade
  a small first one.
- The share is passed to the scanner as its `ceiling`, so the walk stops instead of materialising
  700k paths.
- **Allocation order within a project** (this is what makes degradation useful rather than total):
  1. `.git`, `.git/refs` (tree), the project root directory — always.
  2. the directories under each `restart_when_changed` glob's literal prefix — explicit user intent,
     budgeted before the speculative whole-tree scan.
  3. the whole-tree gitignore-filtered scan.
  If (3) does not fit, `GitStatus` reports `WatchLimit::Degraded` and `Restarts` is unaffected.
  If (2) does not fit, `Restarts` is degraded too. **The two purposes can therefore still diverge**,
  so `WatchStatus`'s whole-set comparison stays load-bearing.

### What degraded mode watches, and what it does not do

Watches: `.git` (non-recursive) + `.git/refs` (tree) + the project root (non-recursive) + the
glob-prefix directories. **It does not poll** — the owner's own reasoning (a 734k-directory repo
forced through a 1 s debounce ceiling is a permanent invisible CPU/IO drain) applies to a degraded
project exactly as it applies to raising the system limit. In practice the git rail still follows
every git operation, because `git commit`/`add`/`checkout`/`fetch` all write inside `.git`; what is
lost is a status refresh triggered by editing a file deep in the tree. The notice says exactly that.

### Incremental add/prune and the race-closing rescan

- The adapter→set channel carries `FileChange { path, kind: Appeared | Modified | Vanished }`.
  `Appeared` covers both `Create(_)` and `Modify(Name(RenameMode::To))`, because notify does not
  flag a moved-in directory (verified above), so the set cannot tell them apart and must ask.
- On `Appeared(p)`: the set calls `scanner.scan(ScanRequest { root: p, .. })` on the blocking pool.
  The scanner stats `p`; a file or an ignored path yields an empty scan. A directory yields **every
  non-ignored path at and beneath it** — which is the race-closing rescan: the walk happens *after*
  creation, so anything written between `mkdir` and our registration is still found. The set
  registers the directories and republishes every scanned path (capped at
  `MAX_APPEARED_REPLAY = 4_096`) onto the fan-out, so the restart reactor matches files that were
  created before their directory was watched.
- On `Vanished(p)`: the set drops `p` and every registration beneath it, calls
  `session.unwatch(p)` (best-effort), and returns the count to the budget.
- **Registrations are refcounted by owning project.** `git::watched` documents that projects nest
  ("a repository opened inside another project's tree"), so one directory can be wanted by two
  projects; `unwatch` fires only when the last owner drops it. Without this, closing the inner
  project blinds the outer one.
- **Dropped changes self-heal.** `open()` takes an `Arc<AtomicU64>` the adapter increments whenever
  `try_send` fails on the bounded channel. The set reads it on each drain; if it moved, it arms a
  `RESCAN_QUIET`-debounced full re-plan of every watched project. This is the existing
  `Err(RecvError::Lagged(_)) => resync` idiom applied to the mpsc, and it closes the one new
  correctness hole per-directory registration creates (a dropped `Appeared` for a directory would
  otherwise mean a permanently unwatched subtree). It is edge-triggered, not polling.
- **Determinism in tests:** `MockClock` drives every debounce; `FakeFileWatcher`/`FakeWatchSession`
  and a new `FakeWatchScanner` in `core::testing` supply the events and the scan answers, so no test
  touches the filesystem or real time. Adapter-level reality is proven separately in
  `crates/sys/tests/` against `tempfile` directories.

### Does the UI notice need a new state — yes

The announcement type changes from `WatchError` to:

```rust
pub enum WatchLimit { Refused(WatchError), Degraded }
```

and the event is renamed to match what it now carries. Exact serde output (externally tagged; a
newtype variant becomes an object, a unit variant a string):

| Rust | JSON |
|---|---|
| `WatchLimit::Refused(WatchError::BudgetExhausted)` | `{"refused":"budget_exhausted"}` |
| `WatchLimit::Degraded` | `"degraded"` |

```
{"type":"WatchLimitChanged","project":1,"limits":{"git_status":{"refused":"budget_exhausted"}}}
{"type":"WatchLimitChanged","project":1,"limits":{"restarts":"degraded","git_status":"degraded"}}
{"type":"WatchLimitChanged","project":1,"limits":{}}
```

`restarts` precedes `git_status` because `BTreeMap<WatchPurpose, _>` orders by the enum's
declaration order — the existing pinned literal at `crates/core/src/events_tests.rs:51` already
shows that order.

---

## Risks & decisions

- **`ignore` is a new dependency.** ~5 new transitive crates (`crossbeam-deque`/`-epoch`/`-utils`,
  `same-file`, `walkdir` — the last two already in the tree via `notify`). The alternative,
  walking with `walkdir` and batch-querying `git check-ignore` through the existing
  `crates/git` adapter, avoids the dependency but spawns a subprocess, needs a repository (so
  non-repo projects get nothing), and would pipe 700k paths for a large tree. `ignore` is the
  ripgrep author's crate, in-process, and handles `.gitignore` + `.git/info/exclude` + global
  excludes + nested ignore files — which is what makes `.claude/worktrees` (40,483 dirs, excluded
  via `.git/info/exclude`) disappear. **CLAUDE.md §6 requires the size cost be measured**, so T3
  records `just bundle-size` before and after.
- **`ignore`'s defaults are wrong for us and must be overridden.** `hidden(true)` would skip
  `.github`, `.vscode`, and every tracked dot-directory `git status` reads; `ignore(true)` would
  honour `.ignore`/`.rgignore` files git does not, shrinking the set below what git reads. T3 pins
  `hidden(false)`, `ignore(false)`, `git_ignore/global/exclude(true)`, `parents(true)`,
  `require_git(false)`, `follow_links(false)`, and skips `.git` and the `DEFAULT_IGNORES` names via
  `filter_entry`.
- **The gitignore filter would have broken `restart_when_changed`.** Brief fact #6 ("the correct
  watch set IS the set git status reads") is true for the git rail and **false** for restarts:
  `restart_when_changed: ["dist/config.json"]` works today (`dist` is in `DEFAULT_IGNORES` but
  `WatchRule::matches` is the post-hoc filter, and a gitignored path still matches a user glob) and
  would silently stop. Contained by making the watch set the **union** of the filtered scan and the
  glob-prefix scans (repository ignores disabled for the latter), and by budgeting the prefixes
  before the tree.
- **Module-graph cycle.** `scripts/check-core-cycles.sh` has no allow-list.
  `projects/config_watch.rs` imports `crate::filewatch` today, so putting the watch set inside
  `filewatch` and having it import `crate::projects` would create `filewatch → projects → filewatch`
  and fail the gate. Contained by (a) putting the set in a **new sibling module
  `crates/core/src/watchset/`**, and (b) handing the reactors a plain
  `tokio::sync::broadcast::Receiver<PathBuf>` rather than a type from `watchset`, so no reactor
  imports it. Also: `STATE_DIR`/`REFS_DIR` move from `git/watched.rs` to the dependency-free shared
  kernel `crates/core/src/vcs.rs`, because `watchset` and `git` both need them and
  `filewatch → git` would close a ring with the existing `git → filewatch`.
- **Fan-out loss asymmetry.** On `broadcast` `Lagged` the git reactor arms every project it watches
  (a status re-read is idempotent), while the restart reactor does nothing — a missed restart is a
  missed convenience, a spurious restart kills a running dev server. Stated in both tasks.
- **R2's accounting unit is the watched path, not the project.** A project has three watches with
  three different modes and two different reporting rules, so a refusal is not binary and a
  *partial* refusal is the ordinary case under budget pressure. Any per-project caching — an opaque
  handle `Vec`, or handles bucketed by role — leaves a granted watch pinning a refused sibling that
  is never retried. Deciding this in Wave 1 rather than Wave 3 costs nothing (T1 is still three
  files) and avoids writing the accounting twice, because T4's budget is keyed by path for the same
  reason. Full justification and the rejected alternative are in T1.
- **T2 is the largest task and cannot be split.** Changing a trait's shape and a serialized event's
  payload makes every call site fail to compile at once. It is kept mechanical (behaviour unchanged)
  and additive where possible: `open`/`WatchSession` are added *alongside* `watch`/`watch_dir`, which
  T6 deletes once the last caller is gone.
- **The Rust→TS rename is deliberate churn.** `WatchRefusalChanged` carrying `Degraded` would be a
  name that lies (CLAUDE.md §8/§15). T5 renames the event, the payload field, the TS type, the hook,
  and the component together, and the plan pins every new name so nothing is guessed.
- **A file may appear in more than one wave.** The invariant is *within-wave* disjointness only; the
  collision table below is grouped by wave for that reason. `filewatch/reactor.rs`,
  `git/watched.rs`, `testing/filewatch.rs` and `composition.rs` are legitimately touched twice.

---

## Wave 1 — the sticky refusal (runs alone: small, isolated, lands even if the rest slips)

### T1 — a git-status watch refused once is re-attempted on every resync

- **Owns (exclusive write):** `crates/core/src/git/watched.rs`,
  `crates/core/src/git/watched_tests.rs` (new), `crates/core/src/testing/filewatch.rs`
- **May read:** `crates/core/src/filewatch/status.rs`, `crates/core/src/git/watch.rs`,
  `crates/core/src/watch.rs`
- **Depends on:** nothing.
- **The bug.** `crates/core/src/git/watched.rs:108-115`:
  ```rust
  if let Entry::Vacant(slot) = self.handles.entry(project) {
      let established = establish(&self.watcher, &watched, self.changes.clone()).await;
      slot.insert(established.handles);   // inserts even when `handles` is EMPTY
      ...
  }
  ```
  On a total refusal an empty `Vec` is still inserted, so every later resync takes the
  `Entry::Occupied` branch, skips re-establishment, and replays the cached refusal for the life of
  the process. Recovery needs `release(id)` (re-open the project) or `release_all()` (a bus lag).
  Nothing polls; every reactor is edge-triggered. This contradicts
  `crates/core/src/filewatch/status.rs:23`, which documents that a refused root is retried on every
  resync "so a refusal that has since cleared is not permanent" — true for
  `filewatch/reactor.rs:190`, false here.
- **The shape decision (settled: per watched *path*, not per project).** There are **three**
  watches per project — `.git` non-recursive (line 195), `.git/refs` recursive (line 197), the root
  recursive (line 202) — so a refusal is not binary. A **partial** refusal is the ordinary case
  under budget pressure: the two cheap `.git` watches succeed (~30 watches) and the expensive root
  watch is refused, which yields a **non-empty** `handles` vec together with `refusal: Some(_)`.
  Two consequences:
  - "Insert only when `handles` is non-empty" fixes the total-refusal case alone. The partial case
    — the one R4's degraded mode deliberately produces — would still cache for ever.
  - Bucketing handles by role (a `metadata: Vec` plus a `tree: Option`) has the same defect one
    level down: `.git` granted while `.git/refs` is refused leaves a non-empty metadata bucket that
    is never re-attempted.

  So the unit of accounting is the **path**. Rejected alternative: keep the per-project `Vec`,
  retry only on total refusal, and document that a partially refused project stays degraded until
  re-opened. Rejected for three reasons. It leaves the user's failing case half-fixed — a project
  under budget pressure is *partially* refused, which is exactly the shape the bug reports.
  It would require weakening `crates/core/src/filewatch/status.rs:23`, which promises "both ask
  again for a refused root on every re-sync — deliberately, so a refusal that has since cleared is
  not permanent"; CLAUDE.md treats a doc that contradicts the code as a defect to fix, and fixing
  it downward here would delete a guarantee the restart reactor already honours
  (`filewatch/reactor.rs:190`). And per-path accounting is where R3/R4 land anyway — T4's
  `ProjectWatchSet` is keyed by path because the budget cannot be enforced without knowing which
  paths are held, and degraded mode is *defined* as "hold the `.git` paths, drop the tree path".
  Landing the stopgap would mean writing it twice.

  **This does not make T1 too big for the first wave.** It is a restructure of one 233-line module
  plus its new test file plus a three-line addition to a fake — three files, no signature visible
  to `git/watch.rs` changes, so nothing else in the tree moves. It stays the independent first wave
  that unblocks a project holding zero git watches.

- **Do:**
  1. **Collapse the three parallel `ProjectId`-keyed maps into one record.** `handles`, `refusals`
     and `watched` are three maps keyed by the same id, all describing one project, and their being
     separate is *what let the bug exist*: `establish`'s `Entry::Vacant` check consulted `handles`
     alone, so a project present there and stale in `refusals` was unreachable. One record, one
     source of truth (CLAUDE.md §15) — this is caused by the fix, not adjacent cleanup:
     ```rust
     pub(super) struct Watches {
         watcher: Arc<dyn FileWatcher>,
         changes: mpsc::Sender<PathBuf>,
         projects: HashMap<ProjectId, WatchedProject>,
     }

     /// One open project: where its status is read from, and what it has for each place it is
     /// watched.
     struct WatchedProject {
         watched: Watched,
         held: HashMap<PathBuf, Held>,
     }

     /// What a project has for one watched path: the live watch, or the refusal standing in its
     /// place until a re-sync gets one.
     enum Held {
         Watching(Box<dyn WatchHandle>),
         Refused(WatchError),
     }
     ```
  2. **Make the three watch targets addressable** instead of a hand-written array with a special
     case. Add to `Watched`:
     ```rust
     /// The three places this project's status can change: the directory holding its repository
     /// state, the refs tree inside it, and its working tree.
     fn targets(&self) -> [WatchTarget; 3]

     struct WatchTarget {
         path: PathBuf,
         recursive: bool,
         /// Whether losing this one is worth telling the user about. Only the working tree's is:
         /// a project that is not a repository has no `.git`, so a state-dir refusal is the
         /// ordinary case rather than a loss, and reporting it would put a notice on every
         /// project not under version control.
         reported: bool,
     }
     ```
     `.git` → `{ recursive: false, reported: false }`; `.git/refs` → `{ true, false }`;
     the root → `{ true, true }`. This is exactly the information the free `establish` function
     encodes today — the `filter_map(…ok())` for the first two and the `match` for the third; it
     just stops being positional.
  3. **Re-attempt exactly what is unheld.** `Watches::establish` becomes:
     - rebuild `watched` from `root` and store it (as today, so a project is matched and read at
       the path it has now);
     - for each `target` whose `held` entry is absent **or** `Held::Refused`, register it —
       `watch_dir` when `!recursive`, `watch` when `recursive` — and store `Watching` or `Refused`;
     - leave every `Held::Watching` untouched, so a project already watched still causes no churn;
     - return `WatchOutcome { project, refusal }`, where `refusal` is the stored `Refused` for the
       one `reported` target.
     Registration still goes to `run_blocking` as one batch, for the reason the current doc comment
     gives (a large repository must never park a runtime worker). Keep `traced` and its exact
     wording. A persistently refused path now logs on every re-sync rather than once — that is
     bounded, because re-syncs are edge-triggered by project lifecycle events, not by time, and it
     is the same volume `filewatch/reactor.rs:227` already produces. Do not "fix" it into a
     transition-only log; that is a change of behaviour with no caller asking for it.
  4. **Move `retain`, `release`, `release_all` onto the single map**, preserving their exact
     meanings:
     - `retain(&open)` → `self.projects.retain(|project, _| open.contains(project))` (one line
       where there were three, and no way for the three to disagree);
     - `release(project)` → clear that project's `held`, dropping its handles **and** forgetting its
       refusals, while keeping its `watched`. That is what the caller wants: `ProjectOpened` fires
       `release` because the directory may have been replaced (a fresh clone over a deleted
       checkout is a new inode), so a stale refusal must go with the stale handle;
     - `release_all()` → the same for every project.
     `projects_of` and `root_of` read `entry.watched` and keep their signatures, so
     `crates/core/src/git/watch.rs` needs no change at all.
  5. `crates/core/src/testing/filewatch.rs`: add `pub fn allow(&self, root: &Path)` beside
     `refuse`, removing `root` from the refused list, documented as its mirror — a test needs a way
     to state that a budget freed up.
  6. **The doc comments the code now has to match.** `Watches`' own doc (lines 73–80) says a
     re-sync "does not re-establish the watches it already holds" — make it say what is now true:
     it re-attempts the paths it does **not** hold, and leaves the ones it does alone.
     `crates/core/src/filewatch/status.rs:23` promises that "both ask again for a refused root on
     every re-sync — deliberately, so a refusal that has since cleared is not permanent." That was
     true of the restart reactor and false of this one. **This change makes the code match the doc,
     so `status.rs:23` needs no edit.** Re-read it when you are done and confirm in your report
     that it now reads true; do not weaken it.
- **Contract it establishes:** `Watches::establish` returns `WatchOutcome { project, refusal: None }`
  on the first resync after a refusal has cleared — for a total refusal and for a partial one alike
  — and re-registers only the paths it does not already hold. `retain`, `release`, `release_all`,
  `projects_of`, `root_of` keep their signatures and meanings, so no other file changes.
  (`WatchOutcome`'s field is still named `refusal` here; T2 renames it to `limit`.)
- **Tests (must be seen failing first):**
  - New file `crates/core/src/git/watched_tests.rs`, wired with
    `#[cfg(test)] #[path = "watched_tests.rs"] mod tests;` at the foot of `watched.rs`.
  - `a_cleared_total_refusal_is_established_on_the_next_resync`: `refuse` all three paths;
    `establish` → assert `refusal == Some(WatchError::BudgetExhausted)`; `allow` all three;
    `establish` again → assert `refusal.is_none()` and `watcher.live()` contains the root.
  - `a_partial_refusal_re_attempts_only_the_refused_path` — **the case option (b) cannot fix.**
    Refuse only the root, leaving `.git` and `.git/refs` grantable. `establish` twice, then
    `allow(root)` and `establish` a third time. Assert: `watcher.watched()` contains `.git` exactly
    **once** and the root **three** times (the healthy watch was never churned, the refused one was
    retried every time), and the third outcome's `refusal.is_none()`.
  - `a_refused_refs_tree_is_re_attempted_without_disturbing_the_state_dir` — the sub-case a
    two-bucket (metadata/tree) design would still miss. Refuse only `.git/refs`; `establish` twice;
    assert `.git/refs` was asked for twice while `.git` and the root were each asked once, and that
    `outcome.refusal.is_none()` throughout (a refs refusal is not `reported`).
  - `a_granted_watch_is_not_re_established_on_resync`: nothing refused; `establish` twice; assert
    each of the three paths appears exactly once in `watcher.watched()`.
  - `only_the_working_trees_refusal_is_reported`: refuse `.git` alone; assert
    `outcome.refusal.is_none()` and that the root watch is live.
  - `releasing_a_project_forgets_its_refusal`: refuse the root, `establish`, `allow`,
    `release(project)`, `establish` → assert no refusal. This is what proves `release` clears the
    whole `held` record rather than only the handles.
  - **How to prove they fail, in order.** (1) Run
    `a_partial_refusal_re_attempts_only_the_refused_path` and
    `a_refused_refs_tree_is_re_attempted_without_disturbing_the_state_dir` against the
    **unmodified** `watched.rs` — both fail, because `Entry::Occupied` short-circuits every resync.
    (2) Now write the naive fix ("insert only when `handles` is non-empty") and run them again —
    the partial-refusal test **still fails**. That is the demonstration, in the test suite itself,
    that per-path accounting is required rather than preferred; note it in your report. (3) Write
    the real fix and watch all six go green.
- **Verify:** `cargo test -p soloist-core git::watched` → the three new tests pass;
  `cargo test -p soloist-core` and `cargo clippy -p soloist-core --all-targets -- -D warnings` clean.

---

## Wave 2 — the shared contracts (runs alone: it settles every type the rest depends on)

### T2 — session-based watcher port, scanner port, and the limit vocabulary

- **Owns (exclusive write):**
  `crates/core/src/filewatch/watcher.rs`, `crates/core/src/filewatch/scan.rs` (new),
  `crates/core/src/filewatch/mod.rs`, `crates/core/src/filewatch/status.rs`,
  `crates/core/src/filewatch/status_tests.rs`, `crates/core/src/watch.rs`,
  `crates/core/src/vcs.rs`, `crates/core/src/events.rs`, `crates/core/src/events_tests.rs`,
  `crates/core/src/lib.rs`, `crates/core/src/composition.rs`,
  `crates/core/src/testing/filewatch.rs`, `crates/core/src/testing/watchscan.rs` (new),
  `crates/core/src/testing/mod.rs`,
  `crates/core/src/filewatch/reactor.rs`, `crates/core/src/filewatch/reactor_tests.rs`,
  `crates/core/src/git/watched.rs`, `crates/core/src/git/watched_tests.rs`,
  `crates/core/src/git/watch.rs`, `crates/core/src/git/watch_tests.rs`,
  `crates/sys/src/filewatch.rs`, `crates/sys/tests/filewatch.rs`
- **May read:** everything else; changes nothing outside the list.
- **Depends on:** T1 (it edits `watched.rs` after T1's restructure).
- **Sequence it as two halves, and stop between them.** The steps below are ordered so the
  workspace is green twice, not once. This matters because T2 is the only trait-shape change and a
  late failure here takes T3/T4/T5 with it.

  **Checkpoint A — "the new port exists and nobody uses it" (steps 1, 2, 8, 9, 11).** Define the
  session and scanner ports and satisfy them in all three implementors. Verified by
  `cargo test --workspace` green with **zero behaviour change**, because nothing calls `open` yet.
  This is where the real risk lives (the notify adapter); reaching it green retires it.
  **Stop and report here before continuing.**

  **Checkpoint B — the vocabulary (steps 3–7, 10, 12).** `WatchLimit`, the event rename, the
  `WatchStatus` plumbing, the mechanical `.refusal` → `.limit` updates, and the adapter's
  integration tests. Verified by `cargo test --workspace` plus the pinned JSON literals.

  Useful scheduling consequence: **T3 depends only on Checkpoint A**, T5 only on Checkpoint B, and
  T4 on both. If B slips, T3 can still be dispatched.

- **There are exactly three `FileWatcher` implementors and T2 owns all three** —
  `crates/sys/src/filewatch.rs:39` (`NotifyFileWatcher`),
  `crates/core/src/testing/filewatch.rs:178` (`FakeFileWatcher`),
  `crates/core/src/filewatch/watcher.rs:62` (`NoopFileWatcher`) — and the same three for
  `WatchHandle`. Verified by `rg "impl .*FileWatcher for" crates/`. There is no implementor outside
  this task's write set, which is what makes a trait-shape change safe to do in one pass.

- **Do:**
  1. **`filewatch/watcher.rs`** — add, *keeping* `watch`/`watch_dir` and `WatchHandle` so every
     existing caller still compiles:
     ```rust
     #[derive(Clone, Copy, PartialEq, Eq, Debug)]
     pub enum FileChangeKind { Appeared, Modified, Vanished }

     pub struct FileChange { pub path: PathBuf, pub kind: FileChangeKind }

     pub trait WatchSession: Send + Sync {
         fn watch_dir(&self, dir: &Path) -> Result<(), WatchError>;
         fn watch_tree(&self, root: &Path) -> Result<(), WatchError>;
         fn unwatch(&self, path: &Path);
     }
     ```
     and on `FileWatcher`:
     ```rust
     fn open(&self, changes: mpsc::Sender<FileChange>, dropped: Arc<AtomicU64>)
         -> Result<Arc<dyn WatchSession>, WatchError>;
     fn capacity(&self) -> Option<usize>;
     ```
     Doc `Appeared` as "created **or moved in** — the backend does not say whether a moved-in path
     is a directory, so the caller must ask", `unwatch` as best-effort (notify errors on an
     unregistered path), and `watch_tree` as "for a small bounded tree only". `NoopFileWatcher`
     returns a `NoopWatchSession` whose methods succeed and do nothing, and `capacity() -> None`.
  2. **`filewatch/scan.rs`** (new):
     ```rust
     pub struct ScanRequest {
         pub root: PathBuf,
         /// Directory names never descended into, whatever the repository says.
         pub ignored_names: Vec<String>,
         /// Whether the repository's own ignore rules apply. False for a directory a user's
         /// `restart_when_changed` glob names explicitly.
         pub honour_repository_ignores: bool,
         /// The most paths the walk may report before it stops and says it was cut short.
         pub ceiling: usize,
     }
     pub struct ScannedPath { pub path: PathBuf, pub directory: bool }
     pub struct Scan { pub paths: Vec<ScannedPath>, pub truncated: bool }

     pub trait WatchScanner: Send + Sync {
         /// Blocking: the caller reaches it off the runtime.
         fn scan(&self, request: ScanRequest) -> Scan;
     }

     #[derive(Clone, Copy, Default)]
     pub struct NoopWatchScanner;
     ```
     `NoopWatchScanner::scan` reports the root alone (as a directory) when it exists and nothing
     otherwise — document that a build without the real adapter watches each project's root and
     repository state and nothing deeper, which is the degraded mode rather than a failure.
  3. **`watch.rs`** — add and export:
     ```rust
     #[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
     #[serde(rename_all = "snake_case")]
     pub enum WatchLimit { Refused(WatchError), Degraded }
     ```
     with a doc comment giving both consequences. Change `WatchOutcome`'s field from
     `refusal: Option<WatchError>` to `limit: Option<WatchLimit>`. Keep `WatchError` and
     `WatchPurpose` exactly as they are.
  4. **`vcs.rs`** — move `STATE_DIR` (`".git"`) and `REFS_DIR` (`"refs"`) here from
     `git/watched.rs`, `pub`, with the doc comments they carry today. `vcs` depends on nothing, so
     both `git` and (later) `watchset` can name them without closing a ring.
     `git/watched.rs` imports them from `crate::vcs`; `LOCK_EXTENSION` stays where it is (one
     consumer).
  5. **`events.rs`** — rename the variant and its field:
     ```rust
     WatchLimitChanged { project: ProjectId, limits: BTreeMap<WatchPurpose, WatchLimit> },
     ```
     Rewrite its doc comment to name both a refusal and a degradation. Update `events_tests.rs`;
     the pinned literals become exactly:
     ```
     {"type":"WatchLimitChanged","project":1,"limits":{"git_status":{"refused":"budget_exhausted"}}}
     {"type":"WatchLimitChanged","project":1,"limits":{"restarts":{"refused":"unwatchable"},"git_status":{"refused":"unavailable"}}}
     {"type":"WatchLimitChanged","project":1,"limits":{}}
     ```
     and add one new case pinning `{"type":"WatchLimitChanged","project":1,"limits":{"git_status":"degraded"}}`.
  6. **`filewatch/status.rs`** — `type Refusals` becomes `type Limits = BTreeMap<WatchPurpose, WatchLimit>`;
     rename `refused`/`announced` state accordingly and publish `WatchLimitChanged`. The whole-set
     comparison in `settle` stays: the two purposes can still differ, because the budget is spent on
     glob-prefix directories before the whole-tree scan. Update `status_tests.rs` mechanically and
     add one case where `Restarts` is `Refused` while `GitStatus` is `Degraded`, asserting one
     announcement carrying both.
  7. **`composition.rs`** — add `watch_scanner: Arc<dyn WatchScanner>` to `CorePorts`, defaulting to
     `Arc::new(NoopWatchScanner)`, with a `CorePortsBuilder::watch_scanner` method matching the
     existing `file_watcher` one, and add it to the doc comment's port list at line 46.
  8. **`testing/filewatch.rs`** — extend `FakeFileWatcher` to implement `open`: it returns a
     `FakeWatchSession` recording `watch_dir`/`watch_tree`/`unwatch` calls in order, honouring
     `refuse`/`allow`, and routing `FakeFileWatcher::change` to registered paths using the same
     `covers` rule (`watch_tree` → prefix match, `watch_dir` → direct child). Add
     `pub fn registered(&self) -> Vec<PathBuf>` — the **live** set, paths held by a **live**
     session, so registrations on a dropped session disappear from it (mirroring `live()`'s existing
     semantics) — `pub fn unwatched(&self) -> Vec<PathBuf>` (the log of `unwatch` calls),
     `pub fn sessions_opened(&self) -> usize` (how many sessions `open` has handed out — the
     resource-lifecycle fact T4's supervision test needs to tell "rebuilt" from "reused"),
     `pub fn change_of(&self, path, kind: FileChangeKind)`, and
     `pub fn with_capacity(self, watches: usize) -> Self` backing `capacity()`. Keep every existing
     method working — the reactors still use them until T6.
  9. **`testing/watchscan.rs`** (new) — `FakeWatchScanner`: a map from root → the paths it reports,
     `pub fn reporting(root, paths: Vec<(&str, bool)>)`, `pub fn truncating(root)` to force
     `truncated: true`, `pub fn panicking_once(root)` to make the next scan of `root` panic (so a
     test can exercise the supervised loop's restart deterministically), and
     `pub fn requests(&self) -> Vec<ScanRequest>` so a test can assert
     `honour_repository_ignores` and the ceiling it was given. Export from `testing/mod.rs`.
  10. **Mechanical call-site updates only** in `filewatch/reactor.rs`, `git/watched.rs`,
      `git/watch.rs` and their test files: `WatchOutcome { refusal }` → `{ limit }`, wrapping the
      `WatchError` in `WatchLimit::Refused`. No behaviour change.
  11. **`crates/sys/src/filewatch.rs`** — implement `open` and `capacity` on `NotifyFileWatcher`,
      leaving `watch`/`watch_dir` untouched:
      - `open` builds **one** `RecommendedWatcher`. Its closure maps `notify::EventKind`
        exhaustively — verified against `notify-8.2.0/src/inotify.rs:230-330`:

        | `EventKind` | `FileChangeKind` |
        |---|---|
        | `Create(_)` | `Appeared` |
        | `Modify(Name(RenameMode::To))` | `Appeared` |
        | `Modify(Name(RenameMode::From))` | `Vanished` |
        | `Modify(Name(RenameMode::Both))` | **dropped** |
        | `Modify(Name(Any \| Other))`, `Modify(Data(_) \| Metadata(_) \| Any \| Other)` | `Modified` |
        | `Remove(_)` | `Vanished` |
        | `Access(_)`, `Any`, `Other` | dropped |

        Two of those need saying out loud. `MOVED_FROM` reaches us as `RenameMode::From` and is a
        **disappearance**, not a modification — notify has already dropped its own registration for
        the moved-away path (`remove_watch_by_event`, line 233), so if the core does not treat it as
        `Vanished` it keeps a registration for a directory that no longer exists and never refunds
        the budget. And `RenameMode::Both` is **redundant**: notify emits the `From` and the `To`
        events first and then a third summary event carrying both paths (lines 232–265), so acting
        on it would prune and re-scan the same rename twice.
        On a failed `try_send` the closure does `dropped.fetch_add(1, Ordering::Relaxed)`.
        `open` returns `Arc<NotifyWatchSession>` holding `Mutex<RecommendedWatcher>`.
      - **Lock it poison-safely, and do not re-roll a helper.** notify's `watch_inner` and
        `unwatch_inner` do `self.channel.send(msg).unwrap()` and `rx.recv().unwrap()`
        (`inotify.rs:55-60, 571-576`), so a dead event-loop thread panics *inside* our lock and
        poisons it — after which every later registration would fail on `.unwrap()`.
        `soloist_core::sync::lock` is **not** reachable: `sync` is `pub(crate)` and unexported.
        Reuse the idiom this crate already has at `crates/sys/src/metrics.rs:76-77` —
        `.lock().unwrap_or_else(|poisoned| poisoned.into_inner())` — and say in your report that you
        searched and reused it (CLAUDE.md §17). Never `unwrap()` a lock in a long-lived adapter.
      - `watch_dir` → `watcher.watch(dir, RecursiveMode::NonRecursive)`,
        `watch_tree` → `RecursiveMode::Recursive`, both mapping errors through the existing
        `refusal()`; `unwatch` → `watcher.unwatch(path)` discarding the result (notify returns
        `WatchNotFound` for a path already gone — document it).
      - `capacity` reads and parses `/proc/sys/fs/inotify/max_user_watches`, returning `None` if it
        cannot.
      Extend the module doc: one instance now backs many directories, and non-recursive registration
      means the core, not notify, adds newly created subdirectories.
  12. **`crates/sys/tests/filewatch.rs`** — add integration tests over a `tempfile` directory
      (**add** these; do not convert the existing `watch`/`watch_dir` tests, which still cover the
      legacy path that ships until T6 deletes it):
      - `one_session_reports_from_two_sibling_directories` — **write and run this first.** It is the
        earliest signal that the design holds: if a single `RecommendedWatcher` cannot serve two
        registered paths, everything downstream is wrong and you want to know in minutes.
      - `unwatching_one_directory_leaves_the_other_reporting`.
      - `a_file_in_a_subdirectory_of_a_watched_directory_does_not_report` — **MANDATORY, and it is
        only meaningful as a pair.** It asserts an *absence*, so it passes trivially when nothing is
        being delivered at all — a broken session, a full channel, a closure that never fires would
        all make it green. It must therefore assert, **on the same session and in the same test**,
        that a file created *directly* in the watched directory **does** report, before asserting
        that one created a level deeper does not. A green run then means "delivery works and is
        non-recursive"; without the positive half it means nothing.
        Observe it failing for the right reason: register the directory with `watch_tree` instead of
        `watch_dir` and confirm the negative half fails while the positive half still passes. If
        **both** halves fail you have a delivery problem, not a recursion result — stop and fix that
        before reading anything into the test.
      - `capacity_reports_the_systems_watch_limit` — `Some(n)` with `n > 0` on this host.
- **Contract it establishes (paste into dependents):** the two traits and the value types in step 1
  and 2 verbatim; `WatchLimit` and `WatchOutcome { project, limit }` from step 3;
  `crate::vcs::{STATE_DIR, REFS_DIR}` from step 4; the JSON literals from step 5;
  `CorePortsBuilder::watch_scanner` from step 7; the fake surfaces from steps 8–9.
- **Verify:** `cargo test --workspace` green (no behaviour changed, so every existing watch test
  still passes); `cargo clippy --workspace --all-targets -- -D warnings`;
  `./scripts/check-core-deps.sh` and `./scripts/check-core-cycles.sh` pass;
  `cargo test -p soloist-core events` shows the new pinned JSON.

---

## Wave 3 — the three implementations (T3, T4, T5 have disjoint write sets — safe to run concurrently)

### T3 — the gitignore-aware scanner adapter

- **Owns (exclusive write):** `crates/sys/src/watchscan.rs` (new), `crates/sys/src/lib.rs`,
  `crates/sys/Cargo.toml`, `crates/sys/tests/watchscan.rs` (new)
- **May read:** `crates/core/src/filewatch/scan.rs` (settled in T2), `crates/sys/src/filewatch.rs`,
  `crates/sys/src/proc.rs`
- **Depends on:** T2.
- **Given contract (do not open T2's files to learn this):**
  ```rust
  pub struct ScanRequest {
      pub root: PathBuf,
      pub ignored_names: Vec<String>,
      pub honour_repository_ignores: bool,
      pub ceiling: usize,
  }
  pub struct ScannedPath { pub path: PathBuf, pub directory: bool }
  pub struct Scan { pub paths: Vec<ScannedPath>, pub truncated: bool }
  pub trait WatchScanner: Send + Sync { fn scan(&self, request: ScanRequest) -> Scan; }
  ```
  All exported from `soloist_core`.
- **Do:**
  1. Add to `crates/sys/Cargo.toml`:
     `ignore = { version = "0.4", default-features = false }` — with a comment in the style of the
     existing dependency comments explaining *why* (it is the only in-process implementation of
     git's own ignore precedence: `.gitignore`, `.git/info/exclude`, global excludes, nested ignore
     files — which is what excludes `.claude/worktrees` and a Laravel `storage/` tree that no
     hardcoded name list can reach).
  2. `crates/sys/src/watchscan.rs`: `pub struct IgnoreWatchScanner;` implementing `WatchScanner`
     with a `WalkBuilder` configured **explicitly, not by default**:
     - `hidden(false)` — `.github`, `.vscode` and other tracked dot-directories are part of what
       `git status` reads;
     - `ignore(false)` — `.ignore`/`.rgignore` are not git's rules and must not shrink the set below
       what git reads;
     - `git_ignore(request.honour_repository_ignores)`,
       `git_global(request.honour_repository_ignores)`,
       `git_exclude(request.honour_repository_ignores)`,
       `parents(request.honour_repository_ignores)`;
     - `require_git(false)`;
     - `follow_links(false)` — a symlinked tree must not be walked twice or escape the project;
     - `filter_entry(...)` rejecting any entry whose file name is `.git` or is in
       `request.ignored_names`;
     - no `max_depth`.
     Collect into `Scan`, stopping at `request.ceiling` with `truncated: true`. A `DirEntry` error is
     skipped (an unreadable directory is not a failure of the scan). `directory` comes from
     `entry.file_type().is_some_and(|t| t.is_dir())`. The root itself is included.
  3. Export `IgnoreWatchScanner` from `crates/sys/src/lib.rs` and name it in the crate doc alongside
     `NotifyFileWatcher`.
  4. Measure the dependency cost (CLAUDE.md §6): run `just bundle-size` before and after and record
     both numbers in your report. If it exceeds a few hundred KB, say so rather than proceeding
     silently.
- **Tests (must be seen failing first):** `crates/sys/tests/watchscan.rs`, each building a real
  `tempfile` tree:
  - `a_gitignored_directory_is_not_reported`: `git init`-less tree with a `.gitignore` containing
    `build/`; assert `build` and its children are absent and `src` is present. Fails first by
    asserting the *absence* against a stub that returns everything — write the assertion, run it
    against an empty `scan` implementation returning all of `walkdir`, watch it fail.
  - `a_path_excluded_by_git_info_exclude_is_not_reported`: create `.git/info/exclude` with
    `worktrees/`; assert absent. (Use a real `git init` via `std::process::Command`; skip the test
    with a clear message if `git` is unavailable, do not silently pass.)
  - `a_tracked_dot_directory_is_reported`: `.github/workflows` present in the result — this is the
    test that fails if anyone leaves `hidden(true)`.
  - `an_ignored_name_is_never_descended_into`: `ignored_names: vec!["node_modules".into()]` with a
    deep `node_modules` tree; assert absent even without a `.gitignore`.
  - `repository_ignores_can_be_disabled`: same tree as the first test with
    `honour_repository_ignores: false`; assert `build` **is** reported — this is the
    `restart_when_changed`-on-a-gitignored-path case.
  - `a_walk_past_the_ceiling_says_it_was_cut_short`: ceiling 3 over a larger tree; assert
    `paths.len() == 3` and `truncated`.
  - `the_root_itself_is_reported_as_a_directory`.
- **Verify:** `cargo test -p soloist-sys watchscan` → all pass;
  `cargo clippy -p soloist-sys --all-targets -- -D warnings`; `just bundle-size` numbers recorded.

### T4 — the watch set: one owner, one session, budgeted and incrementally maintained

- **Owns (exclusive write):** `crates/core/src/watchset/mod.rs` (new),
  `crates/core/src/watchset/plan.rs` (new), `crates/core/src/watchset/plan_tests.rs` (new),
  `crates/core/src/watchset/budget.rs` (new), `crates/core/src/watchset/budget_tests.rs` (new),
  `crates/core/src/watchset/set.rs` (new), `crates/core/src/watchset/set_tests.rs` (new),
  `crates/core/src/filewatch/policy.rs`, `crates/core/src/filewatch/policy_tests.rs`,
  `crates/core/src/lib.rs`
- **May read:** `crates/core/src/filewatch/{watcher,scan,status}.rs`, `crates/core/src/watch.rs`,
  `crates/core/src/vcs.rs`, `crates/core/src/{projects,supervisor,debounce,events,ports,supervision}`,
  `crates/core/src/testing/`
- **Depends on:** T2.
- **Given contract (do not open T2's or T3's files to learn this):**
  ```rust
  // soloist_core::filewatch
  pub enum FileChangeKind { Appeared, Modified, Vanished }
  pub struct FileChange { pub path: PathBuf, pub kind: FileChangeKind }
  pub trait WatchSession: Send + Sync {
      fn watch_dir(&self, dir: &Path) -> Result<(), WatchError>;
      fn watch_tree(&self, root: &Path) -> Result<(), WatchError>;
      fn unwatch(&self, path: &Path);
  }
  pub trait FileWatcher: Send + Sync {
      fn open(&self, changes: mpsc::Sender<FileChange>, dropped: Arc<AtomicU64>)
          -> Result<Arc<dyn WatchSession>, WatchError>;
      fn capacity(&self) -> Option<usize>;
      // plus the legacy watch/watch_dir, which T6 deletes — do not use them
  }
  pub struct ScanRequest { pub root: PathBuf, pub ignored_names: Vec<String>,
                           pub honour_repository_ignores: bool, pub ceiling: usize }
  pub struct ScannedPath { pub path: PathBuf, pub directory: bool }
  pub struct Scan { pub paths: Vec<ScannedPath>, pub truncated: bool }
  pub trait WatchScanner: Send + Sync { fn scan(&self, request: ScanRequest) -> Scan; }
  // soloist_core::watch
  pub enum WatchLimit { Refused(WatchError), Degraded }
  pub struct WatchOutcome { pub project: ProjectId, pub limit: Option<WatchLimit> }
  // soloist_core::vcs
  pub const STATE_DIR: &str = ".git";
  pub const REFS_DIR: &str = "refs";
  ```
  Fakes available in `core::testing`: `FakeFileWatcher` (`refuse`, `allow`, `with_capacity`,
  `registered()`, `unwatched()`, `change_of(path, kind)`), `FakeWatchScanner`
  (`reporting`, `truncating`, `panicking_once`, `requests()`), `MockClock`, `FakeProjectRepo`.
- **Do:**
  1. **`filewatch/policy.rs`** — add one pure function beside `compile`:
     ```rust
     /// The directory a glob is anchored at: its leading components up to the first that carries a
     /// glob metacharacter (`*`, `?`, `[`, `{`). `None` for a pattern anchored at the root itself.
     pub(crate) fn literal_prefix(pattern: &str) -> Option<PathBuf>
     ```
     Make it `pub(crate)` and re-export it from `filewatch` so `watchset` can use it. Cover in
     `policy_tests.rs`: `"src/**/*.rs"` → `src`; `"dist/config.json"` → `dist`;
     `"*.toml"` → `None`; `"a/b/c.txt"` → `a/b`; `"a/[bc]/d"` → `a`; `"{a,b}/c"` → `None`.
     **Prove failing:** write the table first against an unimplemented function.
  2. **`watchset/budget.rs`** — pure:
     ```rust
     const BUDGET_FRACTION: usize = 2;      // Soloist may hold half the machine's watch budget
     const ASSUMED_CAPACITY: usize = 8_192; // the kernel default, when the OS will not say
     pub(crate) struct Budget { total: usize, spent: usize }
     impl Budget {
         pub(crate) fn new(capacity: Option<usize>) -> Self;
         pub(crate) fn share(&self, open_projects: usize) -> usize;  // total / max(1, n)
         pub(crate) fn spend(&mut self, watches: usize);
         pub(crate) fn refund(&mut self, watches: usize);
         pub(crate) fn remaining(&self) -> usize;
     }
     ```
     Tests: an unknown capacity yields `ASSUMED_CAPACITY / 2`; the share halves when a second
     project opens; spend/refund round-trips to the same `remaining`; `remaining` saturates at 0
     rather than underflowing.
  3. **`watchset/plan.rs`** — the pure policy. No clock, no I/O; it is handed scan results:
     ```rust
     pub(crate) struct ProjectPlan {
         pub(crate) trees: Vec<PathBuf>,        // .git/refs only
         pub(crate) directories: Vec<PathBuf>,  // everything registered non-recursively
         pub(crate) limit: HashMap<WatchPurpose, WatchLimit>,
     }
     pub(crate) fn plan(
         root: &Path,
         globs: &[String],
         tree: &Scan,          // the repository-ignore-honouring scan of the root
         prefixes: &[Scan],    // one per distinct glob literal prefix, ignores disabled
         share: usize,
     ) -> ProjectPlan
     ```
     Allocation order, which is the whole point:
     1. always: `root`, `root/STATE_DIR`, and `root/STATE_DIR/REFS_DIR` as a tree;
     2. then the `prefixes` directories — explicit user intent;
     3. then the `tree` directories.
     If (3) does not fit in `share` (or `tree.truncated`), drop it entirely and set
     `GitStatus => Degraded`. If (2) does not fit, drop it too and set `Restarts => Degraded` as
     well. Never register a partial tree scan — half a tree is a watch set that lies about coverage.
     Tests (all pure, no fakes needed): a small tree fits and produces no limit; an oversized tree
     degrades `GitStatus` only and still contains every prefix directory; oversized prefixes degrade
     both; a truncated scan degrades even when the count would fit; `.git` and `.git/refs` are
     present in every outcome including the fully degraded one; a project with no globs asks for no
     prefixes and reports no `Restarts` entry at all.
     **Prove failing:** the "prefixes survive degradation" test fails against a naive
     implementation that simply drops everything past `share` in scan order.
  4. **`watchset/set.rs`** — `ProjectWatchSet` plus its loop.
     State:
     ```rust
     struct Registration { owners: HashSet<ProjectId>, tree: bool }
     // path -> Registration : refcounted, because projects nest (git::watched::projects_of
     // documents a repository opened inside another project's tree). unwatch fires only when the
     // last owner drops it.
     ```
     plus `HashMap<ProjectId, Registered { root, paths: HashSet<PathBuf>, limit }>`, the `Budget`,
     `Arc<dyn WatchSession>` (opened lazily on first resync; a failed `open` makes every project
     report `Refused(Unavailable)` and is retried on the next resync), and a
     `broadcast::Sender<PathBuf>` with `FANOUT_CAPACITY = 1_024`.
     Public surface:
     ```rust
     pub struct ProjectWatchSet { /* … */ }
     impl ProjectWatchSet {
         pub(crate) fn new(clock, watcher, scanner, bus: &EventBus,
                           projects: Arc<Projects>, supervisor: Weak<Supervisor>,
                           status: Arc<WatchStatus>) -> Self;
         pub fn subscribe(&self) -> broadcast::Receiver<PathBuf>;
         /// Runs under `supervision::supervise`, which restarts a panicked loop after a
         /// backoff — so this makes a **fresh** loop future each time it is called.
         pub async fn run(self);
     }
     ```
     **Shape constraint — get this right now, it is expensive to retrofit.** `supervise` takes
     `FnMut() -> Fut` and calls it once per restart, so the loop future must be re-creatable:
     shape this exactly like `crates/core/src/metrics/sampler.rs:78`
     (`supervise(clock, move || self.clone().run_loop()).await`).

     **The rule, stated so it cannot be got subtly wrong: per-run state is created inside
     `run_loop`, never held in the `Arc`.**

     | Lives in the `Arc` (survives a restart) | Created inside `run_loop` (rebuilt on restart) |
     |---|---|
     | `clock`, `watcher`, `scanner`, `bus`, `projects`, `supervisor`, `status`, the fan-out `broadcast::Sender` | the `WatchSession`, the per-path registration map, the `Budget`'s spent count, the debouncers, the `dropped` counter |

     **Why this is load-bearing rather than stylistic.** The loop panics for a reason, and the most
     likely reason is that the session itself is broken — notify's event-loop thread has died, so
     its internal `unwrap()`s panic and every later registration fails. If the session and the
     registration map lived in the `Arc` they would survive that panic: the restarted run would
     resync, see every path already registered, register nothing, and go on holding a **dead**
     session for the life of the process. File watching would be silently and permanently gone —
     the exact "a watch that yields no events looks like a tree nobody edits" failure this whole
     subsystem is written to avoid, now made unrecoverable. With per-run state the restart re-opens
     the session and re-plans from the registry, so the fault self-heals. `resync` is already
     idempotent and driven by `Projects::list()`, so this costs no extra code — a rebuilt run
     re-plans every project correctly on its own.
     `run()`:
     - resync once at start, then on `DomainEvent::{ProjectOpened, ProjectRemoved, ConfigChanged}`
       and on `RecvError::Lagged` (rebuild every project — a lag may have hidden a re-created root);
       break on `RecvError::Closed`.
     - resync: read `Projects::list()` and `Supervisor::watch_targets()`; recompute
       `budget.share(open.len())`; for each project **re-plan only if** it is new, has no
       registrations, holds more than the new share, or its globs changed; run the scans through
       `run_blocking` (never on a runtime worker); apply the plan by diffing against what is held —
       `watch_dir`/`watch_tree` for additions, `unwatch` for removals whose owner set empties;
       release everything for projects no longer open. Report to `WatchStatus` **twice**:
       `resynced(WatchPurpose::GitStatus, …)` for every open project, and
       `resynced(WatchPurpose::Restarts, …)` for the subset whose commands declare compilable globs.
     - drain `changes_rx`: for `Appeared`, queue the path for a scan; for `Vanished`, drop the path
       and everything beneath it and refund the budget; republish every change's path on the
       broadcast regardless (a full fan-out buffer drops for a receiver, which the receivers
       tolerate).
     - the queued `Appeared` scans run batched on the next drain boundary through `run_blocking`
       with `honour_repository_ignores: true` and the project's remaining share as the ceiling;
       register the directories found and republish every scanned path, capped at
       `MAX_APPEARED_REPLAY = 4_096`, so a file written into a directory before we could watch it
       still reaches the restart reactor. That is the create-then-populate race closed.
     - read the `dropped` counter on each drain; when it has moved, arm a `MockClock`-driven
       `RESCAN_QUIET = 500ms` debounce that triggers a full re-plan of every watched project.
       Log once per transition, not per drop.
     - dropping the set (loop exit) drops the session, releasing every OS watch.
  5. **`watchset/mod.rs`** — module doc stating what it is: the single owner of the app's filesystem
     watch registrations, serving the config reload, the restart policy and the git rail from one
     session, and why it is a sibling of `filewatch` rather than inside it (it names `projects` and
     `supervisor`, which name `filewatch`; `scripts/check-core-cycles.sh` has no allow-list).
     Re-export `ProjectWatchSet`. Add `pub mod watchset;` and the `ProjectWatchSet` re-export to
     `crates/core/src/lib.rs`.
  6. Constants live once, at the top of the module that owns them, with the reason in the doc
     comment: `CHANGE_BUFFER = 1_024`, `FANOUT_CAPACITY = 1_024`, `MAX_APPEARED_REPLAY = 4_096`,
     `RESCAN_QUIET = 500ms`, `BUDGET_FRACTION`, `ASSUMED_CAPACITY`.
- **Tests (must be seen failing first), `set_tests.rs`, all on `MockClock` + the fakes:**
  - `one_session_backs_every_project`: two projects opened; assert `FakeFileWatcher` was asked to
    `open` exactly once.
  - `only_the_scanners_directories_are_registered`: `FakeWatchScanner` reporting three of five
    directories; assert `registered()` is exactly those three plus root/`.git`/`.git/refs`.
  - `a_created_directory_is_registered_and_its_contents_replayed`: feed
    `change_of("<root>/src/new", Appeared)` with the scanner reporting `new` plus a file inside it;
    assert the directory was registered **and** the file path reached a `subscribe()` receiver.
    **This is the race test** — it fails against any implementation that registers the directory but
    does not replay what the scan found.
  - `a_vanished_directory_is_unwatched`: assert `unwatched()` contains it and the budget refunds.
  - `a_directory_two_projects_share_survives_one_closing`: open nested projects A and B over the
    same directory; remove B; assert the shared path is **not** in `unwatched()` and a change there
    still reaches the receiver. **Fails against a non-refcounted set.**
  - `an_oversized_project_is_degraded_not_refused`: `with_capacity(64)`; assert the announced limit
    is `GitStatus => Degraded`, `.git`/`.git/refs`/root are registered, and a `.git/index` change
    still reaches the receiver.
  - `a_glob_prefix_directory_survives_degradation`: a command with `restart_when_changed:
    ["dist/**/*.json"]` on an oversized project; assert `dist` is registered and `Restarts` carries
    no limit.
  - `a_gitignored_glob_prefix_is_scanned_with_repository_ignores_disabled`: assert
    `FakeWatchScanner::requests()` contains a request for `dist` with
    `honour_repository_ignores == false`.
  - `a_dropped_change_arms_a_full_rescan`: bump the `dropped` counter, advance `MockClock` past
    `RESCAN_QUIET`, assert the scanner was asked again for every open project.
  - `a_refused_session_is_retried_on_the_next_resync`: `refuse` the open, resync (assert
    `Refused(Unavailable)` announced), `allow`, resync again, assert the limit is withdrawn.
  - `a_project_refused_once_is_established_when_the_refusal_clears`: **this is the successor to
    T1's regression test and is what lets T6 delete `git/watched_tests.rs` honestly.** Refuse one
    project's root while a second project is granted; resync; assert the first announces
    `Refused(BudgetExhausted)` and the second announces nothing. `allow()` the first root; resync
    again **without** removing or re-opening the project; assert its limit is withdrawn, its
    directories are in `registered()`, and a change under its root reaches a `subscribe()`
    receiver. Written this way it fails against any implementation that caches a per-project answer
    and skips re-establishment — which is exactly the bug T1 fixed, now guarded one layer up.
  - `closing_the_last_project_releases_every_watch`: assert `registered().len() ==
    unwatched().len()` after removal — the start/stop-loop-returns-to-baseline invariant.
  - `a_panicked_loop_rebuilds_its_watches_without_doubling_them` — **the supervision test.** Open
    one project and record `watcher.registered().len()` (the *live* registration set) as `n`. Make
    the next scan panic (`FakeWatchScanner::panicking_once`, which propagates through
    `run_blocking`'s deliberate `resume_unwind` at `supervision.rs:62`), then advance `MockClock`
    past `supervision::INITIAL_BACKOFF` (200 ms) so `supervise` restarts the loop. Assert three
    things, because no one of them is sufficient alone:
    1. `watcher.registered().len() == n` — **not** `2 * n`. The previous run's registrations went
       with its session and the rebuilt run re-registered the same set rather than stacking a
       second one. This is the leak assertion.
    2. `watcher.sessions_opened() == 2` — a **fresh** session was opened rather than the wedged one
       reused. This is the assertion that actually discriminates: if both the session and the
       registration map were (wrongly) held in the `Arc`, assertion 1 **still passes** — the count
       stays `n` because nothing was re-registered — while the loop goes on holding a dead session.
       It counts OS handles created, which is a resource-lifecycle fact the fake already makes
       observable elsewhere (`live()`), not an assertion about how the loop was called.
    3. A synthetic change under the project root still reaches a `subscribe()` receiver — the
       rebuilt session is delivering, end to end.
    **Prove it fails:** run it against a `ProjectWatchSet` that holds the session and registration
    map in the `Arc`; assertion 2 fails. Then move them into `run_loop` and watch it pass.
- **Verify:** `cargo test -p soloist-core watchset` and `cargo test -p soloist-core filewatch::policy`
  → all pass; `cargo clippy -p soloist-core --all-targets -- -D warnings`;
  `./scripts/check-core-cycles.sh` passes (this is the gate the new module exists to satisfy);
  `./scripts/check-file-size.sh` reports no new file over the ~400 non-test-line smell.

### T5 — the UI says "degraded", not only "refused"

- **Owns (exclusive write):** `crates/app/ui/src/domain.ts`,
  `crates/app/ui/src/store/projection.ts`, `crates/app/ui/src/store/projection.test.ts`,
  `crates/app/ui/src/store/watchContext.ts`,
  `crates/app/ui/src/store/useWatchRefusals.ts` → **renamed to** `useWatchLimits.ts`,
  `crates/app/ui/src/store/useWatchRefusals.test.ts` → **renamed to** `useWatchLimits.test.ts`,
  `crates/app/ui/src/components/sidebar/WatchRefusedNotice.tsx` → **renamed to**
  `WatchLimitNotice.tsx`,
  `crates/app/ui/src/components/sidebar/WatchRefusedNotice.test.tsx` → **renamed to**
  `WatchLimitNotice.test.tsx`,
  `crates/app/ui/src/components/sidebar/ProjectGroup.tsx`,
  `crates/app/ui/src/components/sidebar/ProjectGroup.test.tsx`
- **May read:** `crates/app/ui/src/components/AdvisoryNotice.tsx`, `crates/app/ui/src/lib/utils.ts`
- **Depends on:** T2 (for the wire shape only — everything you need is below).
- **Given contract (do not open any Rust file to learn this):** the event is renamed and its payload
  is now a union.
  ```
  before: {"type":"WatchRefusalChanged","project":1,"refusals":{"git_status":"budget_exhausted"}}
  after:  {"type":"WatchLimitChanged","project":1,"limits":{"git_status":{"refused":"budget_exhausted"}}}
          {"type":"WatchLimitChanged","project":1,"limits":{"restarts":"degraded","git_status":"degraded"}}
          {"type":"WatchLimitChanged","project":1,"limits":{}}
  ```
  `restarts` always precedes `git_status` in the object. `WatchError` and `WatchPurpose` are
  unchanged.
- **Do:**
  1. `domain.ts` — under the "Filesystem watches" section keep `WATCH_ERRORS`/`WatchError`/
     `WatchPurpose`, and replace `PurposeRefusals` with:
     ```ts
     // What limits a project's watching. A refusal means nothing is watched for that purpose; a
     // degradation means only the repository's own state and the directories the project names
     // explicitly are, because its tree needs more watches than its share of the system's budget.
     export type WatchLimit = { refused: WatchError } | "degraded";
     export type PurposeLimits = Partial<Record<WatchPurpose, WatchLimit>>;
     ```
     Rename the `DomainEvent` member to
     `{ type: "WatchLimitChanged"; project: number; limits: PurposeLimits }` and rewrite its comment
     to name both outcomes.
  2. `store/projection.ts` — rename the case in the exhaustive switch to `"WatchLimitChanged"`.
     (`tsc --noEmit` will point at every other site if one is missed.)
  3. `store/watchContext.ts` — `WatchRefusals` → `WatchLimits` (`ReadonlyMap<number, PurposeLimits>`),
     `useWatchRefusal` → `useWatchLimit`, `NOTHING_REFUSED` → `NOTHING_LIMITED`,
     `WatchContext` keeps its name.
  4. `store/useWatchLimits.ts` (renamed file, delete the old) — `useWatchLimits`, reading
     `event.limits`; `alreadyHeld` compares `WatchLimit` values, which are now either a string or an
     object, so compare with a small `sameLimit(a, b)` helper rather than `===`.
     **Search first (CLAUDE.md §17): run `find-similar-functions` for "shallow compare union value"
     before writing `sameLimit`, and say what you found.**
  5. `components/sidebar/WatchLimitNotice.tsx` (renamed file, delete the old) — keep the
     `CONSEQUENCE`/`CAUSE` single-source tables and the `AdvisoryNotice` shell. Add a third table
     for the degraded case and branch on the union:
     - refused (unchanged sentence): `Not watching this project's files for ${losses}, so ${stopped}
       stopped.` + the existing `CAUSE` sentence.
     - degraded: name what still works and what does not, honestly — the git rail still follows every
       commit, stage, checkout and fetch, because those write inside `.git`; what stops is a refresh
       triggered by editing a file deep in the tree. Suggested copy, adjust for tone:
       `Watching only this project's repository state — its file tree needs more watches than the
       system allows. Live git status still follows commits and staging; edits to your files will
       not refresh it on their own.`
       For `restarts` degraded: `restart-on-change only sees the directories your patterns name.`
     - a project with one purpose refused and the other degraded must read as two distinct
       sentences, not one merged claim.
     Rename the exported component to `WatchLimitNotice` and update `ProjectGroup.tsx`'s import and
     the prop name (`refusals` → `limits`).
  6. Keep the `break-words` wrap fix and `urgency="status"` exactly as they are — the rail is narrow
     and `fs.inotify.max_user_watches` is still one long unbreakable token.
  7. **UI gate:** format with `./node_modules/.bin/prettier` (not `npx`), and run UI tooling under
     Node 22 (`~/.nvm/versions/node/v22.18.0/bin`) — the shell default Node 18 breaks Vite.
- **Tests (must be seen failing first):**
  - `useWatchLimits.test.ts` (renamed): existing cases updated to `WatchLimitChanged`/`limits`, plus
    one asserting a repeat `"degraded"` announcement does not re-render (the `alreadyHeld` path with
    a string value) and one asserting a change from `{refused: …}` to `"degraded"` **does**.
    The second fails against a `sameLimit` written as `===` on objects.
  - `WatchLimitNotice.test.tsx` (renamed): existing refusal cases kept; new cases for
    `{git_status: "degraded"}` (asserts the notice names live git status and does **not** claim it
    stopped), `{restarts: "degraded", git_status: "degraded"}`, and the mixed
    `{restarts: {refused: "unwatchable"}, git_status: "degraded"}` (asserts both sentences appear).
    Write the degraded assertions first against the old component — they fail because it renders
    "so it has stopped" for anything present.
  - `projection.test.ts`: the renamed event still leaves the process list untouched.
- **Verify:** `pnpm -C crates/app/ui exec vitest run` (or `just test`) green;
  `pnpm -C crates/app/ui exec tsc --noEmit` clean — this is what proves no rename site was missed;
  ESLint/Prettier clean.

---

## Wave 4 — the rewire (runs alone: the facade, the composition root and all three reactors move together)

### T6 — the three reactors become consumers of one watch set

- **Owns (exclusive write):**
  `crates/core/src/filewatch/reactor.rs`, `crates/core/src/filewatch/reactor_tests.rs`,
  `crates/core/src/filewatch/watcher.rs`, `crates/core/src/filewatch/mod.rs`,
  `crates/core/src/git/watch.rs`, `crates/core/src/git/watch_tests.rs`,
  `crates/core/src/git/watched.rs`, `crates/core/src/git/watched_tests.rs`,
  `crates/core/src/projects/config_watch.rs`, `crates/core/src/projects/config_watch_tests.rs`,
  `crates/core/src/facade.rs`, `crates/core/src/facade/loops.rs`,
  `crates/core/src/composition.rs`, `crates/core/src/testing/filewatch.rs`,
  `crates/sys/src/filewatch.rs`, `crates/sys/tests/filewatch.rs`,
  `crates/sys/tests/config_watch.rs`, `crates/app/src/lib.rs`
- **May read:** `crates/core/src/watchset/` (settled in T4), `crates/core/src/filewatch/scan.rs`
- **Depends on:** T3, T4, T5.
- **Why one task:** all three reactors change constructor shape, and all three are constructed in
  `facade/loops.rs` and wired in `composition.rs` and `app/src/lib.rs`. Splitting by reactor would
  put two tasks in the same three files.
- **Do:**
  1. `facade.rs` / `facade/loops.rs` — hold `Arc<ProjectWatchSet>`, add
     `pub fn watch_set_loop(&self) -> impl Future<Output = ()> + Send + 'static`, and change the
     three existing loops to build their reactor with `watch_set.subscribe()` instead of
     `file_watcher` + `watch_status`. `WatchStatus` is constructed once and handed only to the
     watch set.
     **`watch_set_loop` must be wrapped in `supervision::supervise`** —
     `supervise(self.clock.clone(), move || set.clone().run_loop())`, matching the six existing
     call sites (`metrics/sampler.rs:78`, `agents/idle/sampler.rs:65`, `portscan/scanner.rs:56`,
     `coordination/{template_evictor.rs:55, mailbox/reactor.rs:107, scheduler.rs:69}`).
     This is required by the consolidation, not an adjacent improvement. The three watch loops are
     **not** supervised today (verified: `supervise(` has six call sites and none is a watch
     reactor), so a panic in one kills that one feature. After this rework all three features —
     config reload, restart-on-change, the git rail — flow through the *single* watch-set loop, so
     leaving it unsupervised would turn three independent failure domains into one silent one. It
     also gains a new panic source: `run_blocking` **re-raises** panics by design
     (`supervision.rs:62`, `resume_unwind`) and notify's internal `unwrap()`s can panic on a dead
     event-loop thread. Note that `supervision.rs:3` already names "the file watcher" as an
     intended `supervise` target — this wires an intent the module doc has carried unfulfilled.
  2. `filewatch/reactor.rs` — delete `resync`'s watch establishment, `establish`, the
     `watches: HashMap<PathBuf, Box<dyn WatchHandle>>` field and the `WatchStatus` reporting.
     `WatchReactor` keeps `clock`, `events`, `supervisor`, and gains
     `changes: broadcast::Receiver<PathBuf>`. `resync` now only rebuilds `rules` from
     `watch_targets()`. On `RecvError::Lagged` from the **path** stream: do nothing (a missed
     restart is a missed convenience; arming every rule would restart running dev servers
     spuriously) — document that choice in the loop.
  3. `projects/config_watch.rs` — drop the `watcher` field, the `watches` map and the
     `use crate::filewatch::…` import entirely; take `broadcast::Receiver<PathBuf>`. `resync` now
     only rebuilds `config_paths`. Removing that import is what keeps
     `projects → filewatch → projects` from ever forming.
  4. `git/watched.rs` — `Watches` becomes pure routing: keep the `watched` half of each
     `WatchedProject` record, `projects_of`, `root_of` and `retain`; delete `held`, `Held`,
     `WatchTarget`, `Watched::targets`, `establish`, `release`, `release_all` and the free
     `establish`/`traced` functions. T1's structure goes with them, and `git/watched_tests.rs` is
     deleted — but **only because the guarantee moved, not because it evaporated**: T4's
     `a_project_refused_once_is_established_when_the_refusal_clears` asserts the same behaviour
     (a per-project refusal that later clears is re-established on the next resync, with no
     re-open) at the watch set, which is now the only thing that establishes watches. Confirm that
     test exists and is green **before** deleting the file; if it is missing, stop and say so
     rather than removing a regression test with no successor. `is_lock` stays. `covers` stays.
  5. `git/watch.rs` — take `broadcast::Receiver<PathBuf>`; drop `watcher`, `status`, and the
     `WatchStatus` reporting from `resync`, which now only reconciles `Watches`' routing map. On
     `RecvError::Lagged` from the path stream: arm every watched project's debouncer (a status
     re-read is idempotent, and the alternative is a rail that silently stops following).
  6. `filewatch/watcher.rs` — delete `FileWatcher::watch`, `FileWatcher::watch_dir`, `WatchHandle`,
     `NoopWatchHandle` and their re-exports; `mod.rs` and `lib.rs` follow.
  7. `crates/sys/src/filewatch.rs` — delete `start_watch`, `NotifyWatchHandle` and the two legacy
     trait methods; the session is all that remains. Update the module doc: the adapter now holds one
     inotify instance for the whole app, registers per directory non-recursively, and leaves adding
     newly created subdirectories to the core.
  8. `testing/filewatch.rs` — delete the legacy `watch`/`watch_dir` recording paths and
     `FakeWatchHandle`, keeping the session fake.
  9. `composition.rs` — wire `watch_scanner` into `Facade::new`'s construction of the watch set.
  10. `crates/app/src/lib.rs` — `.watch_scanner(Arc::new(IgnoreWatchScanner::new()))` in
      `build_facade`, and spawn `watch_set_loop()` **before** the three reactor loops (all after
      `restore_projects()`, for the reason the existing comment gives). Update the comments to say
      the three reactors now share one watch set.
  11. `crates/sys/tests/{filewatch,config_watch}.rs` — port to the session API; the config-watch
      integration test now drives the real `NotifyFileWatcher` + `IgnoreWatchScanner` through the
      watch set.
- **Tests:** the existing suites are the contract. Every test in `reactor_tests.rs`,
  `git/watch_tests.rs` and `config_watch_tests.rs` that asserts a *behaviour* (a matching change
  restarts, a non-matching one does not, a burst coalesces, `is_lock` is suppressed, a removed
  project stops being watched, a refusal is announced then withdrawn) **must still pass** after
  being re-pointed at the new construction. Tests that assert *which roots the reactor asked the
  watcher for* are now assertions about the watch set and move to T4's coverage — delete them here
  rather than keep a duplicate.
  Add one new integration test in `crates/sys/tests/config_watch.rs`: a real project directory whose
  `solo.yml` edit reloads through the shared session, proving the config reactor works with no
  watcher of its own.
- **Verify:** `just test` and `just lint` fully green — `cargo fmt --check`, `clippy -D warnings`,
  `tsc --noEmit`, ESLint, `scripts/check-core-deps.sh`, `scripts/check-core-cycles.sh`,
  `scripts/check-file-size.sh`. `rg -n "WatchHandle|fn watch_dir\(&self" crates/` returns nothing
  outside `WatchSession`.

---

## Wave 5 — proof on the real machine (runs alone: it measures what the earlier waves changed)

### T7 — measure, soak, record

- **Owns (exclusive write):** `PROGRESS.md`
- **May read:** everything.
- **Depends on:** T6.
- **Do:**
  1. Build and run the app (`just dev-alongside` if an installed Soloist is running, otherwise
     `just dev`) with this repository open.
  2. Count watches:
     `PID=$(pgrep -f 'soloist$' | head -1); for f in /proc/$PID/fdinfo/*; do grep -c '^inotify' "$f"; done | paste -sd+ | bc`
     — record the number. Expect ≲ 1,000 (was 58,179).
  3. Count inotify instances: `ls -l /proc/$PID/fd | grep -c 'anon_inode:inotify'` — expect 1.
  4. Open two more projects, including one large one. Record each project's watch count and confirm
     no project shows a notice, or that a degraded one shows the degraded notice and its git rail
     still updates after a `git commit` run in a terminal.
  5. Exercise the race by hand: `mkdir -p src/fresh && echo x > src/fresh/f.rs` in a project whose
     `solo.yml` declares `restart_when_changed: ["src/**/*.rs"]`; confirm the command restarts.
  6. Exercise release: close and reopen all three projects five times; re-count watches and confirm
     the number returns to its baseline (no leak).
  7. Run `just soak` and record RSS/FD/task drift.
  8. Run `just bundle-size` and record the delta against the number T3 captured before adding
     `ignore`.
  9. Update `PROGRESS.md` (CLAUDE.md §10): what landed, the before/after watch counts as evidence,
     what is `Verified` vs `Done — pending verify`, and a specific "next session should start with…"
     pointer. Record any measurement that missed its target as a gap with a plan — never a guessed
     number.
- **Verify:** every number above is written down; `just lint && just test` green one final time on
  the integrated branch.

---

## Collision check

Only **within-wave** disjointness is the invariant; a file legitimately appears in more than one
wave (a contract wave settles a type, a later wave changes the behaviour behind it).

| Wave | File | Owned by |
|---|---|---|
| 1 | `crates/core/src/git/watched.rs` | T1 |
| 1 | `crates/core/src/git/watched_tests.rs` | T1 |
| 1 | `crates/core/src/testing/filewatch.rs` | T1 |
| 2 | `crates/core/src/filewatch/watcher.rs` | T2 |
| 2 | `crates/core/src/filewatch/scan.rs` | T2 |
| 2 | `crates/core/src/filewatch/mod.rs` | T2 |
| 2 | `crates/core/src/filewatch/status.rs` | T2 |
| 2 | `crates/core/src/filewatch/status_tests.rs` | T2 |
| 2 | `crates/core/src/filewatch/reactor.rs` | T2 |
| 2 | `crates/core/src/filewatch/reactor_tests.rs` | T2 |
| 2 | `crates/core/src/watch.rs` | T2 |
| 2 | `crates/core/src/vcs.rs` | T2 |
| 2 | `crates/core/src/events.rs` | T2 |
| 2 | `crates/core/src/events_tests.rs` | T2 |
| 2 | `crates/core/src/lib.rs` | T2 |
| 2 | `crates/core/src/composition.rs` | T2 |
| 2 | `crates/core/src/testing/filewatch.rs` | T2 |
| 2 | `crates/core/src/testing/watchscan.rs` | T2 |
| 2 | `crates/core/src/testing/mod.rs` | T2 |
| 2 | `crates/core/src/git/watched.rs` | T2 |
| 2 | `crates/core/src/git/watched_tests.rs` | T2 |
| 2 | `crates/core/src/git/watch.rs` | T2 |
| 2 | `crates/core/src/git/watch_tests.rs` | T2 |
| 2 | `crates/sys/src/filewatch.rs` | T2 |
| 2 | `crates/sys/tests/filewatch.rs` | T2 |
| 3 | `crates/sys/src/watchscan.rs` | T3 |
| 3 | `crates/sys/src/lib.rs` | T3 |
| 3 | `crates/sys/Cargo.toml` | T3 |
| 3 | `crates/sys/tests/watchscan.rs` | T3 |
| 3 | `crates/core/src/watchset/mod.rs` | T4 |
| 3 | `crates/core/src/watchset/plan.rs` | T4 |
| 3 | `crates/core/src/watchset/plan_tests.rs` | T4 |
| 3 | `crates/core/src/watchset/budget.rs` | T4 |
| 3 | `crates/core/src/watchset/budget_tests.rs` | T4 |
| 3 | `crates/core/src/watchset/set.rs` | T4 |
| 3 | `crates/core/src/watchset/set_tests.rs` | T4 |
| 3 | `crates/core/src/filewatch/policy.rs` | T4 |
| 3 | `crates/core/src/filewatch/policy_tests.rs` | T4 |
| 3 | `crates/core/src/lib.rs` | T4 |
| 3 | `crates/app/ui/src/domain.ts` | T5 |
| 3 | `crates/app/ui/src/store/projection.ts` | T5 |
| 3 | `crates/app/ui/src/store/projection.test.ts` | T5 |
| 3 | `crates/app/ui/src/store/watchContext.ts` | T5 |
| 3 | `crates/app/ui/src/store/useWatchLimits.ts` (was `useWatchRefusals.ts`) | T5 |
| 3 | `crates/app/ui/src/store/useWatchLimits.test.ts` (was `useWatchRefusals.test.ts`) | T5 |
| 3 | `crates/app/ui/src/components/sidebar/WatchLimitNotice.tsx` (was `WatchRefusedNotice.tsx`) | T5 |
| 3 | `crates/app/ui/src/components/sidebar/WatchLimitNotice.test.tsx` (was `WatchRefusedNotice.test.tsx`) | T5 |
| 3 | `crates/app/ui/src/components/sidebar/ProjectGroup.tsx` | T5 |
| 3 | `crates/app/ui/src/components/sidebar/ProjectGroup.test.tsx` | T5 |
| 4 | `crates/core/src/filewatch/{reactor,reactor_tests,watcher,mod}.rs` | T6 |
| 4 | `crates/core/src/git/{watch,watch_tests,watched,watched_tests}.rs` | T6 |
| 4 | `crates/core/src/projects/{config_watch,config_watch_tests}.rs` | T6 |
| 4 | `crates/core/src/{facade.rs,facade/loops.rs,composition.rs,testing/filewatch.rs}` | T6 |
| 4 | `crates/sys/src/filewatch.rs`, `crates/sys/tests/{filewatch,config_watch}.rs` | T6 |
| 4 | `crates/app/src/lib.rs` | T6 |
| 5 | `PROGRESS.md` | T7 |

Within every wave, no file appears twice. Wave 3's three tasks touch three disjoint trees
(`crates/sys/**` + its manifest, `crates/core/src/{watchset,filewatch/policy*}` + `lib.rs`,
`crates/app/ui/**`).

---

## Decisions taken (all four open questions settled by the lead)

1. **`crates/core/src/watchset/` stays a sibling of `filewatch` — agreed.** The deciding factor is
   that `scripts/check-core-cycles.sh` has no allow-list, so planning on a ring that only becomes
   legal after T6 lands would turn one late failure into a rollback of four tasks. Record the module
   in `ARCHITECTURE.md`'s table as cross-cutting (owned by no context, like `events`/`debounce`)
   when T7 updates the docs.

2. **Do the frontend rename.** `WatchRefusalChanged` carrying a `Degraded` variant is a name that
   lies, and CLAUDE.md §15 requires names to say what a thing permanently is. Eight files is the
   fair price of "nothing left behind". The JSON stays pinned in both T2 and T5 so T5 never opens a
   Rust file.

3. **A manual "refresh status now" action is out of scope — recorded as a follow-up.** There is no
   user-triggered fresh read today (`Facade::git_status` serves `Git::status`, which returns the
   cache when present, `crates/core/src/git/status.rs:317`), so a degraded project has no manual
   escape hatch. Adding one is a new feature nobody asked for; CLAUDE.md forbids gold-plating.
   **Follow-up, do not lose:** a `Facade::git_refresh` plus a rail action, if the degraded path
   turns out to be reachable in practice. **If T5 finds the degraded notice is actively misleading
   without one, raise it with the lead rather than adding it unilaterally.**

4. **The budget constants are accepted as judgement calls, with one requirement.**
   `BUDGET_FRACTION = 2` and `ASSUMED_CAPACITY = 8_192` must be **named `const`s in one place**
   (`crates/core/src/watchset/budget.rs`), each with its reasoning in a doc comment — half, because
   Soloist shares the machine with editors, language servers and other watchers; 8,192 because that
   is the kernel default and the safe assumption when `/proc` will not answer. They must be
   promotable to config later without touching a call site. **Do not pre-build the config
   plumbing** — no settings field, no builder method, no plumbing of any kind until something asks
   for it.

## Notes for whoever picks this up

- **T1's registration code is deleted by T6, by design.** That is true of any option we could have
  chosen for R2, because the whole point of R1/R3/R4 is that a single owner registers watches. What
  survives T6 is `Watches`' *routing* half (`watched`, `projects_of`, `root_of`, `retain`), the
  *design* (per-path accounting, one record per project — which is what T4 implements), and the
  *guarantee*, which moves to T4's
  `a_project_refused_once_is_established_when_the_refusal_clears`. **T2 does not touch T1's data
  shape**: it changes exactly two things in `git/watched.rs` — the `WatchOutcome` construction
  (`refusal: Option<WatchError>` → `limit: Option<WatchLimit>`, wrapping in `WatchLimit::Refused`)
  and two `const` declarations becoming `use crate::vcs::{STATE_DIR, REFS_DIR}`. `Held` keeps
  `Refused(WatchError)`, because it records what the OS said about one path; `Degraded` is a budget
  decision made by the watch set and can never be what a single path holds.
- **`Held` and T4's `Registration` are not duplication.** `Held` is per-path-per-project and records
  the OS's answer; `Registration { owners: HashSet<ProjectId>, tree: bool }` is per-path-globally
  and records who wants it. Different data, different jobs.
