//! Lifecycle behaviours that relabel or forget a managed process: [`Supervisor::rename`] and
//! [`Supervisor::close`]. Close reaps a live actor's group before dropping the entry, so the
//! "no orphaned children" guarantee is exercised here under the mock clock.

use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::events::DomainEvent;
use crate::ids::ProcessId;
use crate::process::ProcStatus;
use crate::supervisor::test_support::{harness, next_to, terminal};
use crate::supervisor::SupervisorError;
use crate::testing::FakeSpawner;

/// A duration safely past the actor's SIGTERM→SIGKILL grace window.
const PAST_GRACE: Duration = Duration::from_secs(6);

/// Waits for the next `ProcessRenamed` and returns its id and new label.
async fn next_renamed(rx: &mut broadcast::Receiver<DomainEvent>) -> (ProcessId, String) {
    loop {
        match rx.recv().await {
            Ok(DomainEvent::ProcessRenamed { id, label }) => return (id, label),
            Ok(_) | Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => panic!("event bus closed"),
        }
    }
}

/// Waits for the next `ProcessRemoved` and returns its id.
async fn next_removed(rx: &mut broadcast::Receiver<DomainEvent>) -> ProcessId {
    loop {
        match rx.recv().await {
            Ok(DomainEvent::ProcessRemoved { id }) => return id,
            Ok(_) | Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => panic!("event bus closed"),
        }
    }
}

#[tokio::test]
async fn rename_updates_the_label_and_announces_it() {
    let mut h = harness(FakeSpawner::exits_on_kill());
    let id = terminal(&h.sup, "sleep 60");

    h.sup.rename(id, "renamed".into()).expect("rename");
    assert_eq!((id, "renamed".to_string()), next_renamed(&mut h.rx).await);
    assert_eq!(
        h.sup.view(id).expect("registered").label,
        "renamed",
        "the read model reflects the new label"
    );
}

#[tokio::test]
async fn renaming_an_unknown_process_is_not_found() {
    let h = harness(FakeSpawner::exits_on_kill());
    assert!(matches!(
        h.sup.rename(ProcessId::from_raw(999), "x".into()),
        Err(SupervisorError::NotFound(_))
    ));
}

#[tokio::test]
async fn close_removes_a_resting_process_and_announces_it() {
    let mut h = harness(FakeSpawner::exits_on_kill());
    let id = terminal(&h.sup, "sleep 60");
    // Never started: no live actor, so close is a pure removal.
    h.sup.close(id).await.expect("close");
    assert_eq!(id, next_removed(&mut h.rx).await);
    assert!(
        h.sup.view(id).is_none(),
        "a closed process leaves the registry"
    );
}

#[tokio::test]
async fn closing_an_unknown_process_is_not_found() {
    let h = harness(FakeSpawner::exits_on_kill());
    assert!(matches!(
        h.sup.close(ProcessId::from_raw(999)).await,
        Err(SupervisorError::NotFound(_))
    ));
}

#[tokio::test]
async fn close_reaps_a_running_process_before_removing_it() {
    // The fake child ignores SIGTERM, so close cannot return until the grace window elapses
    // and SIGKILL reaps the group — proving close awaits the reap, never abandoning a child.
    let mut h = harness(FakeSpawner::exits_on_kill());
    let id = terminal(&h.sup, "sleep 60");
    h.sup.start(id).expect("start");
    assert_eq!(next_to(&mut h.rx).await, ProcStatus::Starting);
    assert_eq!(next_to(&mut h.rx).await, ProcStatus::Running);

    let sup = h.sup.clone();
    let closing = tokio::spawn(async move { sup.close(id).await });
    // The actor messages Stop and waits out the grace window before SIGKILL.
    assert_eq!(next_to(&mut h.rx).await, ProcStatus::Stopping);
    tokio::task::yield_now().await;
    h.clock.advance(PAST_GRACE);

    closing
        .await
        .expect("join the close task")
        .expect("close succeeds");
    assert_eq!(id, next_removed(&mut h.rx).await);
    assert!(h.sup.view(id).is_none(), "the reaped process is gone");
}
