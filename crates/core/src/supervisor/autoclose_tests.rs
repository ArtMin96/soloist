use super::*;
use crate::process::ProcStatus;
use crate::supervisor::test_support::{harness, terminal, wait_all, Harness};
use crate::testing::FakeSpawner;
use std::time::Duration;
use tokio::sync::broadcast;

/// How long a test waits for a removal before calling it a failure. Generous for a loaded CI
/// box, but bounded so a policy that stops closing fails the run instead of hanging it.
const REMOVAL_TIMEOUT: Duration = Duration::from_secs(10);

/// Awaits `id` leaving the registry, failing rather than hanging if it never does.
async fn wait_removed(rx: &mut broadcast::Receiver<DomainEvent>, id: ProcessId) {
    let removal = async {
        loop {
            match rx.recv().await {
                Ok(DomainEvent::ProcessRemoved { id: got }) if got == id => return,
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => panic!("event bus closed"),
            }
        }
    };
    tokio::time::timeout(REMOVAL_TIMEOUT, removal)
        .await
        .expect("the process is closed once its run ends");
}

fn is_registered(h: &Harness, id: ProcessId) -> bool {
    h.sup.snapshot().iter().any(|view| view.id == id)
}

#[tokio::test]
async fn an_armed_process_is_closed_when_its_run_ends() {
    let mut h = harness(FakeSpawner::exits_with_code(0));
    let id = terminal(&h.sup, "work");
    h.sup.close_when_done(id);
    tokio::spawn(h.sup.auto_close_loop());

    h.sup.start(id).expect("start");
    wait_removed(&mut h.rx, id).await;

    assert!(
        !is_registered(&h, id),
        "a finished armed process is forgotten, not left resting"
    );
}

#[tokio::test]
async fn an_armed_process_that_crashes_is_closed_too() {
    // A run that fails is still a run that ended: the caller asked not to keep the row, and a
    // crashed one left behind would leak exactly as a cleanly-exited one does.
    let mut h = harness(FakeSpawner::exits_with_code(1));
    let id = terminal(&h.sup, "boom");
    h.sup.close_when_done(id);
    tokio::spawn(h.sup.auto_close_loop());

    h.sup.start(id).expect("start");
    wait_removed(&mut h.rx, id).await;

    assert!(!is_registered(&h, id));
}

#[tokio::test]
async fn an_unarmed_process_rests_in_the_registry_after_it_exits() {
    // The default, and why arming is opt-in: an exited process keeps its row and its output so
    // the user can read what it did.
    let mut h = harness(FakeSpawner::exits_with_code(0));
    let armed = terminal(&h.sup, "armed");
    let unarmed = terminal(&h.sup, "unarmed");
    h.sup.close_when_done(armed);
    tokio::spawn(h.sup.auto_close_loop());

    h.sup.start(unarmed).expect("start unarmed");
    h.sup.start(armed).expect("start armed");
    // The armed one's removal proves the reactor has run; the unarmed one is untouched by it.
    wait_removed(&mut h.rx, armed).await;

    assert!(is_registered(&h, unarmed), "an unarmed process is kept");
    assert_eq!(
        h.sup.view(unarmed).map(|view| view.status),
        Some(ProcStatus::Stopped)
    );
}

#[tokio::test]
async fn a_rescan_leaves_an_armed_process_that_has_not_run_yet() {
    // Arming happens before the launch, so an armed process rests for a moment with its run
    // still ahead of it. A rescan landing in that window must not read that resting status as
    // the end of a run and close the process before it ever starts.
    let h = harness(FakeSpawner::exits_with_code(0));
    let waiting = terminal(&h.sup, "waiting");
    h.sup.close_when_done(waiting);

    h.sup.rescan_finished().await;

    assert!(
        is_registered(&h, waiting),
        "a process that has not run yet keeps its registration"
    );
}

#[tokio::test]
async fn a_lagged_reactor_still_closes_a_finished_process() {
    // Simulates the reactor missing the terminal delta (broadcast lag): the loop is never
    // started, so nothing reacts to the exit. Driving the rescan the Lagged arm runs must still
    // close it — a finished process emits nothing further, so a dropped delta would otherwise
    // strand it forever.
    let mut h = harness(FakeSpawner::exits_with_code(0));
    let id = terminal(&h.sup, "work");
    h.sup.close_when_done(id);
    h.sup.start(id).expect("start");
    wait_all(&mut h.rx, &[id], ProcStatus::Stopped).await;
    // Observed from the event stream, as the running reactor would have.
    h.sup.auto_close.observe_active(id);

    h.sup.rescan_finished().await;

    assert!(!is_registered(&h, id));
}
