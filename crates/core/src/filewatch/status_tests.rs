//! Behavioural tests for [`WatchStatus`], the single voice that tells the surfaces which of a
//! project's watches the OS is refusing. Each drives it exactly as a reactor's re-sync does and
//! asserts on what reached the bus, since that is all a surface ever sees.

use std::collections::BTreeMap;

use tokio::sync::broadcast;

use crate::events::{DomainEvent, EventBus};
use crate::ids::ProjectId;
use crate::testing::drain;
use crate::watch::{WatchError, WatchLimit, WatchOutcome, WatchPurpose};

use super::WatchStatus;

const ONE: ProjectId = ProjectId::from_raw(1);
const TWO: ProjectId = ProjectId::from_raw(2);

/// The limit sets announced since the last call, as `(project, limits)`.
fn announced(
    rx: &mut broadcast::Receiver<DomainEvent>,
) -> Vec<(ProjectId, BTreeMap<WatchPurpose, WatchLimit>)> {
    drain(rx)
        .into_iter()
        .filter_map(|event| match event {
            DomainEvent::WatchLimitChanged { project, limits } => Some((project, limits)),
            _ => None,
        })
        .collect()
}

/// What a purpose reports for a project the OS turned its watch down for.
fn refused(project: ProjectId, reason: WatchError) -> WatchOutcome {
    WatchOutcome {
        project,
        limit: Some(WatchLimit::Refused(reason)),
    }
}

/// What a purpose reports for a project degraded to its essential watches.
fn degraded(project: ProjectId) -> WatchOutcome {
    WatchOutcome {
        project,
        limit: Some(WatchLimit::Degraded),
    }
}

/// What a purpose reports for a project whose watch it holds without restriction.
fn watched(project: ProjectId) -> WatchOutcome {
    WatchOutcome {
        project,
        limit: None,
    }
}

#[test]
fn a_refusal_is_announced_once_however_often_the_watch_is_retried() {
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();
    let status = WatchStatus::new(bus);

    status.resynced(
        WatchPurpose::Restarts,
        &[refused(ONE, WatchError::BudgetExhausted)],
    );
    assert_eq!(
        announced(&mut rx),
        vec![(
            ONE,
            BTreeMap::from([(
                WatchPurpose::Restarts,
                WatchLimit::Refused(WatchError::BudgetExhausted)
            )])
        )],
        "the first refusal reaches the surfaces",
    );

    // The reactors ask again on every re-sync, so the same refusal arrives over and over. Saying
    // it again each time would repeat one sentence for as long as the condition lasted.
    for _ in 0..3 {
        status.resynced(
            WatchPurpose::Restarts,
            &[refused(ONE, WatchError::BudgetExhausted)],
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
        &[refused(ONE, WatchError::Unwatchable)],
    );
    let _ = announced(&mut rx);

    status.resynced(WatchPurpose::Restarts, &[watched(ONE)]);
    assert_eq!(
        announced(&mut rx),
        vec![(ONE, BTreeMap::new())],
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
        &[refused(ONE, WatchError::Unwatchable)],
    );
    let _ = announced(&mut rx);

    // A directory that was missing and is now there, on a machine that has since run out of
    // watches: the same project is still refused, but for a reason the user acts on differently.
    status.resynced(
        WatchPurpose::Restarts,
        &[refused(ONE, WatchError::BudgetExhausted)],
    );
    assert_eq!(
        announced(&mut rx),
        vec![(
            ONE,
            BTreeMap::from([(
                WatchPurpose::Restarts,
                WatchLimit::Refused(WatchError::BudgetExhausted)
            )])
        )],
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
        &[refused(ONE, WatchError::BudgetExhausted)],
    );
    let _ = announced(&mut rx);

    // The restart policy's own watch over the same tree was granted. The project's git status has
    // still stopped refreshing, so nothing is withdrawn.
    status.resynced(WatchPurpose::Restarts, &[watched(ONE)]);
    assert!(
        announced(&mut rx).is_empty(),
        "a refusal one purpose still meets survives another purpose's success",
    );

    status.resynced(WatchPurpose::GitStatus, &[watched(ONE)]);
    assert_eq!(
        announced(&mut rx),
        vec![(ONE, BTreeMap::new())],
        "only the last refusal clearing takes the project out of it",
    );
}

#[test]
fn a_project_refused_both_watches_is_announced_with_both_reasons() {
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();
    let status = WatchStatus::new(bus);

    // The two watches over the same tree can fail differently: the restart policy's root is gone,
    // while the git rail's is simply past what the OS will grant. Both consequences follow, and a
    // surface handed one of the two reasons could only name one of them — or, worse, name the
    // wrong one.
    status.resynced(
        WatchPurpose::Restarts,
        &[refused(ONE, WatchError::Unwatchable)],
    );
    let _ = announced(&mut rx);
    status.resynced(
        WatchPurpose::GitStatus,
        &[refused(ONE, WatchError::BudgetExhausted)],
    );
    assert_eq!(
        announced(&mut rx),
        vec![(
            ONE,
            BTreeMap::from([
                (
                    WatchPurpose::Restarts,
                    WatchLimit::Refused(WatchError::Unwatchable)
                ),
                (
                    WatchPurpose::GitStatus,
                    WatchLimit::Refused(WatchError::BudgetExhausted)
                ),
            ])
        )],
        "each refused watch is named with the reason it met",
    );

    // Both reactors ask again, and both are turned down the same way again. Nothing about the
    // project has moved, so there is nothing to say.
    status.resynced(
        WatchPurpose::Restarts,
        &[refused(ONE, WatchError::Unwatchable)],
    );
    status.resynced(
        WatchPurpose::GitStatus,
        &[refused(ONE, WatchError::BudgetExhausted)],
    );
    assert!(
        announced(&mut rx).is_empty(),
        "two standing refusals that have not moved are not announced again",
    );
}

#[test]
fn a_project_only_one_purpose_watches_is_announced_with_only_that_purposes_refusal() {
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();
    let status = WatchStatus::new(bus);

    // A project that declares no `restart_when_changed` never enters the restart policy's outcomes
    // at all, while the git rail watches every open project. Its refusal must not claim that
    // restart-on-change stopped, because nothing there was ever asking for it.
    status.resynced(
        WatchPurpose::Restarts,
        &[refused(TWO, WatchError::Unwatchable)],
    );
    let _ = announced(&mut rx);

    status.resynced(
        WatchPurpose::GitStatus,
        &[
            refused(ONE, WatchError::BudgetExhausted),
            refused(TWO, WatchError::BudgetExhausted),
        ],
    );
    assert_eq!(
        announced(&mut rx),
        vec![
            (
                ONE,
                BTreeMap::from([(
                    WatchPurpose::GitStatus,
                    WatchLimit::Refused(WatchError::BudgetExhausted)
                )])
            ),
            (
                TWO,
                BTreeMap::from([
                    (
                        WatchPurpose::Restarts,
                        WatchLimit::Refused(WatchError::Unwatchable)
                    ),
                    (
                        WatchPurpose::GitStatus,
                        WatchLimit::Refused(WatchError::BudgetExhausted)
                    ),
                ])
            ),
        ],
        "a project is told about the watches it actually asked for, and no others",
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
            refused(ONE, WatchError::BudgetExhausted),
            refused(TWO, WatchError::BudgetExhausted),
        ],
    );
    let _ = announced(&mut rx);

    // The project's last `restart_when_changed` command went away, so nothing is asking for that
    // watch any more — leaving the refusal standing would report a failure nobody is meeting.
    status.resynced(
        WatchPurpose::Restarts,
        &[refused(TWO, WatchError::BudgetExhausted)],
    );
    assert_eq!(
        announced(&mut rx),
        vec![(ONE, BTreeMap::new())],
        "the project nobody watches any more is no longer reported refused",
    );
}

#[test]
fn a_refusal_and_a_degradation_on_the_same_project_are_announced_together() {
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();
    let status = WatchStatus::new(bus);

    // The two purposes can meet different limits on the same tree: the restart policy's root is
    // gone outright, while the git rail's tree is merely too large for its share of the budget.
    // A surface handed only the whole-set comparison, without both purposes present, could not
    // tell the two consequences apart.
    status.resynced(
        WatchPurpose::Restarts,
        &[refused(ONE, WatchError::Unwatchable)],
    );
    let _ = announced(&mut rx);
    status.resynced(WatchPurpose::GitStatus, &[degraded(ONE)]);

    assert_eq!(
        announced(&mut rx),
        vec![(
            ONE,
            BTreeMap::from([
                (
                    WatchPurpose::Restarts,
                    WatchLimit::Refused(WatchError::Unwatchable)
                ),
                (WatchPurpose::GitStatus, WatchLimit::Degraded),
            ])
        )],
        "one announcement carries both purposes' limits, refused and degraded alike",
    );
}
