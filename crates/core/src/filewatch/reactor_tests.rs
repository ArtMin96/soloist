//! Behavioural tests for [`WatchReactor`], kept out of the implementation file. They drive a
//! real [`Supervisor`] over fakes plus a [`FakeFileWatcher`] feeding synthetic change events,
//! so timing is deterministic on the mock clock with no real filesystem and no real time.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;

use crate::config::ProcessSpec;
use crate::events::{DomainEvent, EventBus};
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{CorePorts, PtySize, SpawnSpec, TrustRepo};
use crate::process::ProcessKind;
use crate::supervisor::{Registration, Supervisor};
use crate::testing::{FakeFileWatcher, FakeProjectRepo, FakeSpawner, FakeTrustRepo, MockClock};

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
    let sup = Arc::new(Supervisor::new(&ports, bus.clone()));
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

/// Spawns the reactor and waits until it has established a watch (so the fake holds the change
/// sink). Fails if no watch appears — use [`spawn_reactor`] when none is expected.
async fn start_reactor(s: &Setup) {
    spawn_reactor(s);
    for _ in 0..50 {
        tokio::task::yield_now().await;
        if !s.watcher.watched().is_empty() {
            return;
        }
    }
    panic!("the reactor never established a watch");
}

/// Advances the clock in steps, draining events, until a `FileRestart` is observed.
async fn next_file_restart(s: &mut Setup) -> ProcessId {
    for _ in 0..50 {
        s.clock.advance(STEP);
        yield_many().await;
        while let Ok(event) = s.rx.try_recv() {
            if let DomainEvent::FileRestart { id } = event {
                return id;
            }
        }
    }
    panic!("no FileRestart within the budget");
}

/// Advances several windows and asserts no `FileRestart` was emitted.
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
async fn a_matching_save_burst_triggers_exactly_one_restart() {
    let mut s = setup();
    let web = register_command(&s, "Web", &["src/**/*.rs"], true);
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
async fn an_ignored_or_non_matching_change_does_not_restart() {
    let mut s = setup();
    register_command(&s, "Web", &["**/*.rs"], true);
    start_reactor(&s).await;

    // Inside an ignored directory (matches the glob, but ignored), and a non-matching file.
    s.watcher.change(changed("node_modules/dep.rs"));
    s.watcher.change(changed("docs/readme.md"));
    yield_many().await;

    assert_no_file_restart(&mut s).await;
}

#[tokio::test]
async fn an_untrusted_command_is_not_restarted() {
    let mut s = setup();
    // Watched (command + globs) but never trusted — the restart gate fails closed.
    register_command(&s, "Web", &["src/**/*.rs"], false);
    start_reactor(&s).await;

    s.watcher.change(changed("src/app/main.rs"));
    yield_many().await;

    assert_no_file_restart(&mut s).await;
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
