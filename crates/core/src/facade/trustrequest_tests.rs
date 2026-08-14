//! The round trip: a bound process asks, the user decides, and the requester learns — with the
//! grant pinned to exactly the command that was displayed.

use super::*;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::config::ProcessSpec;
use crate::coordination::{AgentMessageKind, MAX_PENDING_MESSAGES_PER_RECIPIENT};
use crate::facade::{CommandTrustRequest, Facade, RequestTrustError};
use crate::ids::{ProcessId, SessionId, TrustRequestId};
use crate::supervisor::Registration;
use crate::testing::{agent_registration, bound_session, facade_with_agent_tool, TEST_PEER_PGID};
use crate::trustrequest::TrustRequestOutcome;

const COMMAND: &str = "npm run build";
const REASON: &str = "the release build needs it";

fn spec(command: &str, working_dir: Option<&str>) -> ProcessSpec {
    ProcessSpec {
        command: command.into(),
        working_dir: working_dir.map(PathBuf::from),
        auto_start: false,
        auto_restart: false,
        restart_when_changed: Vec::new(),
        env: BTreeMap::new(),
    }
}

fn ask(command: &str, working_dir: Option<&str>) -> CommandTrustRequest {
    CommandTrustRequest {
        command: command.into(),
        working_dir: working_dir.map(PathBuf::from),
        env: BTreeMap::new(),
        label: None,
        reason: REASON.into(),
    }
}

/// A façade with one project, and a session bound to a live agent in it — the caller a real
/// request comes from.
fn facade_with_agent_caller() -> (Facade, ProjectId, ProcessId, SessionId) {
    let (facade, project) = facade_with_agent_tool();
    let agent = facade
        .supervisor()
        .register(agent_registration(project, "lead"));
    let session = bound_session(&facade, agent, TEST_PEER_PGID);
    (facade, project, agent, session)
}

/// The one open request in `project`, which every test below expects there to be exactly one of.
fn only_pending(facade: &Facade, project: ProjectId) -> TrustRequest {
    let mut open = facade.pending_trust_requests(project);
    assert_eq!(open.len(), 1, "expected exactly one open request");
    open.remove(0)
}

/// What `session` polls back for `id` — the channel every requester has.
fn status(facade: &Facade, session: SessionId, id: TrustRequestId) -> TrustRequestState {
    facade
        .scoped(session)
        .trust_request_status(id)
        .expect("poll the request")
}

/// Records a request over `session` and returns what the user would be shown.
fn open_request(facade: &Facade, session: SessionId, project: ProjectId) -> TrustRequest {
    facade
        .scoped(session)
        .request_command_trust(ask(COMMAND, None))
        .expect("record the request");
    only_pending(facade, project)
}

#[test]
fn a_request_for_an_already_trusted_variant_short_circuits_to_granted() {
    let (facade, project, _, session) = facade_with_agent_caller();
    facade
        .trust()
        .trust(project, &spec(COMMAND, None))
        .expect("the user already approved this command");

    let outcome = facade
        .scoped(session)
        .request_command_trust(ask(COMMAND, None))
        .expect("asking about a trusted command still succeeds");

    assert_eq!(
        outcome,
        TrustRequestOutcome {
            request_id: None,
            state: TrustRequestState::Granted,
        }
    );
    assert!(
        facade.pending_trust_requests(project).is_empty(),
        "a decision nobody needs to make must never become a prompt"
    );
}

#[test]
fn approving_a_request_makes_the_variant_startable() {
    let (facade, project, _, session) = facade_with_agent_caller();
    let configured = facade.supervisor().register(Registration::command(
        project,
        Path::new("/"),
        "Build",
        &spec(COMMAND, None),
    ));
    assert!(
        facade
            .process_view(configured)
            .expect("the registered command")
            .requires_trust,
        "the command starts out blocked, which is what approval has to clear"
    );
    let request = open_request(&facade, session, project);

    facade
        .approve_trust_request(request.id, &request.review.variant_hash)
        .expect("approve");

    assert!(facade
        .trust()
        .is_trusted(project, &spec(COMMAND, None))
        .expect("read trust"));
    assert!(
        !facade
            .process_view(configured)
            .expect("the registered command")
            .requires_trust,
        "the read model must clear too, or the user grants trust and the UI still blocks the start"
    );
}

#[test]
fn approving_a_variant_other_than_the_one_displayed_is_refused_and_trusts_nothing() {
    let (facade, project, _, session) = facade_with_agent_caller();
    let request = open_request(&facade, session, project);
    let other = spec("rm -rf /", None).variant_hash().to_hex();

    let refused = facade
        .approve_trust_request(request.id, &other)
        .expect_err("a hash that is not the displayed one must be refused");

    assert!(matches!(
        refused,
        ResolveTrustRequestError::ChangedSinceReview
    ));
    assert!(
        !facade
            .trust()
            .is_trusted(project, &spec(COMMAND, None))
            .expect("read trust"),
        "a refused approval must trust nothing at all"
    );
    assert_eq!(
        facade.pending_trust_requests(project).len(),
        1,
        "the request is still awaiting a decision the user has not validly made"
    );
}

#[test]
fn the_displayed_hash_and_the_granted_hash_come_from_one_raw_spec() {
    let (facade, project, _, session) = facade_with_agent_caller();
    let raw = spec(COMMAND, Some("web"));
    facade
        .scoped(session)
        .request_command_trust(ask(COMMAND, Some("web")))
        .expect("record the request");
    let request = only_pending(&facade, project);

    assert_eq!(
        request.review.variant_hash,
        raw.variant_hash().to_hex(),
        "the review must pin the variant of the working_dir as written, never one resolved \
         against the project root — resolving first displays one command and authorizes another"
    );

    facade
        .approve_trust_request(request.id, &request.review.variant_hash)
        .expect("approve");

    assert!(
        facade
            .trust()
            .is_trusted(project, &raw)
            .expect("read trust"),
        "a solo.yml command with the identical raw shape must now be startable"
    );
    assert!(
        !facade
            .trust()
            .is_trusted(project, &spec(COMMAND, Some("/anywhere/web")))
            .expect("read trust"),
        "and nothing else must be"
    );
}

#[test]
fn denying_a_request_trusts_nothing_and_resolves_it() {
    let (facade, project, _, session) = facade_with_agent_caller();
    let request = open_request(&facade, session, project);

    facade.deny_trust_request(request.id).expect("deny");

    assert!(!facade
        .trust()
        .is_trusted(project, &spec(COMMAND, None))
        .expect("read trust"));
    assert!(facade.pending_trust_requests(project).is_empty());
    assert_eq!(
        status(&facade, session, request.id),
        TrustRequestState::Denied
    );
}

#[test]
fn a_requester_cannot_assert_its_project_or_identity() {
    let (facade, project) = facade_with_agent_tool();
    let first = facade
        .supervisor()
        .register(agent_registration(project, "first"));
    let second = facade
        .supervisor()
        .register(agent_registration(project, "second"));
    // Distinct peer groups: the bind is authentic only when the caller's connecting peer runs
    // in the process's own group, so two callers cannot share one.
    let first_session = bound_session(&facade, first, TEST_PEER_PGID);
    let second_session = bound_session(&facade, second, TEST_PEER_PGID + 1);

    facade
        .scoped(first_session)
        .request_command_trust(ask("npm run build", None))
        .expect("record");
    facade
        .scoped(second_session)
        .request_command_trust(ask("npm run test", None))
        .expect("record");

    let attributed: Vec<_> = facade
        .pending_trust_requests(project)
        .into_iter()
        .map(|request| (request.requested_by, request.project))
        .collect();
    assert_eq!(
        attributed,
        vec![(first, project), (second, project)],
        "attribution and scope come from the authenticated session; the request carries no \
         field either could be asserted through"
    );
}

#[test]
fn an_unbound_caller_cannot_open_a_request() {
    let (facade, _) = facade_with_agent_tool();
    let session = facade.open_session(crate::PeerCredentials::in_group(TEST_PEER_PGID));

    let refused = facade
        .scoped(session)
        .request_command_trust(ask(COMMAND, None))
        .expect_err("a caller with no identity has nobody to attribute a request to");

    assert!(matches!(refused, RequestTrustError::NoBoundProcess));
}

#[test]
fn an_agent_requester_receives_the_decision_in_its_mailbox() {
    let (facade, project, _, session) = facade_with_agent_caller();
    let request = open_request(&facade, session, project);

    facade
        .approve_trust_request(request.id, &request.review.variant_hash)
        .expect("approve");

    let inbox = facade
        .scoped(session)
        .agent_message_list()
        .expect("read the requester's inbox");
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].message.kind, AgentMessageKind::TrustDecision);
    let message = inbox[0].message.id;
    facade
        .scoped(session)
        .agent_message_acknowledge(message)
        .expect("acknowledge");
    assert!(facade
        .scoped(session)
        .agent_message_list()
        .expect("read the requester's inbox")
        .is_empty());
}

#[test]
fn a_non_agent_requester_still_learns_the_outcome_by_status() {
    let (facade, project) = facade_with_agent_tool();
    let command = facade.supervisor().register(Registration::command(
        project,
        Path::new("/"),
        "Runner",
        &spec("sleep 60", None),
    ));
    let session = bound_session(&facade, command, TEST_PEER_PGID);
    let request = open_request(&facade, session, project);

    facade
        .approve_trust_request(request.id, &request.review.variant_hash)
        .expect("approve");

    assert!(
        facade.scoped(session).agent_message_list().is_err(),
        "a command process has no mailbox at all, which is why status is the channel that counts"
    );
    assert_eq!(
        status(&facade, session, request.id),
        TrustRequestState::Granted
    );
}

#[test]
fn a_full_mailbox_cannot_undo_a_granted_trust() {
    let (facade, project, agent, session) = facade_with_agent_caller();
    let request = open_request(&facade, session, project);
    for _ in 0..MAX_PENDING_MESSAGES_PER_RECIPIENT {
        facade
            .scoped(session)
            .agent_message_send(agent, "filling the inbox".into(), None)
            .expect("fill the requester's inbox to its ceiling");
    }

    facade
        .approve_trust_request(request.id, &request.review.variant_hash)
        .expect("a full inbox is not a reason to un-decide what the user decided");

    assert!(facade
        .trust()
        .is_trusted(project, &spec(COMMAND, None))
        .expect("read trust"));
    assert_eq!(
        status(&facade, session, request.id),
        TrustRequestState::Granted
    );
}

#[test]
fn a_grant_records_who_asked_for_it_and_can_be_taken_back() {
    let (facade, project, agent, session) = facade_with_agent_caller();
    let request = open_request(&facade, session, project);
    facade
        .approve_trust_request(request.id, &request.review.variant_hash)
        .expect("approve");

    let grants = facade.list_trusted_commands(project).expect("list grants");
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].requested_by, Some(agent));
    assert_eq!(grants[0].reason.as_deref(), Some(REASON));

    facade
        .revoke_command_trust(project, &grants[0].variant_hash)
        .expect("revoke");

    assert!(
        !facade
            .trust()
            .is_trusted(project, &spec(COMMAND, None))
            .expect("read trust"),
        "if an agent can cause a grant, the user must be able to take it back"
    );
    assert!(facade
        .list_trusted_commands(project)
        .expect("list grants")
        .is_empty());
}
