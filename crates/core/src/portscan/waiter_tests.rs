//! Behavioural tests for [`wait_for_port`], kept out of the implementation file. They drive
//! a real [`Supervisor`] over fakes and the mock clock, flipping the fake probe to simulate
//! a server binding — so the wait, its readiness gate, and the timeout are deterministic
//! with no real time and no real `/proc` read.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::events::DomainEvent;
use crate::ids::ProcessId;
use crate::portscan::test_support::{running_process, setup, terminal, view_of, ADVANCE_STEP};
use crate::testing::FakePortProbe;

use super::{wait_for_port, WaitForPortError};

const PORT: u16 = 8080;

/// Drains any `ReadyStateChanged` for `id` currently queued, returning the booleans in order.
fn drain_ready(rx: &mut broadcast::Receiver<DomainEvent>, id: ProcessId) -> Vec<bool> {
    let mut seen = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let DomainEvent::ReadyStateChanged { id: got, ready } = event {
            if got == id {
                seen.push(ready);
            }
        }
    }
    seen
}

/// Advances the clock and yields until the spawned wait resolves, then returns its result.
async fn drive(
    task: JoinHandle<Result<(), WaitForPortError>>,
    clock: &crate::testing::MockClock,
) -> Result<(), WaitForPortError> {
    for _ in 0..200 {
        if task.is_finished() {
            break;
        }
        clock.advance(ADVANCE_STEP);
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
    }
    task.await.expect("wait task joins")
}

#[tokio::test]
async fn resolves_immediately_when_the_port_is_already_bound() {
    let mut s = setup();
    let id = running_process(&mut s).await;
    let probe = FakePortProbe::returning(vec![PORT]);

    wait_for_port(
        s.sup.clone(),
        Arc::new(probe),
        Arc::new(s.clock.clone()),
        id,
        PORT,
        Duration::from_secs(5),
    )
    .await
    .expect("an already-bound port resolves");

    assert_eq!(view_of(&s.sup, id).ready, Some(true));
    // Ready went straight to true — no spurious "not ready" flicker first.
    assert_eq!(drain_ready(&mut s.rx, id), vec![true]);
}

#[tokio::test]
async fn waits_not_ready_then_resolves_when_the_port_binds() {
    let mut s = setup();
    let id = running_process(&mut s).await;
    let probe = FakePortProbe::returning(vec![]);

    let task = tokio::spawn(wait_for_port(
        s.sup.clone(),
        Arc::new(probe.clone()),
        Arc::new(s.clock.clone()),
        id,
        PORT,
        Duration::from_secs(60),
    ));

    // The wait announces Running-but-not-Ready and parks on its poll.
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    assert_eq!(view_of(&s.sup, id).ready, Some(false));
    assert_eq!(drain_ready(&mut s.rx, id), vec![false]);

    // The server binds; the next poll sees it and the wait resolves Ready.
    probe.set(vec![PORT]);
    drive(task, &s.clock).await.expect("resolves once bound");
    assert_eq!(view_of(&s.sup, id).ready, Some(true));
    assert_eq!(drain_ready(&mut s.rx, id), vec![true]);
}

#[tokio::test]
async fn times_out_when_the_port_never_binds() {
    let mut s = setup();
    let id = running_process(&mut s).await;
    let probe = FakePortProbe::returning(vec![]);

    let task = tokio::spawn(wait_for_port(
        s.sup.clone(),
        Arc::new(probe),
        Arc::new(s.clock.clone()),
        id,
        PORT,
        Duration::from_secs(5),
    ));

    assert_eq!(drive(task, &s.clock).await, Err(WaitForPortError::Timeout));
    // The process is still Running but stays not-ready — the gate reflects the failed wait.
    assert_eq!(view_of(&s.sup, id).ready, Some(false));
}

#[tokio::test]
async fn errors_when_the_process_is_not_running() {
    let s = setup();
    let id = terminal(&s.sup); // registered, never started — no live group.

    let result = wait_for_port(
        s.sup.clone(),
        Arc::new(FakePortProbe::returning(vec![PORT])),
        Arc::new(s.clock.clone()),
        id,
        PORT,
        Duration::from_secs(5),
    )
    .await;

    assert_eq!(result, Err(WaitForPortError::NotRunning));
    assert_eq!(view_of(&s.sup, id).ready, None);
}
