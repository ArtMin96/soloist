//! Behavioural tests for [`WatchReactor`], kept out of the implementation file. They drive a
//! real [`Supervisor`] over fakes, feeding synthetic changed paths straight onto a broadcast
//! channel the reactor consumes in place of [`crate::watchset::ProjectWatchSet`]'s fan-out —
//! which of a project's directories are actually watched, and what a refusal or a degradation
//! means, is that module's own concern and its own test coverage; this file pins only what the
//! reactor decides once a change has already been reported: which changed paths, for which
//! commands, are worth a debounced restart.
//!
//! Waits are event-driven — they await a status transition on the bus ([`wait_all`]), a
//! `FileRestart`, or the reactor arming its debounce deadline ([`MockClock::deadline_armed_at`])
//! — and the debounce window is then advanced on the mock clock, so there is no real filesystem,
//! no real time, and no reliance on scheduler timing (which is what makes a `yield_now` budget
//! flake under load).
//!
//! The one remaining `yield_now` budget is [`yield_many`], and only where the assertion is that
//! nothing happens: there is no event to await for an effect that must never occur, and a budget
//! that runs short there weakens the assertion rather than hanging the test.

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
use crate::ports::{Clock, PtySize, SpawnSpec, TrustRepo};
use crate::process::{ProcStatus, ProcessKind};
use crate::supervisor::{Registration, Supervisor};
use crate::testing::{
    next_matching, wait_all, FakeProjectRepo, FakeSpawner, FakeTrustRepo, MockClock,
};

use super::{WatchReactor, QUIET};

const PROJECT: ProjectId = ProjectId::from_raw(1);
const ROOT: &str = "/project";
/// One advance step, comfortably past the reactor's quiet window so a single step fires it.
const STEP: Duration = Duration::from_millis(400);
/// How many changed paths the test's fan-out buffers before a send starts dropping — generous
/// for a single test's traffic.
const CHANGE_BUFFER: usize = 256;
/// How long one [`changed_until_armed`] attempt waits before resending — short, since a
/// resync that is going to catch up does so within a scheduler tick.
const RETRY_STEP: Duration = Duration::from_millis(50);
/// How many times [`changed_until_armed`] resends before giving up — generous relative to
/// [`RETRY_STEP`], so a resync that never catches up fails the test loudly instead of hanging.
const RETRY_LIMIT: usize = 20;

struct Setup {
    sup: Arc<Supervisor>,
    clock: MockClock,
    bus: EventBus,
    rx: broadcast::Receiver<DomainEvent>,
    trust: Arc<FakeTrustRepo>,
    changes: broadcast::Sender<PathBuf>,
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
    let (changes, _rx) = broadcast::channel(CHANGE_BUFFER);
    Setup {
        sup,
        clock,
        bus,
        rx,
        trust,
        changes,
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
            .set_trusted(PROJECT, &spec.variant_hash(), &spec.command)
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

/// Spawns the reactor over the test's own change fan-out.
fn spawn_reactor(s: &Setup) {
    tokio::spawn(
        WatchReactor::new(
            Arc::new(s.clock.clone()),
            s.changes.subscribe(),
            &s.bus,
            Arc::downgrade(&s.sup),
        )
        .run(),
    );
}

/// Fires the debounce window and awaits the resulting `FileRestart`. Changes must already be
/// fed; advancing the mock clock past the quiet window wakes the reactor's debounce, which
/// then restarts the command and emits the event the test awaits.
///
/// The advance waits for the reactor to have armed that window first — the window this burst
/// opened, named as the clock's current reading plus [`QUIET`], since the mock clock has not moved
/// since the changes were fed. Feeding a change only puts it on the channel — the reactor is woken
/// to consume it from another thread — so advancing straight away can move time before any
/// deadline exists, leaving the burst armed one whole window in the future with no advance left to
/// reach it.
async fn next_file_restart(s: &mut Setup) -> ProcessId {
    s.clock.deadline_armed_at(s.clock.now() + QUIET).await;
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

/// Sends `path` on the change fan-out, retrying until the reactor arms a debounce deadline for
/// it — bounded, so a resync that never catches up fails the test loudly rather than hanging.
///
/// A live resync (on `ProjectOpened`/`ConfigChanged`) rebuilds the reactor's match rules
/// concurrently with whatever the test does next: a real filesystem watch cannot report a change
/// before the directory it lives under is registered, but a test feeding a synthetic change
/// straight onto the fan-out can race ahead of the resync that would have matched it, in which
/// case the change simply matches nothing and has to be sent again once the rules have caught
/// up. Resending costs nothing here — every burst test already sends the same path several times
/// and asserts it coalesces into one restart.
async fn changed_until_armed(s: &Setup, path: &Path) {
    let deadline = s.clock.now() + QUIET;
    for attempt in 0..RETRY_LIMIT {
        let _ = s.changes.send(path.to_path_buf());
        if tokio::time::timeout(RETRY_STEP, s.clock.deadline_armed_at(deadline))
            .await
            .is_ok()
        {
            return;
        }
        assert!(
            attempt + 1 < RETRY_LIMIT,
            "the reactor never armed a debounce for {}",
            path.display(),
        );
    }
}

#[tokio::test]
async fn a_matching_save_burst_to_a_running_command_triggers_exactly_one_restart() {
    let mut s = setup();
    let web = register_command(&s, "Web", &["src/**/*.rs"], true);
    start_running(&mut s, web).await;
    spawn_reactor(&s);

    // A burst of saves for one logical edit.
    for _ in 0..5 {
        let _ = s.changes.send(changed("src/app/main.rs"));
    }

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
    spawn_reactor(&s);

    // Inside an ignored directory (matches the glob, but ignored), and a non-matching file.
    let _ = s.changes.send(changed("node_modules/dep.rs"));
    let _ = s.changes.send(changed("docs/readme.md"));
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
    spawn_reactor(&s);

    let _ = s.changes.send(changed("src/app/main.rs"));
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
    spawn_reactor(&s);

    let _ = s.changes.send(changed("src/app/main.rs"));
    yield_many().await;

    assert_no_file_restart(&mut s).await;
}

#[tokio::test]
async fn a_project_opened_after_startup_is_matched() {
    let mut s = setup();
    // The reactor starts with no watch-eligible commands, so a change matches nothing.
    spawn_reactor(&s);
    let _ = s.changes.send(changed("src/app/main.rs"));
    yield_many().await;
    assert_no_file_restart(&mut s).await;

    // A command registered after startup — as opening a project does — becomes watch
    // eligible; the reactor only learns of it when the open is announced.
    let web = register_command(&s, "Web", &["src/**/*.rs"], true);
    start_running(&mut s, web).await;
    s.bus.publish(DomainEvent::ProjectOpened { id: PROJECT });

    // A matching change now restarts it, once the live resync has rebuilt the match rules.
    changed_until_armed(&s, &changed("src/app/main.rs")).await;
    assert_eq!(next_file_restart(&mut s).await, web);
}

#[tokio::test]
async fn a_config_reload_that_adds_a_watched_command_is_matched() {
    let mut s = setup();
    // Nothing watch-eligible at startup — proven, not assumed: the initial resync must have
    // actually run against the empty registry before a command is registered, or the assertion
    // below would prove nothing about the *live* resync this test exists to check.
    spawn_reactor(&s);
    let _ = s.changes.send(changed("src/app/main.rs"));
    yield_many().await;
    assert_no_file_restart(&mut s).await;

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

    // A matching change now restarts it, once the live resync has rebuilt the match rules.
    changed_until_armed(&s, &changed("src/app/main.rs")).await;
    assert_eq!(next_file_restart(&mut s).await, web);
}

#[tokio::test]
async fn a_terminal_or_a_glob_less_command_is_not_matched() {
    let mut s = setup();
    // A terminal (never file-watched) and a command with no globs — neither is eligible, so no
    // rule is ever built for either and a change against their paths matches nothing.
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

    let _ = s.changes.send(changed("anything.rs"));
    yield_many().await;

    assert_no_file_restart(&mut s).await;
}
