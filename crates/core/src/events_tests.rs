//! Behavioural tests for the event bus, and for what the event stream looks like on the wire.
//!
//! The surfaces mirror [`DomainEvent`] by hand — a TypeScript union on the far side of an IPC
//! boundary that carries JSON — so the spelling of a payload is a contract between two files
//! nothing else compares. The serialization tests pin the spellings a mirror has to match exactly.

use std::collections::BTreeMap;

use crate::ids::{ProcessId, ProjectId};
use crate::watch::{WatchError, WatchPurpose};

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

/// A watch refusal announcement for project 1, as it reaches a surface.
fn refusal_wire(refusals: BTreeMap<WatchPurpose, WatchError>) -> String {
    let event = DomainEvent::WatchRefusalChanged {
        project: ProjectId::from_raw(1),
        refusals,
    };
    serde_json::to_string(&event).expect("a domain event serializes")
}

#[test]
fn a_watch_refusal_carries_one_reason_per_refused_purpose() {
    assert_eq!(
        refusal_wire(BTreeMap::from([(
            WatchPurpose::GitStatus,
            WatchError::BudgetExhausted
        )])),
        r#"{"type":"WatchRefusalChanged","project":1,"refusals":{"git_status":"budget_exhausted"}}"#,
        "one refused watch is one keyed reason",
    );

    assert_eq!(
        refusal_wire(BTreeMap::from([
            (WatchPurpose::Restarts, WatchError::Unwatchable),
            (WatchPurpose::GitStatus, WatchError::Unavailable),
        ])),
        r#"{"type":"WatchRefusalChanged","project":1,"refusals":{"restarts":"unwatchable","git_status":"unavailable"}}"#,
        "both purposes and every reason spell the same on the wire as in the mirror",
    );

    assert_eq!(
        refusal_wire(BTreeMap::new()),
        r#"{"type":"WatchRefusalChanged","project":1,"refusals":{}}"#,
        "a project watched again arrives as an empty set, not as an absent field",
    );
}
