//! Deterministic event-stream waiters shared by the contexts' supervisor and reactor tests.
//!
//! A test that needs to wait for an asynchronous effect — a process reaching a state, a
//! file-restart firing — must not poll a fixed budget of `yield_now`s: that depends on the
//! scheduler and flakes under load. These helpers instead `await` the next matching
//! [`DomainEvent`] on a subscriber, so the test suspends until the effect actually happens and
//! the runtime is free to schedule the producing task whenever it is ready. One definition,
//! reused by every context's tests through [`crate::testing`].

use std::collections::HashSet;

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::events::DomainEvent;
use crate::ids::ProcessId;
use crate::process::ProcStatus;

/// Every event currently buffered for `rx`, drained synchronously — the events a mutation emitted
/// since subscribing. Unlike the waiters below this never suspends, so it suits a synchronous
/// mutation whose events are all published by the time it returns.
pub fn drain(rx: &mut broadcast::Receiver<DomainEvent>) -> Vec<DomainEvent> {
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    events
}

/// Awaits the next event satisfying `pred`, ignoring the rest, and returns it. A lagged
/// subscriber keeps waiting (it only missed events it did not ask for); a closed bus is a
/// test bug.
pub async fn next_matching(
    rx: &mut broadcast::Receiver<DomainEvent>,
    pred: impl Fn(&DomainEvent) -> bool,
) -> DomainEvent {
    loop {
        match rx.recv().await {
            Ok(event) if pred(&event) => return event,
            Ok(_) | Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => panic!("event bus closed before a matching event"),
        }
    }
}

/// Awaits the next [`DomainEvent::ProcessStatusChanged`], returning its target status and exit
/// code.
pub async fn next_change(rx: &mut broadcast::Receiver<DomainEvent>) -> (ProcStatus, Option<i32>) {
    match next_matching(rx, |e| {
        matches!(e, DomainEvent::ProcessStatusChanged { .. })
    })
    .await
    {
        DomainEvent::ProcessStatusChanged { to, exit_code, .. } => (to, exit_code),
        _ => unreachable!("next_matching only returns an event the predicate accepted"),
    }
}

/// Awaits the next status transition, returning the target status.
pub async fn next_to(rx: &mut broadcast::Receiver<DomainEvent>) -> ProcStatus {
    next_change(rx).await.0
}

/// Awaits until every id in `ids` has reached `target`, in any order.
pub async fn wait_all(
    rx: &mut broadcast::Receiver<DomainEvent>,
    ids: &[ProcessId],
    target: ProcStatus,
) {
    let mut remaining: HashSet<ProcessId> = ids.iter().copied().collect();
    while !remaining.is_empty() {
        match rx.recv().await {
            Ok(DomainEvent::ProcessStatusChanged { id, to, .. }) if to == target => {
                remaining.remove(&id);
            }
            Ok(_) | Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => panic!("event bus closed"),
        }
    }
}
