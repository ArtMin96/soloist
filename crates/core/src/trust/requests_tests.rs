//! The pending-request set's own rules: one prompt per command however many ask, a ceiling that
//! refuses rather than evicting, and a request that ages out on its own without a timer.

use super::*;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use crate::testing::MockClock;
use crate::trustrequest::MAX_TRUST_REQUEST_REASON_BYTES;

const EVENT_BUFFER: usize = 512;

fn requests(clock: Arc<MockClock>) -> TrustRequests {
    TrustRequests::new(clock, EventBus::new(EVENT_BUFFER))
}

fn spec(command: &str) -> ProcessSpec {
    ProcessSpec {
        command: command.into(),
        working_dir: None,
        auto_start: false,
        auto_restart: false,
        restart_when_changed: Vec::new(),
        env: BTreeMap::new(),
    }
}

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
        spec: spec(command),
        reason: "the release build needs it".into(),
    }
}

#[test]
fn two_processes_requesting_one_variant_produce_one_pending_request() {
    let project = ProjectId::from_raw(1);
    let requests = requests(Arc::new(MockClock::new()));

    let first = requests
        .record(submission(project, ProcessId::next(), "npm run build"))
        .expect("record the first request");
    let second = requests
        .record(submission(project, ProcessId::next(), "npm run build"))
        .expect("record the second request");

    assert_eq!(
        first, second,
        "a second ask for the same variant must join the open request, not raise a second prompt"
    );
    assert_eq!(
        requests.pending(project).len(),
        1,
        "the user must be asked once however many processes want the command"
    );
}

#[test]
fn a_different_variant_from_the_same_process_is_its_own_request() {
    let project = ProjectId::from_raw(1);
    let requester = ProcessId::next();
    let requests = requests(Arc::new(MockClock::new()));

    let first = requests
        .record(submission(project, requester, "npm run build"))
        .expect("record");
    let second = requests
        .record(submission(project, requester, "npm run test"))
        .expect("record");

    assert_ne!(first, second);
    assert_eq!(requests.pending(project).len(), 2);
}

#[test]
fn the_project_ceiling_refuses_without_dropping_a_queued_request() {
    let project = ProjectId::from_raw(1);
    let requester = ProcessId::next();
    let requests = requests(Arc::new(MockClock::new()));
    let queued: Vec<_> = (0..MAX_PENDING_TRUST_REQUESTS_PER_PROJECT)
        .map(|index| {
            requests
                .record(submission(project, requester, &format!("build {index}")))
                .expect("record up to the ceiling")
        })
        .collect();

    let refused = requests
        .record(submission(project, requester, "one too many"))
        .expect_err("the ceiling must refuse");

    assert_eq!(refused, TrustRequestCapacityError::ProjectQueueFull);
    let still_open: Vec<_> = requests
        .pending(project)
        .into_iter()
        .map(|request| request.id)
        .collect();
    assert_eq!(
        still_open, queued,
        "refusing must never make room by dropping a decision the user has not made yet"
    );
}

#[test]
fn the_global_ceiling_refuses_across_projects() {
    let requester = ProcessId::next();
    let requests = requests(Arc::new(MockClock::new()));
    let projects = MAX_PENDING_TRUST_REQUESTS / MAX_PENDING_TRUST_REQUESTS_PER_PROJECT;
    for project in 0..projects {
        for index in 0..MAX_PENDING_TRUST_REQUESTS_PER_PROJECT {
            requests
                .record(submission(
                    ProjectId::from_raw(project as u64 + 1),
                    requester,
                    &format!("build {project}-{index}"),
                ))
                .expect("record up to the global ceiling");
        }
    }

    let refused = requests
        .record(submission(
            ProjectId::from_raw(projects as u64 + 1),
            requester,
            "one too many",
        ))
        .expect_err("the global ceiling must refuse");

    assert_eq!(refused, TrustRequestCapacityError::GlobalQueueFull);
}

#[test]
fn an_expired_request_reads_back_as_expired_and_frees_its_slot() {
    let project = ProjectId::from_raw(1);
    let requester = ProcessId::next();
    let clock = Arc::new(MockClock::new());
    let requests = requests(clock.clone());
    let first = requests
        .record(submission(project, requester, "npm run build"))
        .expect("record");

    clock.advance(TRUST_REQUEST_TTL + Duration::from_secs(1));

    assert_eq!(
        requests.status(project, first),
        Some(TrustRequestState::Expired)
    );
    assert!(
        requests.pending(project).is_empty(),
        "an aged-out request must stop occupying the user's queue"
    );
    let second = requests
        .record(submission(project, requester, "npm run build"))
        .expect("the freed slot must accept a fresh ask for the same command");
    assert_ne!(
        first, second,
        "a request that expired is over; asking again opens a new one"
    );
}

#[test]
fn an_oversized_reason_is_refused() {
    let project = ProjectId::from_raw(1);
    let requester = ProcessId::next();
    let requests = requests(Arc::new(MockClock::new()));
    let mut oversized = submission(project, requester, "npm run build");
    oversized.reason = "r".repeat(MAX_TRUST_REQUEST_REASON_BYTES + 1);

    let refused = requests.record(oversized).expect_err("the cap must refuse");

    assert_eq!(refused, TrustRequestCapacityError::ReasonTooLarge);
    assert!(requests.pending(project).is_empty());
}

#[test]
fn a_status_read_cannot_see_another_projects_request() {
    let mine = ProjectId::from_raw(1);
    let theirs = ProjectId::from_raw(2);
    let requests = requests(Arc::new(MockClock::new()));
    let elsewhere = requests
        .record(submission(theirs, ProcessId::next(), "npm run build"))
        .expect("record");

    assert_eq!(requests.status(mine, elsewhere), None);
    assert_eq!(
        requests.status(theirs, elsewhere),
        Some(TrustRequestState::Pending)
    );
}

#[test]
fn a_resolved_request_reads_back_its_outcome() {
    let project = ProjectId::from_raw(1);
    let requests = requests(Arc::new(MockClock::new()));
    let id = requests
        .record(submission(project, ProcessId::next(), "npm run build"))
        .expect("record");

    requests.resolve(id, TrustRequestState::Denied);

    assert_eq!(
        requests.status(project, id),
        Some(TrustRequestState::Denied)
    );
    assert!(requests.pending(project).is_empty());
}
