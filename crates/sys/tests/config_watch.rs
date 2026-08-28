//! Integration check for the config-watch chain over the real composition: an external edit to
//! an open project's `solo.yml` on a real filesystem reaches a reload — announced as
//! `ConfigChanged` with its trust review — through the real `notify` watcher, the real
//! `ignore`-backed scanner, the watch set that plans and maintains the registration over both,
//! the real debounce window, and the real sync engine. The mock-clock policy behaviour is
//! covered in the core's reactor tests; this proves the live wiring end to end, headless,
//! exactly as the composition root assembles it — driving the config-watch reactor with no
//! watcher of its own, only the watch set's fan-out. Uses real time (a bounded await), like the
//! other OS-adapter integration tests.

use std::sync::Arc;
use std::time::Duration;

use soloist_core::testing::{FakeProjectRepo, FakeSpawner, FakeTrustRepo};
use soloist_core::{CorePorts, DomainEvent, Facade, TokioClock};
use soloist_sys::{IgnoreWatchScanner, NotifyFileWatcher};

/// How long to wait for the debounced reload before giving up — the quiet window plus
/// generous inotify slack, so a loaded CI box does not flake.
const BUDGET: Duration = Duration::from_secs(10);

#[tokio::test]
async fn an_external_edit_reaches_a_reload_through_the_real_watch_set() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("solo.yml"),
        "processes:\n  Echo:\n    command: echo one\n",
    )
    .expect("write solo.yml");

    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .file_watcher(Arc::new(NotifyFileWatcher::new()))
        .watch_scanner(Arc::new(IgnoreWatchScanner::new()))
        .build(),
    );
    let mut rx = facade.subscribe();

    facade.load_project(dir.path()).expect("open project");
    // The composition root's own order: the watch set must already be serving the fan-out
    // before the reactor that consumes it starts.
    tokio::spawn(facade.watch_set_loop());
    tokio::spawn(facade.config_watch_loop());

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
