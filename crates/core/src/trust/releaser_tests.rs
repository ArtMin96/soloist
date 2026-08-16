//! A closing process takes its unanswered requests with it, through the deterministic close hook
//! rather than the event bus.
//!
//! The distinction the name carries is the point. Cleanup wired to
//! [`ProcessRemoved`](crate::events::DomainEvent::ProcessRemoved) through a reactor passes a
//! happy-path test exactly like this one and fails only when the bus lags — at which moment the
//! user is being asked to authorize a command for a process that is already gone.

use super::*;

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::config::ProcessSpec;
use crate::events::{DomainEvent, EventBus};
use crate::ids::ProjectId;
use crate::testing::MockClock;
use crate::trust::{TrustRequestSubmission, TrustRequests};
use crate::trustrequest::TrustRequestState;

const EVENT_BUFFER: usize = 512;

fn submission(
    project: ProjectId,
    requested_by: ProcessId,
    command: &str,
) -> TrustRequestSubmission {
    TrustRequestSubmission {
        project,
        requested_by,
        requested_by_label: "asker".into(),
        name: "job".into(),
        spec: ProcessSpec {
            command: command.into(),
            working_dir: None,
            auto_start: false,
            auto_restart: false,
            restart_when_changed: Vec::new(),
            env: BTreeMap::new(),
        },
        reason: "the release build needs it".into(),
    }
}

#[test]
fn a_closing_requesters_pending_request_is_dropped_and_announced() {
    let project = ProjectId::from_raw(1);
    let leaving = ProcessId::next();
    let staying = ProcessId::next();
    let bus = EventBus::new(EVENT_BUFFER);
    let requests = TrustRequests::new(Arc::new(MockClock::new()), bus.clone());
    let dropped = requests
        .record(submission(project, leaving, "npm run build"))
        .expect("record the closing process's request");
    let kept = requests
        .record(submission(project, staying, "npm run test"))
        .expect("record the surviving process's request");
    let mut events = bus.subscribe();

    requests.release_all(leaving);

    assert_eq!(
        requests.status(project, dropped),
        Some(TrustRequestState::Withdrawn),
        "a request nobody is asking for any more must not stay pending"
    );
    assert_eq!(
        requests.status(project, kept),
        Some(TrustRequestState::Pending),
        "another process's request is not the closing one's to drop"
    );
    let announced = events.try_recv().expect("the drop must be announced");
    match announced {
        DomainEvent::TrustRequestResolved { id, state, .. } => {
            assert_eq!(id, dropped);
            assert_eq!(state, TrustRequestState::Withdrawn);
        }
        other => panic!("expected a resolution for the dropped request, got {other:?}"),
    }
}
