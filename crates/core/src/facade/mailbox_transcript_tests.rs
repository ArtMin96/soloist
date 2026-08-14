//! Behavioural tests for recording and announcing agent-to-agent exchanges. They drive the façade
//! the way an MCP client does — bound sessions, real authorization — and read the transcript back
//! through the orchestration snapshot, which is the read a human's Messages view actually renders.
//!
//! The load-bearing case is that a full transcript never fails a send: the delivery queue refuses
//! overflow because losing a queued message loses work, while the transcript evicts because
//! refusing would make a send fail over a display record.

use crate::coordination::{
    AgentMessageKind, AgentMessageOutcome, AgentMessageRecord, MAX_TRANSCRIPT_BODY_BYTES,
    MAX_TRANSCRIPT_ENTRIES_PER_PROJECT,
};
use crate::events::DomainEvent;
use crate::facade::{AgentMailboxError, Facade, SpawnAgentRequest};
use crate::ids::{AgentMessageId, ProcessId, ProjectId, SessionId};
use crate::testing::{bound_agent, drain, facade_with_agent_tool, TEST_PEER_PGID};

/// A lead and one worker it spawned, each bound to its own session — the smallest live lineage
/// group in which one agent may address another.
struct Pair {
    facade: Facade,
    project: ProjectId,
    lead: ProcessId,
    lead_session: SessionId,
    worker: ProcessId,
    worker_session: SessionId,
}

fn pair() -> Pair {
    let (facade, project) = facade_with_agent_tool();
    let (lead, lead_session) = bound_agent(&facade, project, "lead", TEST_PEER_PGID);
    let (worker, worker_session) = bound_agent(&facade, project, "worker", TEST_PEER_PGID + 1);
    facade.lineage.record(worker, lead);
    Pair {
        facade,
        project,
        lead,
        lead_session,
        worker,
        worker_session,
    }
}

impl Pair {
    fn messages(&self) -> Vec<AgentMessageRecord> {
        self.messages_in(self.project)
    }

    fn messages_in(&self, project: ProjectId) -> Vec<AgentMessageRecord> {
        self.facade
            .orchestration_snapshot(project)
            .expect("read the orchestration snapshot")
            .messages
    }

    fn send(&self, body: &str) -> Result<AgentMessageId, AgentMailboxError> {
        self.facade
            .scoped(self.lead_session)
            .agent_message_send(self.worker, body.to_owned(), None)
            .map(|delivery| delivery.message.id)
    }

    fn acknowledge(&self, id: AgentMessageId) {
        self.facade
            .scoped(self.worker_session)
            .agent_message_acknowledge(id)
            .expect("the worker acknowledges");
    }
}

fn announced(events: Vec<DomainEvent>) -> Vec<AgentMessageId> {
    events
        .into_iter()
        .filter_map(|event| match event {
            DomainEvent::AgentMessageChanged { id, .. } => Some(id),
            _ => None,
        })
        .collect()
}

#[test]
fn an_overflowing_transcript_evicts_the_oldest_and_the_send_still_succeeds() {
    let pair = pair();
    // Each exchange is acknowledged before the next, so the delivery queue's own recipient ceiling
    // is never the thing under test — only the transcript's.
    let mut refused = Vec::new();
    for index in 0..=MAX_TRANSCRIPT_ENTRIES_PER_PROJECT {
        match pair.send(&index.to_string()) {
            Ok(id) => pair.acknowledge(id),
            Err(error) => refused.push((index, error.to_string())),
        }
    }

    assert!(
        refused.is_empty(),
        "a full transcript must never fail a send: {refused:?}",
    );
    let bodies: Vec<_> = pair
        .messages()
        .into_iter()
        .map(|record| record.delivery.message.body)
        .collect();
    assert_eq!(bodies.len(), MAX_TRANSCRIPT_ENTRIES_PER_PROJECT);
    assert_eq!(
        bodies.first().map(String::as_str),
        Some("1"),
        "the oldest exchange was evicted to make room",
    );
    assert_eq!(
        bodies.last().map(String::as_str),
        Some(MAX_TRANSCRIPT_ENTRIES_PER_PROJECT.to_string().as_str()),
        "the newest exchange is retained",
    );
}

#[test]
fn a_direct_send_is_recorded_and_announced() {
    let pair = pair();
    let mut rx = pair.facade.subscribe();

    let id = pair.send("review the parser").expect("the send succeeds");

    let messages = pair.messages();
    assert_eq!(messages.len(), 1, "one exchange recorded: {messages:?}");
    let record = &messages[0];
    assert_eq!(record.delivery.message.id, id);
    assert_eq!(record.delivery.message.sender, pair.lead);
    assert_eq!(record.delivery.message.recipient, pair.worker);
    assert_eq!(record.delivery.message.kind, AgentMessageKind::Direct);
    assert_eq!(record.delivery.message.body, "review the parser");
    assert!(!record.truncated);
    assert_eq!(announced(drain(&mut rx)), vec![id]);
}

#[test]
fn a_broadcast_records_and_announces_one_entry_per_recipient() {
    let pair = pair();
    let (second, _) = bound_agent(&pair.facade, pair.project, "second", TEST_PEER_PGID + 2);
    pair.facade.lineage.record(second, pair.lead);
    let mut rx = pair.facade.subscribe();

    pair.facade
        .scoped(pair.lead_session)
        .agent_message_broadcast("stand down".to_owned(), None)
        .expect("the broadcast succeeds");

    let messages = pair.messages();
    assert_eq!(
        messages.len(),
        2,
        "one entry per recipient, not one per broadcast: {messages:?}",
    );
    let mut recipients: Vec<_> = messages
        .iter()
        .map(|record| record.delivery.message.recipient)
        .collect();
    recipients.sort();
    let mut expected = vec![pair.worker, second];
    expected.sort();
    assert_eq!(recipients, expected);
    assert!(messages
        .iter()
        .all(|record| record.delivery.message.body == "stand down"));
    let mut ids: Vec<_> = messages
        .iter()
        .map(|record| record.delivery.message.id)
        .collect();
    ids.sort();
    let mut got = announced(drain(&mut rx));
    got.sort();
    assert_eq!(got, ids, "one announcement per recipient");
}

#[tokio::test]
async fn a_spawn_time_task_is_recorded_and_announced() {
    let (facade, project) = facade_with_agent_tool();
    let (_, lead_session) = bound_agent(&facade, project, "lead", TEST_PEER_PGID);
    let mut rx = facade.subscribe();

    let spawned = facade
        .scoped(lead_session)
        .spawn_agent_request(SpawnAgentRequest {
            tool: "worker".into(),
            extra_args: Vec::new(),
            prompt: Some("implement the parser".into()),
            todo_id: None,
            include_agent_instructions: true,
        })
        .expect("spawn the worker with its first task");

    let task = spawned
        .initial_message
        .expect("the task was queued")
        .message
        .id;
    let messages = facade
        .orchestration_snapshot(project)
        .expect("read the orchestration snapshot")
        .messages;
    assert_eq!(
        messages.len(),
        1,
        "the spawn task is recorded: {messages:?}"
    );
    assert_eq!(messages[0].delivery.message.id, task);
    assert_eq!(messages[0].delivery.message.kind, AgentMessageKind::Task);
    assert_eq!(messages[0].delivery.message.body, "implement the parser");
    assert!(announced(drain(&mut rx)).contains(&task));
}

#[test]
fn a_transcript_is_scoped_to_its_project() {
    let pair = pair();
    // Derived from the loaded project rather than minted, so it cannot collide with it.
    let other = ProjectId::from_raw(pair.project.get() + 1);

    pair.send("review the parser").expect("the send succeeds");

    assert_eq!(pair.messages().len(), 1);
    assert!(
        pair.messages_in(other).is_empty(),
        "another project sees none of this project's exchanges",
    );
}

#[test]
fn the_outcome_transitions_are_visible_in_the_transcript() {
    let pair = pair();
    let id = pair.send("review the parser").expect("the send succeeds");
    // The recipient was never classified idle, so the send left the exchange merely queued.
    assert_eq!(
        pair.messages()[0].delivery.outcome,
        AgentMessageOutcome::Queued,
    );

    pair.acknowledge(id);

    let messages = pair.messages();
    assert_eq!(
        messages.len(),
        1,
        "acknowledging updates the entry rather than appending a second: {messages:?}",
    );
    assert_eq!(
        messages[0].delivery.outcome,
        AgentMessageOutcome::Acknowledged,
    );
}

#[test]
fn an_acknowledgement_is_announced() {
    let pair = pair();
    let id = pair.send("review the parser").expect("the send succeeds");
    let mut rx = pair.facade.subscribe();

    pair.acknowledge(id);

    assert_eq!(announced(drain(&mut rx)), vec![id]);
}

#[test]
fn an_oversized_body_is_truncated_in_the_record_but_delivered_intact() {
    let pair = pair();
    // A three-byte codepoint, so the retained-body cap lands mid-character.
    let body = "…".repeat(MAX_TRANSCRIPT_BODY_BYTES);

    let id = pair.send(&body).expect("the send succeeds");

    let record = &pair.messages()[0];
    assert!(record.truncated);
    assert!(record.delivery.message.body.len() <= MAX_TRANSCRIPT_BODY_BYTES);
    let delivered = pair
        .facade
        .scoped(pair.worker_session)
        .agent_message_get(id)
        .expect("the worker reads its message");
    assert_eq!(
        delivered.message.body, body,
        "only the display record is cut; the delivered message is whole",
    );
}
