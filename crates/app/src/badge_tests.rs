//! What the badge decides to draw. The call itself goes to the window system and cannot be
//! asserted headlessly, so these drive the reactor over a real event stream with the sink and the
//! state read injected, and assert the value it decided on rather than that it called anything.

use std::sync::{Arc, Mutex};

use soloist_core::attention::AttentionKind;
use soloist_core::ids::ProcessId;
use soloist_core::notify::ProcessAttention;
use tokio::sync::broadcast;

use super::*;

const WEB: ProcessId = ProcessId::from_raw(1);

fn away() -> Presence {
    Presence {
        focused: false,
        viewing: None,
    }
}

fn at_the_window() -> Presence {
    Presence {
        focused: true,
        viewing: Some(WEB),
    }
}

/// A snapshot of one process holding `alerts` unread — the shape the registry really produces,
/// where the total counts alerts rather than processes.
fn waiting(alerts: usize) -> AttentionSnapshot {
    AttentionSnapshot {
        processes: vec![ProcessAttention {
            process: WEB,
            kinds: vec![AttentionKind::Crashed; alerts],
        }],
        total: alerts,
    }
}

/// Runs the reactor over `events` until the bus closes, reporting every count it decided on.
///
/// The events are queued and the sender dropped before the reactor reads, so it drains them and
/// ends on its own — the assertions never wait on a timer.
async fn badges_for(
    events: Vec<DomainEvent>,
    presence: Presence,
    snapshot: AttentionSnapshot,
) -> Vec<Option<i64>> {
    let (tx, rx) = broadcast::channel(16);
    for event in events {
        tx.send(event).expect("the receiver is still listening");
    }
    drop(tx);

    let decided = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&decided);
    run(
        rx,
        move || (presence, snapshot.clone()),
        move |count| recorded.lock().expect("no test panicked").push(count),
    )
    .await;

    Arc::into_inner(decided)
        .expect("the reactor dropped its sink")
        .into_inner()
        .expect("no test panicked")
}

#[tokio::test]
async fn badge_count_follows_the_snapshot_total() {
    assert_eq!(
        badges_for(vec![DomainEvent::AttentionChanged], away(), waiting(3)).await,
        vec![Some(3)],
    );
}

#[tokio::test]
async fn badge_caps_at_99() {
    // The snapshot itself is never truncated — 150 is the truth the in-window count renders. Only
    // the dock, which has room for two digits, sees the cap.
    assert_eq!(
        badges_for(vec![DomainEvent::AttentionChanged], away(), waiting(150)).await,
        vec![Some(99)],
    );
}

#[tokio::test]
async fn empty_snapshot_removes_the_badge() {
    // `None` removes the badge; `Some(0)` would leave some docks drawing a nought.
    assert_eq!(
        badges_for(
            vec![DomainEvent::AttentionChanged],
            away(),
            AttentionSnapshot::default(),
        )
        .await,
        vec![None],
    );
}

#[tokio::test]
async fn the_badge_clears_while_the_user_is_at_the_window() {
    // The unread survives the arrival — that is what keeps the in-window markers findable. The
    // badge simply stops drawing it, because "while you were away" is over.
    assert_eq!(
        badges_for(
            vec![DomainEvent::AttentionChanged],
            at_the_window(),
            waiting(3),
        )
        .await,
        vec![None],
    );
}

#[tokio::test]
async fn walking_away_from_unread_raises_the_badge_with_no_new_alert() {
    // Nothing about the unread changed — only where the user is. Without presence on the bus the
    // dock would stay bare for someone who walked away from three waiting alerts.
    assert_eq!(
        badges_for(vec![DomainEvent::PresenceChanged], away(), waiting(3)).await,
        vec![Some(3)],
    );
}

#[tokio::test]
async fn an_event_that_changes_neither_leaves_the_badge_alone() {
    assert_eq!(
        badges_for(
            vec![DomainEvent::ProcessRemoved { id: WEB }],
            away(),
            waiting(3),
        )
        .await,
        Vec::new(),
    );
}

#[tokio::test]
async fn falling_behind_the_bus_still_leaves_the_badge_current() {
    // A dropped event must not strand a stale count: the total is re-read, never folded, so
    // catching up is the same recompute a change asks for.
    let (tx, rx) = broadcast::channel(1);
    for _ in 0..3 {
        tx.send(DomainEvent::AttentionChanged)
            .expect("the receiver is still listening");
    }
    drop(tx);

    let decided = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&decided);
    run(
        rx,
        move || (away(), waiting(2)),
        move |count| recorded.lock().expect("no test panicked").push(count),
    )
    .await;

    let counts = decided.lock().expect("no test panicked");
    assert!(
        !counts.is_empty(),
        "the lag was skipped instead of caught up"
    );
    assert!(counts.iter().all(|count| *count == Some(2)));
}

#[tokio::test]
async fn the_reactor_ends_when_the_bus_closes() {
    // Every other test here awaits `run` to completion, so a reactor that outlived its bus would
    // hang the suite rather than fail it. This says so out loud.
    let (tx, rx) = broadcast::channel::<DomainEvent>(1);
    drop(tx);

    run(rx, || (away(), waiting(1)), |_| {}).await;
}
