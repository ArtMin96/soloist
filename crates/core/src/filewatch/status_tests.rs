//! Behavioural tests for [`WatchStatus`], the single voice that tells the surfaces which projects
//! the OS is refusing to watch. Each drives it exactly as a reactor's re-sync does and asserts on
//! what reached the bus, since that is all a surface ever sees.

use tokio::sync::broadcast;

use crate::events::{DomainEvent, EventBus};
use crate::ids::ProjectId;
use crate::testing::drain;
use crate::watch::WatchError;

use super::{WatchPurpose, WatchStatus};

const ONE: ProjectId = ProjectId::from_raw(1);
const TWO: ProjectId = ProjectId::from_raw(2);

/// The refusals announced since the last call, as `(project, refusal)`.
fn announced(rx: &mut broadcast::Receiver<DomainEvent>) -> Vec<(ProjectId, Option<WatchError>)> {
    drain(rx)
        .into_iter()
        .filter_map(|event| match event {
            DomainEvent::WatchRefusalChanged { project, refusal } => Some((project, refusal)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_refusal_is_announced_once_however_often_the_watch_is_retried() {
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();
    let status = WatchStatus::new(bus);

    status.resynced(
        WatchPurpose::Restarts,
        &[(ONE, Some(WatchError::BudgetExhausted))],
    );
    assert_eq!(
        announced(&mut rx),
        vec![(ONE, Some(WatchError::BudgetExhausted))],
        "the first refusal reaches the surfaces",
    );

    // The reactors ask again on every re-sync, so the same refusal arrives over and over. Saying
    // it again each time would repeat one sentence for as long as the condition lasted.
    for _ in 0..3 {
        status.resynced(
            WatchPurpose::Restarts,
            &[(ONE, Some(WatchError::BudgetExhausted))],
        );
    }
    assert!(
        announced(&mut rx).is_empty(),
        "a refusal that has not changed is not announced again",
    );
}

#[test]
fn a_watch_established_after_a_refusal_announces_the_clear() {
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();
    let status = WatchStatus::new(bus);

    status.resynced(
        WatchPurpose::Restarts,
        &[(ONE, Some(WatchError::Unwatchable))],
    );
    let _ = announced(&mut rx);

    status.resynced(WatchPurpose::Restarts, &[(ONE, None)]);
    assert_eq!(
        announced(&mut rx),
        vec![(ONE, None)],
        "the surfaces are told the project is watched again",
    );
}

#[test]
fn a_refusal_that_changes_reason_is_announced_again() {
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();
    let status = WatchStatus::new(bus);

    status.resynced(
        WatchPurpose::Restarts,
        &[(ONE, Some(WatchError::Unwatchable))],
    );
    let _ = announced(&mut rx);

    // A directory that was missing and is now there, on a machine that has since run out of
    // watches: the same project is still degraded, but for a reason the user acts on differently.
    status.resynced(
        WatchPurpose::Restarts,
        &[(ONE, Some(WatchError::BudgetExhausted))],
    );
    assert_eq!(
        announced(&mut rx),
        vec![(ONE, Some(WatchError::BudgetExhausted))],
        "a refusal for a different reason is worth saying",
    );
}

#[test]
fn one_purposes_watch_does_not_erase_another_purposes_refusal() {
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();
    let status = WatchStatus::new(bus);

    status.resynced(
        WatchPurpose::GitStatus,
        &[(ONE, Some(WatchError::BudgetExhausted))],
    );
    let _ = announced(&mut rx);

    // The restart policy's own watch over the same tree was granted. The project is still
    // degraded — its git status has stopped refreshing — so nothing is withdrawn.
    status.resynced(WatchPurpose::Restarts, &[(ONE, None)]);
    assert!(
        announced(&mut rx).is_empty(),
        "a project stays reported degraded while either watch is refused",
    );

    status.resynced(WatchPurpose::GitStatus, &[(ONE, None)]);
    assert_eq!(
        announced(&mut rx),
        vec![(ONE, None)],
        "only the last refusal clearing takes the project out of it",
    );
}

#[test]
fn a_project_a_purpose_stops_watching_has_its_refusal_withdrawn() {
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();
    let status = WatchStatus::new(bus);

    status.resynced(
        WatchPurpose::Restarts,
        &[
            (ONE, Some(WatchError::BudgetExhausted)),
            (TWO, Some(WatchError::BudgetExhausted)),
        ],
    );
    let _ = announced(&mut rx);

    // The project's last `restart_when_changed` command went away, so nothing is asking for that
    // watch any more — leaving the refusal standing would report a failure nobody is meeting.
    status.resynced(
        WatchPurpose::Restarts,
        &[(TWO, Some(WatchError::BudgetExhausted))],
    );
    assert_eq!(
        announced(&mut rx),
        vec![(ONE, None)],
        "the project nobody watches any more is no longer reported refused",
    );
}
