//! Behavioural tests for [`ProjectWatchSet`], kept out of the implementation file. They drive a
//! real [`Supervisor`] and [`Projects`] registry over fakes plus a [`MockClock`], the same shape
//! as the reactor tests this module replaces.
//!
//! Waits fall into two kinds. [`expect_change`] genuinely awaits the fan-out channel (bounded by
//! a real timeout) — the deterministic, event-driven wait for a change to be delivered.
//! [`wait_until`] polls fake-backed state (`registered()`, `unwatched()`, `sessions_opened()`) on
//! a short real interval, bounded the same way: nothing here exposes a signal of its own for
//! "one re-sync finished", so this is the closest bounded, non-yield-counting equivalent
//! available without reaching into `crate::testing::wait`, which is private to that module.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::composition::CorePorts;
use crate::config::ProcessSpec;
use crate::configchange::ConfigSync;
use crate::events::{DomainEvent, EventBus};
use crate::filewatch::{FileChangeKind, WatchReactor};
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{ProjectRepo, TrustRepo};
use crate::process::ProcStatus;
use crate::supervisor::{Registration, Supervisor};
use crate::testing::{
    next_matching, wait_all, FakeFileWatcher, FakeProjectRepo, FakeSpawner, FakeTrustRepo,
    FakeWatchScanner, MockClock,
};
use crate::watch::{WatchError, WatchLimit, WatchPurpose};
use crate::watchset::Scan;

use super::*;

const ROOT: &str = "/project";

/// A watch capacity small enough for a test to fill by hand. Soloist holds half of it, so one
/// open project's share is 32 watches and a second open project halves that again.
const SMALL_CAPACITY: usize = 64;

/// How far past a deadline a test advances the clock, so the deadline fires rather than the
/// advance landing exactly on it.
const PAST_THE_DEADLINE: Duration = Duration::from_millis(100);

/// How far [`expect_file_restart`] advances the clock per attempt. The restart reactor's quiet
/// window belongs to its own module and is not visible here, so this is a step generous enough to
/// clear any plausible one rather than a value pinned to it.
const RESTART_ADVANCE: Duration = Duration::from_secs(1);

/// How many advances [`expect_file_restart`] makes before failing. The reactor is woken from
/// another task, so an advance can land before it has armed the window the change opened, and the
/// next one then clears it.
const RESTART_ATTEMPTS: usize = 20;

/// How long [`expect_file_restart`] lets real time pass after each advance, so the reactor — woken
/// on another task — gets a real opportunity to act on it.
const SETTLE: Duration = Duration::from_millis(20);

fn root() -> PathBuf {
    PathBuf::from(ROOT)
}

fn under(relative: &str) -> PathBuf {
    root().join(relative)
}

fn state_dir() -> PathBuf {
    under(".git")
}

fn refs_dir() -> PathBuf {
    under(".git/refs")
}

struct Setup {
    clock: MockClock,
    bus: EventBus,
    rx: broadcast::Receiver<DomainEvent>,
    watcher: Arc<FakeFileWatcher>,
    scanner: Arc<FakeWatchScanner>,
    repo: Arc<FakeProjectRepo>,
    projects: Arc<Projects>,
    sup: Arc<Supervisor>,
    status: Arc<WatchStatus>,
    trust: Arc<FakeTrustRepo>,
}

fn setup() -> Setup {
    let bus = EventBus::new(256);
    let rx = bus.subscribe();
    let clock = MockClock::new();
    let repo = Arc::new(FakeProjectRepo::new());
    let trust = Arc::new(FakeTrustRepo::new());
    let projects = Arc::new(Projects::new(repo.clone()));
    let ports = CorePorts::builder(
        Arc::new(FakeSpawner::exits_on_kill()),
        Arc::new(clock.clone()),
        trust.clone(),
        repo.clone(),
    )
    .build();
    let sup = Arc::new(Supervisor::new(ports.supervisor_ports(), bus.clone()));
    Setup {
        status: Arc::new(WatchStatus::new(bus.clone())),
        clock,
        bus,
        rx,
        watcher: Arc::new(FakeFileWatcher::new()),
        scanner: Arc::new(FakeWatchScanner::new()),
        repo,
        projects,
        sup,
        trust,
    }
}

fn watch_set(s: &Setup, scanner: Arc<dyn WatchScanner>) -> ProjectWatchSet {
    ProjectWatchSet::new(
        Arc::new(s.clock.clone()),
        s.watcher.clone(),
        scanner,
        &s.bus,
        s.projects.clone(),
        Arc::downgrade(&s.sup),
        s.status.clone(),
    )
}

/// Builds the set over the setup's own scanner and spawns it.
fn spawn_set(s: &Setup) -> ProjectWatchSet {
    spawn_set_with(s, s.scanner.clone())
}

/// Builds the set over `scanner` and spawns it, returning the handle a test subscribes from — a
/// fresh `watch_set` call would open an unrelated fan-out, so the spawned and subscribed instances
/// must be the same value (clones of it, sharing the same broadcast channel).
fn spawn_set_with(s: &Setup, scanner: Arc<dyn WatchScanner>) -> ProjectWatchSet {
    let ws = watch_set(s, scanner);
    tokio::spawn(ws.clone().run());
    ws
}

/// Spawns the restart reactor over the set's fan-out, so a test can assert on the restart a
/// registered directory's change actually produces rather than on the registration alone.
fn spawn_reactor(s: &Setup, changes: broadcast::Receiver<PathBuf>) {
    tokio::spawn(
        WatchReactor::new(
            Arc::new(s.clock.clone()),
            changes,
            &s.bus,
            Arc::downgrade(&s.sup),
        )
        .run(),
    );
}

/// Answers a repository-ignore-honouring scan and an ignore-disabled scan of the *same* root
/// differently — what a real repository does for a gitignored directory, and what the shared
/// [`FakeWatchScanner`] cannot express on its own because it answers per root.
struct IgnoreAwareScanner {
    honouring: Arc<FakeWatchScanner>,
    disabled: Arc<FakeWatchScanner>,
}

impl WatchScanner for IgnoreAwareScanner {
    fn scan(&self, request: ScanRequest) -> Scan {
        if request.honour_repository_ignores {
            self.honouring.scan(request)
        } else {
            self.disabled.scan(request)
        }
    }
}

fn watched_spec(globs: &[&str]) -> ProcessSpec {
    ProcessSpec {
        command: "sleep 60".into(),
        working_dir: None,
        auto_start: false,
        auto_restart: false,
        restart_when_changed: globs.iter().map(|g| g.to_string()).collect(),
        env: BTreeMap::new(),
    }
}

fn register_command(s: &Setup, project: ProjectId, name: &str, globs: &[&str]) -> ProcessId {
    s.sup.register(Registration::command(
        project,
        &root(),
        name,
        &watched_spec(globs),
    ))
}

/// Registers a watch-eligible command, trusts it, starts it and awaits its `Running` transition
/// — a file-watch restart fires only for a live, trusted command.
async fn running_command(
    s: &mut Setup,
    project: ProjectId,
    name: &str,
    globs: &[&str],
) -> ProcessId {
    let id = register_command(s, project, name, globs);
    let spec = watched_spec(globs);
    s.trust
        .set_trusted(project, &spec.variant_hash(), &spec.command)
        .expect("trust");
    s.sup.start(id).expect("start");
    wait_all(&mut s.rx, &[id], ProcStatus::Running).await;
    id
}

/// Fires the restart reactor's debounce and asserts the restart it produces is `expected`.
/// Bounded in both directions: a generous advance per attempt and a fixed number of them, so a
/// change that never restarts anything fails loudly instead of hanging.
async fn expect_file_restart(s: &mut Setup, expected: ProcessId) {
    for _ in 0..RESTART_ATTEMPTS {
        s.clock.advance(RESTART_ADVANCE);
        tokio::time::sleep(SETTLE).await;
        while let Ok(event) = s.rx.try_recv() {
            let DomainEvent::FileRestart { id } = event else {
                continue;
            };
            assert_eq!(id, expected, "a different command restarted");
            return;
        }
    }
    panic!("the watched change never restarted the command");
}

fn seed_scan(scanner: &FakeWatchScanner, scan_root: PathBuf, entries: &[(PathBuf, bool)]) {
    let owned: Vec<(String, bool)> = entries
        .iter()
        .map(|(path, directory)| (path.to_string_lossy().into_owned(), *directory))
        .collect();
    scanner.reporting(
        scan_root,
        owned
            .iter()
            .map(|(path, directory)| (path.as_str(), *directory))
            .collect(),
    );
}

/// `count` distinct directories under `scan_root`, for a scan sized against a small budget.
fn directories_under(scan_root: &Path, count: usize) -> Vec<(PathBuf, bool)> {
    (0..count)
        .map(|i| (scan_root.join(format!("d{i}")), true))
        .collect()
}

/// `count` distinct directories under the project root, for a scan too large to fit a small
/// budget.
fn many_directories(count: usize) -> Vec<(PathBuf, bool)> {
    directories_under(&root(), count)
}

/// Polls a synchronous condition on a short real interval until it holds or a generous ceiling
/// elapses. Bounded, and not the yield-count anti-pattern: real time genuinely passes on each
/// iteration, so cross-thread work (a scan dispatched to the blocking pool) gets real
/// opportunities to finish, rather than being starved by a tight in-process spin.
async fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !condition() {
        if tokio::time::Instant::now() >= deadline {
            panic!("timed out waiting for {what}");
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// Awaits `path` arriving on `rx`, draining and ignoring anything else — the deterministic,
/// bounded wait for a change actually reaching a consumer.
async fn expect_change(rx: &mut broadcast::Receiver<PathBuf>, path: &Path) {
    let outcome = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match rx.recv().await {
                Ok(received) if received == path => return,
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => {
                    panic!("the fan-out closed while awaiting {}", path.display())
                }
            }
        }
    })
    .await;
    outcome.unwrap_or_else(|_| panic!("timed out waiting for a change at {}", path.display()));
}

#[tokio::test]
async fn one_session_backs_every_project() {
    let s = setup();
    s.repo.upsert(&root(), None, None).expect("seed p1");
    let other = PathBuf::from("/other");
    s.repo.upsert(&other, None, None).expect("seed p2");
    spawn_set(&s);

    wait_until("both project roots registered", || {
        let live = s.watcher.registered();
        live.contains(&root()) && live.contains(&other)
    })
    .await;

    assert_eq!(s.watcher.sessions_opened(), 1);
}

#[tokio::test]
async fn only_the_scanners_directories_are_registered() {
    let s = setup();
    s.repo.upsert(&root(), None, None).expect("seed");
    seed_scan(
        &s.scanner,
        root(),
        &[(under("a"), true), (under("b"), true), (under("c"), true)],
    );
    spawn_set(&s);

    let mut expected = vec![
        root(),
        state_dir(),
        refs_dir(),
        under("a"),
        under("b"),
        under("c"),
    ];
    expected.sort();
    wait_until("every scanned directory registered", || {
        let mut live = s.watcher.registered();
        live.sort();
        live == expected
    })
    .await;
}

#[tokio::test]
async fn a_created_directory_is_registered_and_its_contents_replayed() {
    let s = setup();
    s.repo.upsert(&root(), None, None).expect("seed");
    seed_scan(&s.scanner, root(), &[(under("src"), true)]);
    let ws = spawn_set(&s);
    let mut rx = ws.subscribe();
    wait_until("src registered", || {
        s.watcher.registered().contains(&under("src"))
    })
    .await;

    let new_dir = under("src/new");
    let file_in_new = under("src/new/file.txt");
    seed_scan(
        &s.scanner,
        new_dir.clone(),
        &[(new_dir.clone(), true), (file_in_new.clone(), false)],
    );
    s.watcher
        .change_of(new_dir.clone(), FileChangeKind::Appeared);

    expect_change(&mut rx, &file_in_new).await;
    wait_until("the appeared directory is registered", || {
        s.watcher.registered().contains(&new_dir)
    })
    .await;
}

#[tokio::test]
async fn a_vanished_directory_is_unwatched() {
    let s = setup();
    s.repo.upsert(&root(), None, None).expect("seed");
    seed_scan(&s.scanner, root(), &[(under("gone"), true)]);
    spawn_set(&s);
    wait_until("gone registered", || {
        s.watcher.registered().contains(&under("gone"))
    })
    .await;

    s.watcher.change_of(under("gone"), FileChangeKind::Vanished);

    wait_until("gone unwatched", || {
        s.watcher.unwatched().contains(&under("gone"))
    })
    .await;
    assert!(!s.watcher.registered().contains(&under("gone")));
}

#[tokio::test]
async fn a_directory_two_projects_share_survives_one_closing() {
    let s = setup();
    let inner_root = under("inner");
    let inner_only = under("inner/only_inner");
    s.repo.upsert(&root(), None, None).expect("seed outer");
    seed_scan(&s.scanner, root(), &[(inner_root.clone(), true)]);
    let inner = s
        .repo
        .upsert(&inner_root, None, None)
        .expect("seed inner")
        .id;
    seed_scan(
        &s.scanner,
        inner_root.clone(),
        &[(inner_only.clone(), true)],
    );
    let ws = spawn_set(&s);
    let mut rx = ws.subscribe();
    wait_until("the inner-only path is registered", || {
        s.watcher.registered().contains(&inner_only)
    })
    .await;
    assert!(s.watcher.registered().contains(&inner_root));

    s.repo.remove(inner).expect("remove inner");
    s.bus.publish(DomainEvent::ProjectRemoved { id: inner });

    wait_until("the inner-only path is released", || {
        s.watcher.unwatched().contains(&inner_only)
    })
    .await;
    assert!(!s.watcher.registered().contains(&inner_only));

    // The discriminating check: the shared path is not just absent from `unwatched()`, it is
    // still actually delivering.
    assert!(s.watcher.registered().contains(&inner_root));
    let still_there = under("inner/still-there");
    s.watcher
        .change_of(still_there.clone(), FileChangeKind::Modified);
    expect_change(&mut rx, &still_there).await;
}

#[tokio::test]
async fn an_oversized_project_is_degraded_not_refused() {
    let mut s = setup();
    s.watcher = Arc::new(FakeFileWatcher::new().with_capacity(SMALL_CAPACITY));
    let project = s.repo.upsert(&root(), None, None).expect("seed").id;
    seed_scan(&s.scanner, root(), &many_directories(40));
    let ws = spawn_set(&s);
    let mut rx = ws.subscribe();

    let announced = next_matching(
        &mut s.rx,
        |event| matches!(event, DomainEvent::WatchLimitChanged { project: p, .. } if *p == project),
    )
    .await;
    match announced {
        DomainEvent::WatchLimitChanged { limits, .. } => {
            assert_eq!(
                limits.get(&WatchPurpose::GitStatus),
                Some(&WatchLimit::Degraded)
            );
        }
        other => unreachable!("awaited a WatchLimitChanged, got {other:?}"),
    }

    assert!(s.watcher.registered().contains(&root()));
    assert!(s.watcher.registered().contains(&state_dir()));
    let index = under(".git/index");
    s.watcher.change_of(index.clone(), FileChangeKind::Modified);
    expect_change(&mut rx, &index).await;
}

#[tokio::test]
async fn a_glob_prefix_directory_survives_degradation() {
    let mut s = setup();
    s.watcher = Arc::new(FakeFileWatcher::new().with_capacity(SMALL_CAPACITY));
    let project = s.repo.upsert(&root(), None, None).expect("seed").id;
    register_command(&s, project, "Build", &["dist/**/*.json"]);
    seed_scan(&s.scanner, under("dist"), &[(under("dist"), true)]);
    seed_scan(&s.scanner, root(), &many_directories(40));
    spawn_set(&s);

    wait_until("dist registered", || {
        s.watcher.registered().contains(&under("dist"))
    })
    .await;

    while let Ok(event) = s.rx.try_recv() {
        if let DomainEvent::WatchLimitChanged { project: p, limits } = event {
            if p == project {
                assert!(
                    !matches!(
                        limits.get(&WatchPurpose::Restarts),
                        Some(WatchLimit::Degraded)
                    ),
                    "restarts must not degrade when its own prefix directory fit: {limits:?}",
                );
            }
        }
    }
}

#[tokio::test]
async fn a_halved_share_releases_the_tree_it_no_longer_covers() {
    let mut s = setup();
    s.watcher = Arc::new(FakeFileWatcher::new().with_capacity(SMALL_CAPACITY));
    s.repo.upsert(&root(), None, None).expect("seed");
    // The root, the state directory, the refs tree and these fit a lone project's share of 32.
    const TREE_DIRECTORIES: usize = 28;
    seed_scan(&s.scanner, root(), &many_directories(TREE_DIRECTORIES));
    spawn_set(&s);
    let last = under(&format!("d{}", TREE_DIRECTORIES - 1));
    wait_until("the whole tree is registered", || {
        s.watcher.registered().contains(&last)
    })
    .await;

    let other = PathBuf::from("/other");
    let opened = s.repo.upsert(&other, None, None).expect("seed second").id;
    s.bus.publish(DomainEvent::ProjectOpened { id: opened });

    wait_until(
        "the tree the halved share no longer covers is released",
        || !s.watcher.registered().contains(&last),
    )
    .await;
    assert!(
        s.watcher.registered().contains(&root()),
        "what the shrunken plan still wants stays watched",
    );
}

#[tokio::test]
async fn a_project_holding_its_whole_share_does_not_watch_a_newly_appeared_directory() {
    let mut s = setup();
    s.watcher = Arc::new(FakeFileWatcher::new().with_capacity(SMALL_CAPACITY));
    s.repo.upsert(&root(), None, None).expect("seed");
    // The root, the state directory, the refs tree and these are exactly a lone project's share
    // of 32 — a directory appearing afterwards has nothing left to be watched with.
    const TREE_DIRECTORIES: usize = 29;
    seed_scan(&s.scanner, root(), &many_directories(TREE_DIRECTORIES));
    let ws = spawn_set(&s);
    let mut rx = ws.subscribe();
    let last = under(&format!("d{}", TREE_DIRECTORIES - 1));
    wait_until("the whole tree is registered", || {
        s.watcher.registered().contains(&last)
    })
    .await;
    let held_at_the_share = s.watcher.registered().len();

    let appeared = under("appeared");
    seed_scan(&s.scanner, appeared.clone(), &[(appeared.clone(), true)]);
    s.watcher
        .change_of(appeared.clone(), FileChangeKind::Appeared);

    // Changes are handled one at a time and each is republished only once handled, so a later
    // change reaching the fan-out proves the appearance before it was absorbed in full.
    let afterwards = under("afterwards");
    s.watcher
        .change_of(afterwards.clone(), FileChangeKind::Modified);
    expect_change(&mut rx, &afterwards).await;

    assert!(
        !s.watcher.registered().contains(&appeared),
        "a project already holding its whole share must not spend past it",
    );
    assert_eq!(s.watcher.registered().len(), held_at_the_share);
}

#[tokio::test]
async fn closing_a_sibling_re_establishes_the_tree_the_shared_budget_had_dropped() {
    let mut s = setup();
    s.watcher = Arc::new(FakeFileWatcher::new().with_capacity(SMALL_CAPACITY));
    let project = s.repo.upsert(&root(), None, None).expect("seed").id;
    let sibling_root = PathBuf::from("/other");
    let sibling = s
        .repo
        .upsert(&sibling_root, None, None)
        .expect("seed sibling")
        .id;
    // Two open projects share 16 watches each: this tree plus the three essentials does not fit,
    // while the sibling's smaller one does.
    const TREE_DIRECTORIES: usize = 27;
    const SIBLING_DIRECTORIES: usize = 10;
    seed_scan(&s.scanner, root(), &many_directories(TREE_DIRECTORIES));
    seed_scan(
        &s.scanner,
        sibling_root.clone(),
        &directories_under(&sibling_root, SIBLING_DIRECTORIES),
    );
    spawn_set(&s);
    let sibling_last = sibling_root.join(format!("d{}", SIBLING_DIRECTORIES - 1));
    wait_until("the sibling's tree is registered", || {
        s.watcher.registered().contains(&sibling_last)
    })
    .await;
    let degraded = next_matching(&mut s.rx, |event| {
        matches!(event, DomainEvent::WatchLimitChanged { project: p, limits }
            if *p == project && limits.get(&WatchPurpose::GitStatus) == Some(&WatchLimit::Degraded))
    })
    .await;
    let _ = degraded;

    s.repo.remove(sibling).expect("remove the sibling");
    s.bus.publish(DomainEvent::ProjectRemoved { id: sibling });

    let withdrawn = next_matching(&mut s.rx, |event| {
        matches!(event, DomainEvent::WatchLimitChanged { project: p, limits }
            if *p == project && limits.is_empty())
    })
    .await;
    let _ = withdrawn;
    wait_until(
        "the whole tree the widened share now covers is registered",
        || {
            let live = s.watcher.registered();
            (0..TREE_DIRECTORIES).all(|i| live.contains(&under(&format!("d{i}"))))
        },
    )
    .await;
}

#[tokio::test]
async fn a_gitignored_glob_prefix_is_scanned_with_repository_ignores_disabled() {
    let s = setup();
    let project = s.repo.upsert(&root(), None, None).expect("seed").id;
    register_command(&s, project, "Build", &["dist/config.json"]);
    spawn_set(&s);

    wait_until("dist scan requested", || {
        s.scanner
            .requests()
            .iter()
            .any(|request| request.root == under("dist"))
    })
    .await;

    let request = s
        .scanner
        .requests()
        .into_iter()
        .find(|request| request.root == under("dist"))
        .expect("a scan of dist was requested");
    assert!(!request.honour_repository_ignores);
}

#[tokio::test]
async fn a_gitignored_directory_a_glob_names_still_restarts_its_command() {
    let mut s = setup();
    let project = s.repo.upsert(&root(), None, None).expect("seed").id;
    let build = running_command(&mut s, project, "Build", &["generated/config.json"]).await;
    // The repository ignores `generated`, so the whole-tree scan does not report it...
    seed_scan(&s.scanner, root(), &[(under("src"), true)]);
    // ...but the glob names it, and the scan serving that glob runs with those ignores disabled.
    let disabled = Arc::new(FakeWatchScanner::new());
    seed_scan(&disabled, under("generated"), &[(under("generated"), true)]);
    let ws = spawn_set_with(
        &s,
        Arc::new(IgnoreAwareScanner {
            honouring: s.scanner.clone(),
            disabled,
        }),
    );
    spawn_reactor(&s, ws.subscribe());
    wait_until(
        "the gitignored directory the glob names is registered",
        || s.watcher.registered().contains(&under("generated")),
    )
    .await;

    s.watcher
        .change_of(under("generated/config.json"), FileChangeKind::Modified);

    expect_file_restart(&mut s, build).await;
}

#[tokio::test]
async fn a_gitignored_directory_a_prefixless_glob_matches_still_restarts_its_command() {
    let mut s = setup();
    let project = s.repo.upsert(&root(), None, None).expect("seed").id;
    let build = running_command(&mut s, project, "Build", &["**/*.json"]).await;
    // As above, `generated` is gitignored and absent from the whole-tree scan. This glob names no
    // directory at all, so what it asks to have scanned with the ignores disabled is the root.
    seed_scan(&s.scanner, root(), &[(under("src"), true)]);
    let disabled = Arc::new(FakeWatchScanner::new());
    seed_scan(&disabled, root(), &[(under("generated"), true)]);
    let ws = spawn_set_with(
        &s,
        Arc::new(IgnoreAwareScanner {
            honouring: s.scanner.clone(),
            disabled,
        }),
    );
    spawn_reactor(&s, ws.subscribe());
    wait_until(
        "a gitignored directory the prefix-less glob can match is registered",
        || s.watcher.registered().contains(&under("generated")),
    )
    .await;

    s.watcher
        .change_of(under("generated/config.json"), FileChangeKind::Modified);

    expect_file_restart(&mut s, build).await;
}

#[tokio::test]
async fn a_named_glob_prefix_is_watched_before_a_prefixless_globs_whole_root() {
    let mut s = setup();
    s.watcher = Arc::new(FakeFileWatcher::new().with_capacity(SMALL_CAPACITY));
    let project = s.repo.upsert(&root(), None, None).expect("seed").id;
    register_command(&s, project, "Build", &["assets/config.json", "**/*.json"]);
    let disabled = Arc::new(FakeWatchScanner::new());
    // One cheap directory for the glob that names one...
    seed_scan(&disabled, under("assets"), &[(under("assets"), true)]);
    // ...and, for the glob that names none, a whole root that fits the share of 32 on its own
    // (with the root, the state directory and the refs tree) but leaves nothing over. Only the
    // order decides which of the two is watched.
    seed_scan(&disabled, root(), &many_directories(29));
    spawn_set_with(
        &s,
        Arc::new(IgnoreAwareScanner {
            honouring: s.scanner.clone(),
            disabled,
        }),
    );

    wait_until("the directory the glob names is registered", || {
        s.watcher.registered().contains(&under("assets"))
    })
    .await;
    assert!(
        !s.watcher.registered().contains(&under("d0")),
        "the whole-root scan did not fit and must not have been registered",
    );
}

#[tokio::test]
async fn a_project_refused_once_is_established_when_the_refusal_clears() {
    let mut s = setup();
    let granted = PathBuf::from("/other");
    s.watcher.refuse(root());
    let refused_id = s.repo.upsert(&root(), None, None).expect("seed refused").id;
    let granted_id = s
        .repo
        .upsert(&granted, None, None)
        .expect("seed granted")
        .id;
    let ws = spawn_set(&s);
    let mut rx = ws.subscribe();

    let announced = next_matching(&mut s.rx, |event| {
        matches!(event, DomainEvent::WatchLimitChanged { project, .. } if *project == refused_id)
    })
    .await;
    match announced {
        DomainEvent::WatchLimitChanged { limits, .. } => assert_eq!(
            limits.get(&WatchPurpose::GitStatus),
            Some(&WatchLimit::Refused(WatchError::BudgetExhausted)),
        ),
        other => unreachable!("awaited a WatchLimitChanged, got {other:?}"),
    }

    wait_until("the granted project settles", || {
        s.watcher.registered().contains(&granted)
    })
    .await;
    while let Ok(event) = s.rx.try_recv() {
        if let DomainEvent::WatchLimitChanged { project, .. } = event {
            assert_ne!(
                project, granted_id,
                "the granted project must never be limited"
            );
        }
    }

    s.watcher.allow(root());
    s.bus.publish(DomainEvent::ConfigChanged {
        project: refused_id,
        diff: ConfigSync::default(),
        requires_trust: false,
        commands: Vec::new(),
    });

    let withdrawn = next_matching(&mut s.rx, |event| {
        matches!(event, DomainEvent::WatchLimitChanged { project, limits } if *project == refused_id && limits.is_empty())
    })
    .await;
    let _ = withdrawn;

    assert!(s.watcher.registered().contains(&root()));
    let touched = under("touched");
    s.watcher
        .change_of(touched.clone(), FileChangeKind::Modified);
    expect_change(&mut rx, &touched).await;
}

#[tokio::test]
async fn a_refused_session_is_retried_on_the_next_resync() {
    let mut s = setup();
    s.watcher.refuse_open();
    let project = s.repo.upsert(&root(), None, None).expect("seed").id;
    register_command(&s, project, "Build", &["src/**/*.rs"]);
    let ws = spawn_set(&s);
    let mut rx = ws.subscribe();

    let announced = next_matching(&mut s.rx, |event| {
        matches!(event, DomainEvent::WatchLimitChanged { project: p, limits }
            if *p == project
                && limits.get(&WatchPurpose::GitStatus)
                    == Some(&WatchLimit::Refused(WatchError::Unavailable))
                && limits.get(&WatchPurpose::Restarts)
                    == Some(&WatchLimit::Refused(WatchError::Unavailable)))
    })
    .await;
    let _ = announced;
    assert_eq!(
        s.watcher.sessions_opened(),
        0,
        "open() failed, so no session was ever handed out"
    );

    s.watcher.allow_open();
    s.bus.publish(DomainEvent::ConfigChanged {
        project,
        diff: ConfigSync::default(),
        requires_trust: false,
        commands: Vec::new(),
    });

    let withdrawn = next_matching(&mut s.rx, |event| {
        matches!(event, DomainEvent::WatchLimitChanged { project: p, limits } if *p == project && limits.is_empty())
    })
    .await;
    let _ = withdrawn;

    assert_eq!(
        s.watcher.sessions_opened(),
        1,
        "the next re-sync opens a fresh session rather than staying wedged"
    );
    assert!(s.watcher.registered().contains(&root()));
    let touched = under("touched");
    s.watcher
        .change_of(touched.clone(), FileChangeKind::Modified);
    expect_change(&mut rx, &touched).await;
}

#[tokio::test]
async fn closing_the_last_project_releases_every_watch() {
    let s = setup();
    let project = s.repo.upsert(&root(), None, None).expect("seed").id;
    seed_scan(&s.scanner, root(), &[(under("src"), true)]);
    spawn_set(&s);
    wait_until("src registered", || {
        s.watcher.registered().contains(&under("src"))
    })
    .await;
    let registered_count = s.watcher.registered().len();
    assert!(registered_count > 0);

    s.repo.remove(project).expect("remove");
    s.bus.publish(DomainEvent::ProjectRemoved { id: project });

    wait_until("every watch is released", || {
        s.watcher.registered().is_empty()
    })
    .await;
    assert_eq!(s.watcher.unwatched().len(), registered_count);
}

#[tokio::test]
async fn a_panicked_loop_rebuilds_its_watches_without_doubling_them() {
    let s = setup();
    s.repo.upsert(&root(), None, None).expect("seed");
    seed_scan(&s.scanner, root(), &[(under("src"), true)]);
    let ws = spawn_set(&s);
    let mut rx = ws.subscribe();

    wait_until("src registered", || {
        s.watcher.registered().contains(&under("src"))
    })
    .await;
    let held_before = s.watcher.registered().len();
    assert_eq!(s.watcher.sessions_opened(), 1);

    // A directory appearing under the already-watched `src` triggers a scan of it — arm that
    // scan to panic once, driving the loop through `supervise`'s panic-isolation boundary.
    let new_dir = under("src/new");
    s.scanner.panicking_once(new_dir.clone());
    s.watcher
        .change_of(new_dir.clone(), FileChangeKind::Appeared);

    // `supervise` catches the panic and sleeps `INITIAL_BACKOFF` (200ms, per
    // `crate::supervision`, private to that module) before restarting the loop.
    s.clock
        .deadline_armed_at(s.clock.now() + Duration::from_millis(200))
        .await;
    s.clock.advance(Duration::from_millis(250));

    // Assertion 2 is the one that discriminates: a fresh session was opened, not the wedged one
    // reused. If the session and registration map had (wrongly) survived in the `Arc`, assertion
    // 1 alone would still pass — nothing would be re-registered — while file watching stayed
    // dead for the rest of the process.
    wait_until("a fresh session is opened", || {
        s.watcher.sessions_opened() == 2
    })
    .await;
    wait_until("the watches are rebuilt without doubling", || {
        s.watcher.registered().len() == held_before
    })
    .await;

    // Assertion 3: the rebuilt session is delivering, end to end.
    let after_restart = under("src/after-restart.txt");
    s.watcher
        .change_of(after_restart.clone(), FileChangeKind::Modified);
    expect_change(&mut rx, &after_restart).await;

    assert_eq!(
        s.watcher.registered().len(),
        held_before,
        "no leak: the same set, not doubled"
    );
    assert_eq!(
        s.watcher.sessions_opened(),
        2,
        "a fresh session, not the wedged one reused"
    );
}

#[tokio::test]
async fn a_dropped_change_arms_a_full_rescan() {
    let s = setup();
    s.repo.upsert(&root(), None, None).expect("seed");
    let ws = spawn_set(&s);
    let mut rx = ws.subscribe();
    wait_until("the project settles", || {
        s.watcher.registered().contains(&root())
    })
    .await;
    let root_scans_before = s
        .scanner
        .requests()
        .iter()
        .filter(|request| request.root == root())
        .count();

    // A directory the next full scan will find but nothing has registered yet — standing in
    // for one whose own `Appeared` notification never arrived.
    let missed = under("missed");
    seed_scan(&s.scanner, root(), &[(missed.clone(), true)]);

    // Overflows the bounded change channel with no `.await` in the loop, so the loop task
    // cannot drain any of it concurrently: on a single-threaded runtime this deterministically
    // forces `try_send` failures past `CHANGE_BUFFER`, which the fake mirrors onto the
    // `dropped` counter exactly as the real adapter's contract does.
    let noise = under("noise");
    for _ in 0..CHANGE_BUFFER + 100 {
        s.watcher.change_of(noise.clone(), FileChangeKind::Modified);
    }

    // The loop notices the counter moved and arms the rescan debounce.
    s.clock
        .deadline_armed_at(s.clock.now() + RESCAN_QUIET)
        .await;
    s.clock.advance(RESCAN_QUIET + PAST_THE_DEADLINE);

    // The observable outcome, not the mechanism: the missed directory is registered, and a
    // change under it reaches a consumer end to end — not merely that a scan was requested
    // again (which would pass even if the scan's result were discarded).
    wait_until("the missed directory is registered", || {
        s.watcher.registered().contains(&missed)
    })
    .await;
    let root_scans_after = s
        .scanner
        .requests()
        .iter()
        .filter(|request| request.root == root())
        .count();
    assert!(
        root_scans_after > root_scans_before,
        "a dropped change must re-scan every open project, not just retry what is already known",
    );

    let inside_missed = under("missed/file.txt");
    s.watcher
        .change_of(inside_missed.clone(), FileChangeKind::Modified);
    expect_change(&mut rx, &inside_missed).await;
}

#[tokio::test]
async fn a_reopened_project_has_its_registrations_re_established() {
    // A project reopened at the same root, with the same restart-eligible globs and the same
    // budget share, looks identical to `resync`'s replan check — nothing about it moved. But the
    // path underneath an unchanged root can have: a fresh clone over a deleted checkout, a
    // directory swapped wholesale. `ProjectOpened` must therefore drop what this project holds
    // and re-establish it, not merely reconcile against the unchanged-looking cached plan.
    let s = setup();
    let project = s.repo.upsert(&root(), None, None).expect("seed").id;
    seed_scan(&s.scanner, root(), &[(under("src"), true)]);
    let ws = spawn_set(&s);
    let mut rx = ws.subscribe();
    wait_until("src registered", || {
        s.watcher.registered().contains(&under("src"))
    })
    .await;
    let unwatched_before = s.watcher.unwatched().len();

    s.bus.publish(DomainEvent::ProjectOpened { id: project });

    wait_until(
        "the root's stale handle is dropped and a fresh one taken",
        || s.watcher.unwatched().contains(&root()) && s.watcher.registered().contains(&root()),
    )
    .await;
    assert!(
        s.watcher.unwatched().len() > unwatched_before,
        "reopening an unchanged project must still drop and re-take its registrations",
    );

    // End to end: the re-established registration is actually delivering.
    let touched = under("src/touched");
    s.watcher
        .change_of(touched.clone(), FileChangeKind::Modified);
    expect_change(&mut rx, &touched).await;
}

#[tokio::test]
async fn only_the_working_trees_refusal_is_reported() {
    let mut s = setup();
    s.watcher.refuse(state_dir());
    let project = s.repo.upsert(&root(), None, None).expect("seed").id;
    let ws = spawn_set(&s);
    let mut rx = ws.subscribe();
    wait_until("the root settles", || {
        s.watcher.registered().contains(&root())
    })
    .await;

    while let Ok(event) = s.rx.try_recv() {
        if let DomainEvent::WatchLimitChanged { project: p, limits } = event {
            if p == project {
                assert!(
                    limits.is_empty(),
                    "a state-dir refusal must not be reported: {limits:?}",
                );
            }
        }
    }

    let touched = under("touched");
    s.watcher
        .change_of(touched.clone(), FileChangeKind::Modified);
    expect_change(&mut rx, &touched).await;
}

#[tokio::test]
async fn a_restart_eligible_projects_root_refusal_marks_both_purposes() {
    let mut s = setup();
    s.watcher.refuse(root());
    let project = s.repo.upsert(&root(), None, None).expect("seed").id;
    register_command(&s, project, "Build", &["src/**/*.rs"]);
    spawn_set(&s);

    // Each purpose settles (and may announce) independently, so the first `WatchLimitChanged`
    // seen for this project can carry only `GitStatus` a moment before `Restarts` catches up —
    // the predicate waits for the settled state where both hold, the same shape
    // `a_refused_session_is_retried_on_the_next_resync` above uses.
    let announced = next_matching(&mut s.rx, |event| {
        matches!(event, DomainEvent::WatchLimitChanged { project: p, limits }
            if *p == project
                && limits.get(&WatchPurpose::GitStatus)
                    == Some(&WatchLimit::Refused(WatchError::BudgetExhausted))
                && limits.get(&WatchPurpose::Restarts)
                    == Some(&WatchLimit::Refused(WatchError::BudgetExhausted)))
    })
    .await;
    let _ = announced;
}
