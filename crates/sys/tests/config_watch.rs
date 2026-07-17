//! Integration check for the config-watch chain over the real `notify` adapter: an external
//! edit to an open project's `solo.yml` on a real filesystem reaches a reload — announced as
//! `ConfigChanged` with its trust review — through the real watcher, the real debounce
//! window, and the real sync engine. The mock-clock policy behaviour is covered in the
//! core's reactor tests; this proves the live wiring end to end, headless. Uses real time
//! (a bounded await), like the other OS-adapter integration tests.

use std::sync::Arc;
use std::time::Duration;

use soloist_core::testing::{FakeProjectRepo, FakeSpawner, FakeTrustRepo};
use soloist_core::{
    ConfigEngine, ConfigWatchReactor, CorePorts, DomainEvent, EventBus, ProjectService, Projects,
    Supervisor, TokioClock,
};
use soloist_sys::NotifyFileWatcher;

/// How long to wait for the debounced reload before giving up — the quiet window plus
/// generous inotify slack, so a loaded CI box does not flake.
const BUDGET: Duration = Duration::from_secs(10);

#[tokio::test]
async fn an_external_edit_reaches_a_reload_through_the_real_watcher() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("solo.yml"),
        "processes:\n  Echo:\n    command: echo one\n",
    )
    .expect("write solo.yml");

    let bus = EventBus::new(1024);
    let mut rx = bus.subscribe();
    let trust = Arc::new(FakeTrustRepo::new());
    let repo = Arc::new(FakeProjectRepo::new());
    let ports = CorePorts::builder(
        Arc::new(FakeSpawner::exits_on_terminate()),
        Arc::new(TokioClock),
        trust.clone(),
        repo.clone(),
    )
    .build();
    let sup = Arc::new(Supervisor::new(ports.supervisor_ports(), bus.clone()));
    let projects = Arc::new(Projects::new(repo));
    let config = Arc::new(ConfigEngine::new(trust, bus.clone()));

    ProjectService::new(&projects, &config, &sup, &bus)
        .open(dir.path())
        .expect("open project");
    tokio::spawn(
        ConfigWatchReactor::new(
            Arc::new(TokioClock),
            Arc::new(NotifyFileWatcher::new()),
            &bus,
            Arc::downgrade(&sup),
            projects,
            config,
        )
        .run(),
    );
    // The reactor establishes its watch synchronously in its startup re-sync; yielding lets
    // the spawned task reach it before the edit lands.
    tokio::task::yield_now().await;

    std::fs::write(
        dir.path().join("solo.yml"),
        "processes:\n  Echo:\n    command: echo two\n",
    )
    .expect("edit solo.yml");

    let changed = tokio::time::timeout(BUDGET, async {
        loop {
            match rx.recv().await.expect("event bus open") {
                DomainEvent::ConfigChanged {
                    diff,
                    requires_trust,
                    ..
                } => break (diff, requires_trust),
                _ => continue,
            }
        }
    })
    .await
    .expect("the external edit is announced as ConfigChanged within the budget");

    assert_eq!(changed.0.updated, vec!["Echo".to_string()]);
    assert!(changed.1, "the changed variant needs re-trust");
}
