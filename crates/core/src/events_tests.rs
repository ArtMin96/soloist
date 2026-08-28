//! Behavioural tests for the event bus, and for what the event stream looks like on the wire.
//!
//! The surfaces mirror [`DomainEvent`] by hand — a TypeScript union on the far side of an IPC
//! boundary that carries JSON — so the spelling of a payload is a contract between two files
//! nothing else compares. The serialization tests pin the spellings a mirror has to match exactly.

use std::collections::BTreeMap;

use crate::ids::{ProcessId, ProjectId};
use crate::watch::{WatchError, WatchLimit, WatchPurpose};

use super::{DomainEvent, EventBus};

#[tokio::test]
async fn published_events_reach_a_subscriber() {
    let bus = EventBus::new(16);
    let mut rx = bus.subscribe();
    let id = ProcessId::next();
    bus.publish(DomainEvent::ProcessRemoved { id });
    match rx.recv().await {
        Ok(DomainEvent::ProcessRemoved { id: got }) => assert_eq!(got, id),
        other => panic!("unexpected event: {other:?}"),
    }
}

/// A watch limit announcement for project 1, as it reaches a surface.
fn limit_wire(limits: BTreeMap<WatchPurpose, WatchLimit>) -> String {
    let event = DomainEvent::WatchLimitChanged {
        project: ProjectId::from_raw(1),
        limits,
    };
    serde_json::to_string(&event).expect("a domain event serializes")
}

#[test]
fn a_watch_limit_carries_one_reason_per_limited_purpose() {
    assert_eq!(
        limit_wire(BTreeMap::from([(
            WatchPurpose::GitStatus,
            WatchLimit::Refused(WatchError::BudgetExhausted)
        )])),
        r#"{"type":"WatchLimitChanged","project":1,"limits":{"git_status":{"refused":"budget_exhausted"}}}"#,
        "one refused watch is one keyed reason, as a newtype variant",
    );

    assert_eq!(
        limit_wire(BTreeMap::from([
            (
                WatchPurpose::Restarts,
                WatchLimit::Refused(WatchError::Unwatchable)
            ),
            (
                WatchPurpose::GitStatus,
                WatchLimit::Refused(WatchError::Unavailable)
            ),
        ])),
        r#"{"type":"WatchLimitChanged","project":1,"limits":{"restarts":{"refused":"unwatchable"},"git_status":{"refused":"unavailable"}}}"#,
        "both purposes and every reason spell the same on the wire as in the mirror",
    );

    assert_eq!(
        limit_wire(BTreeMap::new()),
        r#"{"type":"WatchLimitChanged","project":1,"limits":{}}"#,
        "a project watched again arrives as an empty set, not as an absent field",
    );

    assert_eq!(
        limit_wire(BTreeMap::from([(
            WatchPurpose::GitStatus,
            WatchLimit::Degraded
        )])),
        r#"{"type":"WatchLimitChanged","project":1,"limits":{"git_status":"degraded"}}"#,
        "a degradation is a unit variant, so it spells as a bare string rather than an object",
    );
}
