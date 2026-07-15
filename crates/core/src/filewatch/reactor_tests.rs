//! Behavioural tests for [`WatchReactor`], kept out of the implementation file. They drive a
//! real [`Supervisor`] over fakes plus a [`FakeFileWatcher`] feeding synthetic change events.
//! Waits are event-driven — they await a status transition on the bus ([`wait_all`]), the
//! watcher's `established` signal, or a `FileRestart` — and the debounce window is advanced on
//! the mock clock, so there is no real filesystem, no real time, and no reliance on scheduler
//! timing (which is what makes a `yield_now` budget flake under load).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;

use crate::composition::CorePorts;
use crate::config::ProcessSpec;
use crate::configchange::ConfigSync;
use crate::events::{DomainEvent, EventBus};
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{PtySize, SpawnSpec, TrustRepo};
use crate::process::{ProcStatus, ProcessKind};
use crate::supervisor::{Registration, Supervisor};
use crate::testing::{
    next_matching, wait_all, FakeFileWatcher, FakeProjectRepo, FakeSpawner, FakeTrustRepo,
    MockClock,
};

use super::WatchReactor;

const PROJECT: ProjectId = ProjectId::from_raw(1);
const ROOT: &str = "/project";
/// One advance step, comfortably past the reactor's quiet window so a single step fires it.
const STEP: Duration = Duration::from_millis(400);

struct Setup {
    sup: Arc<Supervisor>,
    clock: MockClock,
    bus: EventBus,
    rx: broadcast::Receiver<DomainEvent>,
    trust: Arc<FakeTrustRepo>,
    watcher: Arc<FakeFileWatcher>,
}

fn setup() -> Setup {
    let bus = EventBus::new(256);
    let rx = bus.subscribe();
    let clock = MockClock::new();
    let trust = Arc::new(FakeTrustRepo::new());
    let ports = CorePorts::builder(
        // Stays Running until terminated, so a restart cycles a live process in place.
        Arc::new(FakeSpawner::exits_on_terminate()),
        Arc::new(clock.clone()),
        trust.clone(),
        Arc::new(FakeProjectRepo::new()),
    )
    .build();
    let sup = Arc::new(Supervisor::new(ports.supervisor_ports(), bus.clone()));
    Setup {
        sup,
        clock,
        bus,
        rx,
        trust,
        watcher: Arc::new(FakeFileWatcher::new()),
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

/// Registers a command with the given watch globs at [`ROOT`], trusting it when `trusted`.
fn register_command(s: &Setup, name: &str, globs: &[&str], trusted: bool) -> ProcessId {
    let spec = watched_spec(globs);
    let id = s
        .sup
        .register(Registration::command(PROJECT, Path::new(ROOT), name, &spec));
    if trusted {
        s.trust
            .set_trusted(PROJECT, &spec.variant_hash())
            .expect("trust");
    }
    id
}

/// Starts a registered command and awaits its `Running` transition on the bus, so a watched
/// change cycles a live process (file-watch reloads a running command, not a resting one).
async fn start_running(s: &mut Setup, id: ProcessId) {
    s.sup.start(id).expect("start");
    wait_all(&mut s.rx, &[id], ProcStatus::Running).await;
}

fn changed(relative: &str) -> PathBuf {
    Path::new(ROOT).join(relative)
}

async fn yield_many() {
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
}

/// Spawns the reactor without waiting — for asserting an ineligible target is never watched.
fn spawn_reactor(s: &Setup) {
    tokio::spawn(
        WatchReactor::new(
            Arc::new(s.clock.clone()),
            s.watcher.clone(),
            &s.bus,
            Arc::downgrade(&s.sup),
        )
        .run(),
    );
}

/// Spawns the reactor and awaits its first established watch (so the fake holds the change
/// sink). Use [`spawn_reactor`] directly when no watch is expected.
async fn start_reactor(s: &Setup) {
    spawn_reactor(s);
    s.watcher.established().await;
}

/// Fires the debounce window and awaits the resulting `FileRestart`. Changes must already be
/// fed; advancing the mock clock past the quiet window wakes the reactor's debounce, which
/// then restarts the command and emits the event the test awaits.
async fn next_file_restart(s: &mut Setup) -> ProcessId {
    s.clock.advance(STEP);
    match next_matching(&mut s.rx, |e| matches!(e, DomainEvent::FileRestart { .. })).await {
        DomainEvent::FileRestart { id } => id,
        other => unreachable!("awaited a FileRestart, got {other:?}"),
    }
}

/// Fires several debounce windows and asserts no `FileRestart` was emitted — the change was
/// ignored, non-matching, or against a command the policy must not reload.
async fn assert_no_file_restart(s: &mut Setup) {
    for _ in 0..5 {
        s.clock.advance(STEP);
        yield_many().await;
    }
    while let Ok(event) = s.rx.try_recv() {
        assert!(
            !matches!(event, DomainEvent::FileRestart { .. }),
            "unexpected FileRestart: {event:?}",
        );
    }
}

#[tokio::test]
async fn a_matching_save_burst_to_a_running_command_triggers_exactly_one_restart() {
    let mut s = setup();
    let web = register_command(&s, "Web", &["src/**/*.rs"], true);
    start_running(&mut s, web).await;
    start_reactor(&s).await;

    // A burst of saves for one logical edit.
    for _ in 0..5 {
        s.watcher.change(changed("src/app/main.rs"));
    }
    yield_many().await;

    // Coalesced into a single restart of exactly that command.
    assert_eq!(next_file_restart(&mut s).await, web);
    // ...and no second restart from the same burst.
    assert_no_file_restart(&mut s).await;
}

#[tokio::test]
async fn an_ignored_or_non_matching_change_to_a_running_command_does_not_restart() {
    let mut s = setup();
    let web = register_command(&s, "Web", &["**/*.rs"], true);
    start_running(&mut s, web).await;
    start_reactor(&s).await;

    // Inside an ignored directory (matches the glob, but ignored), and a non-matching file.
    s.watcher.change(changed("node_modules/dep.rs"));
    s.watcher.change(changed("docs/readme.md"));
    yield_many().await;

    assert_no_file_restart(&mut s).await;
}

#[tokio::test]
async fn a_change_to_a_stopped_command_does_not_start_it() {
    let mut s = setup();
    // Trusted and watch-eligible, but never started: file-watch reloads a running command and
    // must not resurrect a resting one (otherwise an edit would start a command the user
    // stopped, or a restored-but-resting one on launch).
    let web = register_command(&s, "Web", &["src/**/*.rs"], true);
    start_reactor(&s).await;

    s.watcher.change(changed("src/app/main.rs"));
    yield_many().await;

    assert_no_file_restart(&mut s).await;
    assert!(
        s.sup
            .snapshot()
            .iter()
            .any(|v| v.id == web && v.status == ProcStatus::Stopped),
        "the stopped command stays resting after a watched change",
    );
}

#[tokio::test]
async fn an_untrusted_command_is_never_restarted() {
    let mut s = setup();
    // Watched (command + globs) but never trusted: it cannot be started, so it is never
    // running, and a watched change never reloads it (the restart gate also fails closed).
    register_command(&s, "Web", &["src/**/*.rs"], false);
    start_reactor(&s).await;

    s.watcher.change(changed("src/app/main.rs"));
    yield_many().await;

    assert_no_file_restart(&mut s).await;
}

#[tokio::test]
async fn a_project_opened_after_startup_is_watched() {
    let mut s = setup();
    // The reactor starts with no watch-eligible commands, so it watches nothing.
    spawn_reactor(&s);
    yield_many().await;
    assert!(
        s.watcher.watched().is_empty(),
        "there is nothing to watch at startup",
    );

    // A command registered after startup — as opening a project does — becomes watch
    // eligible; the reactor only learns of it when the open is announced.
    let web = register_command(&s, "Web", &["src/**/*.rs"], true);
    start_running(&mut s, web).await;
    s.bus.publish(DomainEvent::ProjectOpened { id: PROJECT });

    // The reactor re-syncs on the open and now establishes the watch for the new command.
    s.watcher.established().await;
    assert!(
        !s.watcher.watched().is_empty(),
        "the project opened after startup is now watched",
    );

    // A matching change to it now restarts it, proving the re-watch is live.
    s.watcher.change(changed("src/app/main.rs"));
    yield_many().await;
    assert_eq!(next_file_restart(&mut s).await, web);
}

#[tokio::test]
async fn a_config_reload_that_adds_a_watched_command_is_watched() {
    let mut s = setup();
    // Nothing watch-eligible at startup.
    spawn_reactor(&s);
    yield_many().await;
    assert!(
        s.watcher.watched().is_empty(),
        "nothing to watch at startup"
    );

    // A solo.yml reload adds a watch-eligible command: the command is registered (as the
    // config engine's reload does) and the reload is announced with ConfigChanged. The reactor
    // must re-sync on that — not only on a project open — or the new command's globs go unwatched
    // until the project is re-opened.
    let web = register_command(&s, "Web", &["src/**/*.rs"], true);
    start_running(&mut s, web).await;
    s.bus.publish(DomainEvent::ConfigChanged {
        project: PROJECT,
        diff: ConfigSync::default(),
        requires_trust: false,
        commands: Vec::new(),
    });

    s.watcher.established().await;
    assert!(
        !s.watcher.watched().is_empty(),
        "the reloaded command is now watched",
    );

    // A matching change now restarts it, proving the re-watch is live.
    s.watcher.change(changed("src/app/main.rs"));
    yield_many().await;
    assert_eq!(next_file_restart(&mut s).await, web);
}

#[tokio::test]
async fn a_removed_projects_root_watch_is_released() {
    let mut s = setup();
    let web = register_command(&s, "Web", &["src/**/*.rs"], true);
    start_running(&mut s, web).await;
    start_reactor(&s).await;
    assert_eq!(s.watcher.live(), vec![PathBuf::from(ROOT)]);

    // Project removal's teardown: its processes close, then the removal is announced. The
    // reactor re-syncs on the announcement; with no watch-eligible command left at ROOT,
    // the root's OS watch is dropped (releasing its resources), not merely unmatched.
    s.sup.close_all(PROJECT).await;
    s.bus.publish(DomainEvent::ProjectRemoved { id: PROJECT });

    s.watcher.released().await;
    assert!(
        s.watcher.live().is_empty(),
        "the removed project's watch is dropped",
    );
}

#[tokio::test]
async fn a_terminal_or_a_glob_less_command_is_not_watched() {
    let s = setup();
    // A terminal (never file-watched) and a command with no globs — neither is eligible.
    s.sup.register(Registration::launched(
        PROJECT,
        ProcessKind::Terminal,
        "shell",
        SpawnSpec {
            command: "bash".into(),
            working_dir: PathBuf::from(ROOT),
            env: BTreeMap::new(),
            size: PtySize::default(),
        },
    ));
    register_command(&s, "NoGlob", &[], true);

    spawn_reactor(&s);
    yield_many().await;

    assert!(
        s.watcher.watched().is_empty(),
        "an ineligible target is never watched",
    );
}
