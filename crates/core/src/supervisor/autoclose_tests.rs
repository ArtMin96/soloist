use super::*;
use crate::process::ProcStatus;
use crate::supervisor::test_support::{harness, terminal, wait_all, Harness};
use crate::testing::FakeSpawner;
use std::time::Duration;
use tokio::sync::broadcast;

/// How long a test waits for a removal before calling it a failure. Generous for a loaded CI
/// box, but bounded so a policy that stops closing fails the run instead of hanging it.
const REMOVAL_TIMEOUT: Duration = Duration::from_secs(10);

/// A duration safely past the actor's SIGTERM→SIGKILL grace window, advanced on the mock clock so
/// a stop completes without real time passing.
const PAST_GRACE: Duration = Duration::from_secs(6);

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

/// Whether `id`'s rendered output still holds `needle` — the scrollback a close would have freed.
fn output_holds(h: &Harness, id: ProcessId, needle: &str) -> bool {
    h.sup
        .rendered(id)
        .is_some_and(|screen| screen.lines.iter().any(|line| line.contains(needle)))
}

#[tokio::test]
async fn an_armed_process_is_closed_when_its_run_ends() {
    let mut h = harness(FakeSpawner::exits_with_code(0));
    let id = terminal(&h.sup, "work");
    h.sup.close_when_done(id, ClosePolicy::WhenRunEnds);
    tokio::spawn(h.sup.auto_close_loop());

    h.sup.start(id).expect("start");
    wait_removed(&mut h.rx, id).await;

    assert!(
        !is_registered(&h, id),
        "a finished armed process is forgotten, not left resting"
    );
}

#[tokio::test]
async fn an_armed_process_that_crashes_keeps_its_row_and_its_crash_output() {
    // A crash is not a run that ended on its own — it is the one ending with something to read.
    // Reaping it would take the crash output with it before anyone could see what went wrong.
    let mut h = harness(FakeSpawner::streams_then_crashes(
        vec![b"panicked at the disco\n".to_vec()],
        1,
    ));
    let id = terminal(&h.sup, "boom");
    h.sup.close_when_done(id, ClosePolicy::WhenRunEnds);

    h.sup.start(id).expect("start");
    wait_all(&mut h.rx, &[id], ProcStatus::Crashed).await;
    // Driven directly rather than through the reactor, so this is what the reactor would decide
    // rather than a race against it deciding nothing yet.
    h.sup.close_if_run_ended(id).await;

    assert!(is_registered(&h, id), "a crashed process keeps its row");
    assert!(
        output_holds(&h, id, "panicked at the disco"),
        "and the output that says why"
    );
}

#[tokio::test]
async fn an_armed_process_the_caller_stopped_keeps_its_row_and_its_output() {
    // Stop is someone wanting to look at what happened, not to discard it: the run was ended for
    // the process, so the resting status that follows is not a run that ended on its own.
    let mut h = harness(FakeSpawner::streams_then_stays_alive(vec![
        b"half way through\n".to_vec(),
    ]));
    let id = terminal(&h.sup, "work");
    h.sup.close_when_done(id, ClosePolicy::WhenRunEnds);

    h.sup.start(id).expect("start");
    wait_all(&mut h.rx, &[id], ProcStatus::Running).await;
    assert!(h.sup.stop(id), "the running process is messaged");
    wait_all(&mut h.rx, &[id], ProcStatus::Stopping).await;
    tokio::task::yield_now().await;
    h.clock.advance(PAST_GRACE);
    wait_all(&mut h.rx, &[id], ProcStatus::Stopped).await;
    h.sup.close_if_run_ended(id).await;

    assert!(
        is_registered(&h, id),
        "a process the caller stopped keeps its row"
    );
    assert!(
        output_holds(&h, id, "half way through"),
        "and the scrollback behind it"
    );
}

#[tokio::test]
async fn an_armed_process_that_owes_a_handover_keeps_its_row_until_it_is_made() {
    // A run whose result never reached anyone is a run nobody has read: closing it discards the
    // work and the record of it at once. The handover is the caller's own signal that it landed.
    let mut h = harness(FakeSpawner::exits_with_code(0));
    let silent = terminal(&h.sup, "silent");
    let handed_over = terminal(&h.sup, "handed-over");
    for id in [silent, handed_over] {
        h.sup
            .close_when_done(id, ClosePolicy::WhenRunEndsAndHandedOver);
    }
    h.sup.record_handover(handed_over);
    tokio::spawn(h.sup.auto_close_loop());

    h.sup.start(silent).expect("start silent");
    wait_all(&mut h.rx, &[silent], ProcStatus::Stopped).await;
    // Ended after the silent one, so the reactor has demonstrably passed that run's end by the
    // time this one's removal arrives.
    h.sup.start(handed_over).expect("start handed-over");
    wait_removed(&mut h.rx, handed_over).await;

    assert!(
        is_registered(&h, silent),
        "a run whose result never reached anyone keeps its row"
    );
}

#[tokio::test]
async fn a_stale_exit_never_reaps_a_process_that_has_since_started_again() {
    // The reactor can block on one process's close for a whole SIGTERM grace, and drain another's
    // exit event only after that process has been started again. Acting on the status the queued
    // event carried, rather than the one the registry holds now, reaps a live child mid-run.
    let (spawner, exits) = FakeSpawner::exits_when_told();
    let mut h = harness(spawner);
    let id = terminal(&h.sup, "work");
    h.sup.close_when_done(id, ClosePolicy::WhenRunEnds);

    h.sup.start(id).expect("start");
    wait_all(&mut h.rx, &[id], ProcStatus::Running).await;
    exits.fire(id);
    wait_all(&mut h.rx, &[id], ProcStatus::Stopped).await;
    h.sup.start(id).expect("start again");
    wait_all(&mut h.rx, &[id], ProcStatus::Running).await;

    // The reactor drains the first run's exit only now, long after it stopped describing the
    // process.
    h.sup.close_if_run_ended(id).await;

    assert!(is_registered(&h, id), "the running process is not reaped");
    assert_eq!(
        h.sup.view(id).map(|view| view.status),
        Some(ProcStatus::Running),
        "and its second run is still going"
    );
}

#[tokio::test]
async fn an_unarmed_process_rests_in_the_registry_after_it_exits() {
    // The default, and why arming is opt-in: an exited process keeps its row and its output so
    // the user can read what it did.
    let mut h = harness(FakeSpawner::exits_with_code(0));
    let armed = terminal(&h.sup, "armed");
    let unarmed = terminal(&h.sup, "unarmed");
    h.sup.close_when_done(armed, ClosePolicy::WhenRunEnds);
    h.sup.close_when_done(unarmed, ClosePolicy::Keep);
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
    h.sup.close_when_done(waiting, ClosePolicy::WhenRunEnds);

    h.sup.rescan_finished().await;

    assert!(
        is_registered(&h, waiting),
        "a process that has not run yet keeps its registration"
    );
}

#[tokio::test]
async fn a_rescan_closes_a_finished_process_whose_whole_run_the_reactor_missed() {
    // Broadcast lag drops every event of a run — its launch as well as its exit — so a reactor
    // that learned "this one has started" from the stream would never consider it again, and its
    // row and buffers would be stranded for the rest of the session: the very leak arming asked
    // to avoid. The loop is never started here, so nothing has observed anything.
    let mut h = harness(FakeSpawner::exits_with_code(0));
    let id = terminal(&h.sup, "work");
    h.sup.close_when_done(id, ClosePolicy::WhenRunEnds);
    h.sup.start(id).expect("start");
    wait_all(&mut h.rx, &[id], ProcStatus::Stopped).await;

    h.sup.rescan_finished().await;

    assert!(!is_registered(&h, id));
}
