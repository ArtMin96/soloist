use super::*;
use crate::testing::{
    agent_registration, authentic_session, facade_with_agent_tool, TEST_PEER_PGID,
};

fn agent(facade: &crate::facade::Facade, project: crate::ids::ProjectId, label: &str) -> ProcessId {
    facade
        .supervisor()
        .register(agent_registration(project, label))
}

#[test]
fn direct_messages_require_a_live_related_recipient_in_the_same_project() {
    let (facade, project) = facade_with_agent_tool();
    let lead = agent(&facade, project, "lead");
    let child = agent(&facade, project, "child");
    let unrelated = agent(&facade, project, "unrelated");
    let foreign = agent(&facade, crate::ids::ProjectId::next(), "foreign");
    facade.lineage.record(child, lead);
    let session = authentic_session(&facade, lead, TEST_PEER_PGID);
    facade
        .scoped(session)
        .bind_session_process(lead)
        .expect("bind lead");

    let delivery = facade
        .scoped(session)
        .agent_message_send(child, "work".to_owned(), None)
        .expect("related delivery");
    assert_eq!(delivery.message.recipient, child);
    assert!(matches!(
        facade
            .scoped(session)
            .agent_message_send(unrelated, "work".to_owned(), None),
        Err(AgentMailboxError::UnrelatedRecipient)
    ));
    assert!(matches!(
        facade
            .scoped(session)
            .agent_message_send(foreign, "work".to_owned(), None),
        Err(AgentMailboxError::UnknownRecipient)
    ));
}

#[test]
fn roster_keeps_surviving_siblings_in_the_dead_ancestors_authorization_group() {
    let (facade, project) = facade_with_agent_tool();
    let lead = agent(&facade, project, "lead");
    let first = agent(&facade, project, "first");
    let second = agent(&facade, project, "second");
    facade.lineage.record(first, lead);
    facade.lineage.record(second, lead);
    facade
        .lineage
        .retain_live(&std::collections::HashSet::from([first, second]));
    let session = authentic_session(&facade, first, TEST_PEER_PGID);
    facade
        .scoped(session)
        .bind_session_process(first)
        .expect("bind worker");

    let roster = facade.scoped(session).agent_roster().expect("roster");

    assert_eq!(roster.len(), 2);
    assert!(roster.iter().any(|entry| {
        entry.process == second && entry.relationship == AgentRelationship::Sibling
    }));
    assert!(roster.iter().all(|entry| entry.root == lead));
}
