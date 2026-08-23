use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::agents::{AgentKind, AgentTool, PromptMode};
use crate::composition::CorePorts;
use crate::coordination::{
    BlockerGate, CommentAuthor, CommentEdit, ScratchpadLink, StoredTodo,
    TodoCompletionAtomicResult, TodoCompletionCompareResult, TodoCompletionContext,
    TodoCompletionDecision, TodoDoc, TodoRepo, TodoStatus, TodoWriteResult,
    MAX_PENDING_MESSAGES_PER_RECIPIENT,
};
use crate::events::DomainEvent;
use crate::facade::Facade;
use crate::ids::{ProjectId, ScratchpadId, SessionId};
use crate::ports::ProjectRepo;
use crate::testing::{
    agent_registration, authentic_session, bound_agent, drain, facade_with_agent_tool,
    FakeAgentToolRepo, FakeProjectRepo, FakeSpawner, FakeTodoRepo, FakeTrustRepo, MockClock,
    TEST_PEER_PGID,
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
    // Derived from the loaded project rather than minted, so it cannot collide with it.
    let other_project = crate::ids::ProjectId::from_raw(project.get() + 1);
    let foreign = agent(&facade, other_project, "foreign");
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

#[tokio::test]
async fn roster_keeps_surviving_siblings_in_the_dead_ancestors_authorization_group() {
    let (facade, project) = facade_with_agent_tool();
    let lead = agent(&facade, project, "lead");
    let first = agent(&facade, project, "first");
    let second = agent(&facade, project, "second");
    facade.lineage.record(first, lead);
    facade.lineage.record(second, lead);
    facade
        .supervisor()
        .close(lead)
        .await
        .expect("close the lead");
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

/// The bodies pending for `process`, read by process id so a recipient with no session of its own —
/// or one outside the sender's project — is inspected the same way as a bound one.
fn pending_bodies(facade: &Facade, process: ProcessId) -> Vec<String> {
    facade
        .mailbox
        .list(process)
        .into_iter()
        .map(|delivery| delivery.message.body)
        .collect()
}

#[test]
fn a_broadcast_reaches_every_other_live_member_of_the_senders_group() {
    let (facade, project) = facade_with_agent_tool();
    let (lead, session) = bound_agent(&facade, project, "lead", TEST_PEER_PGID);
    let first = agent(&facade, project, "first");
    let second = agent(&facade, project, "second");
    facade.lineage.record(first, lead);
    facade.lineage.record(second, lead);

    let receipt = facade
        .scoped(session)
        .agent_message_broadcast("regroup".to_owned(), None)
        .expect("the broadcast succeeds");

    let mut addressed: Vec<_> = receipt
        .deliveries
        .iter()
        .map(|delivery| delivery.recipient)
        .collect();
    addressed.sort();
    let mut group = vec![first, second];
    group.sort();
    assert_eq!(
        addressed, group,
        "the receipt names both other members: {receipt:?}",
    );
    for member in [first, second] {
        let pending = facade.mailbox.list(member);
        assert_eq!(
            pending.len(),
            1,
            "one message reached {member:?}: {pending:?}"
        );
        assert_eq!(pending[0].message.body, "regroup");
        assert_eq!(pending[0].message.kind, AgentMessageKind::Direct);
        assert_eq!(pending[0].message.sender, lead);
        assert!(
            receipt
                .deliveries
                .iter()
                .any(|delivery| delivery.recipient == member
                    && delivery.message_id == pending[0].message.id),
            "the receipt names the message that landed for {member:?}: {receipt:?}",
        );
    }
}

#[test]
fn a_broadcast_never_reaches_its_own_sender() {
    let (facade, project) = facade_with_agent_tool();
    let (lead, lead_session) = bound_agent(&facade, project, "lead", TEST_PEER_PGID);
    let (child, child_session) = bound_agent(&facade, project, "child", TEST_PEER_PGID + 1);
    facade.lineage.record(child, lead);
    facade
        .scoped(child_session)
        .agent_message_send(lead, "status".to_owned(), None)
        .expect("the child reports in before the lead broadcasts");

    facade
        .scoped(lead_session)
        .agent_message_broadcast("regroup".to_owned(), None)
        .expect("the broadcast succeeds");

    assert_eq!(
        pending_bodies(&facade, lead),
        vec!["status".to_owned()],
        "the sender's inbox still holds only what another agent sent it",
    );
    assert_eq!(pending_bodies(&facade, child), vec!["regroup".to_owned()]);
}

#[test]
fn a_broadcast_addresses_no_agent_outside_the_senders_group_or_project() {
    let (facade, project) = facade_with_agent_tool();
    let (lead, session) = bound_agent(&facade, project, "lead", TEST_PEER_PGID);
    let child = agent(&facade, project, "child");
    let unrelated = agent(&facade, project, "unrelated");
    // Derived from the loaded project rather than minted, so it cannot collide with it.
    let other_project = ProjectId::from_raw(project.get() + 1);
    let foreign = agent(&facade, other_project, "foreign");
    facade.lineage.record(child, lead);
    // A lineage child in another project, so only the project boundary can exclude it.
    facade.lineage.record(foreign, lead);

    let receipt = facade
        .scoped(session)
        .agent_message_broadcast("regroup".to_owned(), None)
        .expect("the broadcast succeeds");

    assert_eq!(
        receipt
            .deliveries
            .iter()
            .map(|delivery| delivery.recipient)
            .collect::<Vec<_>>(),
        vec![child],
        "only the group member in the sender's project was addressed: {receipt:?}",
    );
    assert_eq!(pending_bodies(&facade, child), vec!["regroup".to_owned()]);
    assert!(
        pending_bodies(&facade, unrelated).is_empty(),
        "an agent outside the sender's lineage group receives nothing",
    );
    assert!(
        pending_bodies(&facade, foreign).is_empty(),
        "an agent in another project receives nothing",
    );
}

#[test]
fn a_broadcast_that_would_overflow_one_inbox_delivers_to_nobody() {
    let (facade, project) = facade_with_agent_tool();
    let (lead, session) = bound_agent(&facade, project, "lead", TEST_PEER_PGID);
    let spare = agent(&facade, project, "spare");
    let full = agent(&facade, project, "full");
    facade.lineage.record(spare, lead);
    facade.lineage.record(full, lead);
    for _ in 0..MAX_PENDING_MESSAGES_PER_RECIPIENT {
        facade
            .scoped(session)
            .agent_message_send(full, "status".to_owned(), None)
            .expect("fill one member's inbox to its ceiling");
    }

    let refused = facade
        .scoped(session)
        .agent_message_broadcast("regroup".to_owned(), None);

    assert!(
        matches!(refused, Err(AgentMailboxError::RecipientQueueFull)),
        "the group send is refused whole: {refused:?}",
    );
    assert!(
        pending_bodies(&facade, spare).is_empty(),
        "no member was enqueued before the capacity refusal",
    );
    assert_eq!(
        facade.mailbox.list(full).len(),
        MAX_PENDING_MESSAGES_PER_RECIPIENT,
        "the full inbox is left exactly as it was",
    );
}

#[test]
fn a_broadcast_with_no_other_live_members_succeeds_and_delivers_nothing() {
    let (facade, project) = facade_with_agent_tool();
    let (lead, session) = bound_agent(&facade, project, "lead", TEST_PEER_PGID);
    let unrelated = agent(&facade, project, "unrelated");

    let receipt = facade
        .scoped(session)
        .agent_message_broadcast("regroup".to_owned(), None)
        .expect("a group of one still succeeds");

    assert!(
        receipt.deliveries.is_empty(),
        "nothing was addressed: {receipt:?}",
    );
    assert!(
        pending_bodies(&facade, lead).is_empty(),
        "the lone sender receives nothing",
    );
    assert!(pending_bodies(&facade, unrelated).is_empty());
}

/// A façade with one project, one launchable agent tool, and `todos` wired, so a lead can spawn a
/// worker carrying an addressed first task and that worker's report reaches durable storage.
fn facade_with(todos: Arc<dyn TodoRepo>) -> (Facade, ProjectId) {
    let projects = Arc::new(FakeProjectRepo::new());
    let project = projects
        .upsert(Path::new("/"), Some("proj"), None)
        .expect("seed a project")
        .id;
    let tool = AgentTool {
        name: "worker".into(),
        command: "true".into(),
        default_args: Vec::new(),
        kind: AgentKind::Generic,
        prompt_mode: PromptMode::AppendedArg,
    };
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(MockClock::new()),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .agent_tools(Arc::new(FakeAgentToolRepo::new(vec![tool])))
        .todo_repo(todos)
        .build(),
    );
    (facade, project)
}

/// The ordinary composition: a real (in-memory) todo store behind the façade.
fn facade() -> (Facade, ProjectId) {
    facade_with(Arc::new(FakeTodoRepo::new()))
}

/// The lead agent of a spawn, bound to its own session.
fn bound_lead(facade: &Facade, project: ProjectId) -> (ProcessId, SessionId) {
    bound_agent(facade, project, "lead", TEST_PEER_PGID)
}

/// A worker the lead spawned with one addressed first task, itself bound to `pgid`, with the id of
/// the task it must report against.
fn worker_with_task(
    facade: &Facade,
    lead_session: SessionId,
    todo_id: Option<TodoId>,
    pgid: i32,
) -> (ProcessId, SessionId, AgentMessageId) {
    let spawned = facade
        .scoped(lead_session)
        .spawn_agent_request(SpawnAgentRequest {
            tool: "worker".into(),
            extra_args: Vec::new(),
            prompt: Some("implement the parser".into()),
            todo_id,
            include_agent_instructions: true,
        })
        .expect("spawn the worker with its first task");
    let task = spawned
        .initial_message
        .expect("the task was queued")
        .message
        .id;
    let session = authentic_session(facade, spawned.process, pgid);
    facade
        .scoped(session)
        .bind_session_process(spawned.process)
        .expect("bind the worker to its own process");
    (spawned.process, session, task)
}

/// A representative todo document.
fn todo_doc(title: &str) -> TodoDoc {
    TodoDoc {
        title: title.into(),
        body: format!("do {title}"),
        status: TodoStatus::Open,
    }
}

/// The lead's pending inbox.
fn lead_inbox(facade: &Facade, lead_session: SessionId) -> Vec<AgentMessageDelivery> {
    facade
        .scoped(lead_session)
        .agent_message_list()
        .expect("the lead's inbox")
}

/// The durable state of `todo`: its status and its comment bodies.
fn todo_state(facade: &Facade, session: SessionId, todo: TodoId) -> (TodoStatus, Vec<String>) {
    let view = facade
        .scoped(session)
        .todo_get(todo)
        .expect("read the todo");
    (
        view.doc.status,
        view.comments.into_iter().map(|c| c.body).collect(),
    )
}

#[tokio::test]
async fn completing_a_task_that_carried_a_todo_records_one_result_and_notifies_the_lead() {
    let (facade, project) = facade();
    let (lead, lead_session) = bound_lead(&facade, project);
    let todo = facade
        .scoped(lead_session)
        .todo_create(todo_doc("build"), None)
        .expect("create the todo")
        .view;
    let (_, worker_session, task) =
        worker_with_task(&facade, lead_session, Some(todo.id), TEST_PEER_PGID + 1);
    let mut rx = facade.subscribe();

    let report = facade
        .scoped(worker_session)
        .agent_report_completion(task, Some(todo.id), "parser landed".to_owned())
        .expect("the report succeeds");

    assert_eq!(report.occurrence, Some(TodoCompletionOccurrence::Recorded));
    assert_eq!(
        todo_state(&facade, lead_session, todo.id),
        (TodoStatus::Done, vec!["parser landed".to_owned()]),
        "the durable todo is done with exactly one result comment",
    );
    let inbox = lead_inbox(&facade, lead_session);
    assert_eq!(inbox.len(), 1, "one notice reached the lead: {inbox:?}");
    assert_eq!(inbox[0].message.kind, AgentMessageKind::Completion);
    assert_eq!(inbox[0].message.body, "parser landed");
    assert_eq!(inbox[0].message.recipient, lead);
    assert!(matches!(
        report.notification,
        CompletionNotification::Enqueued { .. }
    ));
    let changed: Vec<_> = drain(&mut rx)
        .into_iter()
        .filter(|event| matches!(event, DomainEvent::TodoChanged { id, .. } if *id == todo.id))
        .collect();
    assert_eq!(changed.len(), 1, "one todo change published: {changed:?}");
}

#[tokio::test]
async fn completing_a_task_that_carried_no_todo_notifies_the_lead_and_touches_no_todo() {
    let (facade, project) = facade();
    let (_, lead_session) = bound_lead(&facade, project);
    let todo = facade
        .scoped(lead_session)
        .todo_create(todo_doc("build"), None)
        .expect("create the todo")
        .view;
    let (_, worker_session, task) =
        worker_with_task(&facade, lead_session, None, TEST_PEER_PGID + 1);

    let report = facade
        .scoped(worker_session)
        .agent_report_completion(task, None, "parser landed".to_owned())
        .expect("a task with no todo still completes");

    assert_eq!(report.completion, None, "no durable record without a todo");
    assert_eq!(report.occurrence, None);
    assert!(matches!(
        report.notification,
        CompletionNotification::Enqueued { .. }
    ));
    assert_eq!(
        lead_inbox(&facade, lead_session).len(),
        1,
        "the lead still learns the task finished",
    );
    assert_eq!(
        todo_state(&facade, lead_session, todo.id),
        (TodoStatus::Open, Vec::new()),
        "an unrelated todo is untouched",
    );
}

#[tokio::test]
async fn a_completion_naming_a_todo_the_task_did_not_carry_is_refused() {
    let (facade, project) = facade();
    let (_, lead_session) = bound_lead(&facade, project);
    let todo = facade
        .scoped(lead_session)
        .todo_create(todo_doc("build"), None)
        .expect("create the todo")
        .view;
    let (_, with_todo, carried) =
        worker_with_task(&facade, lead_session, Some(todo.id), TEST_PEER_PGID + 1);
    let (_, without_todo, uncarried) =
        worker_with_task(&facade, lead_session, None, TEST_PEER_PGID + 2);

    assert!(
        matches!(
            facade
                .scoped(with_todo)
                .agent_report_completion(carried, None, "done".to_owned()),
            Err(AgentMailboxError::UnknownTodo)
        ),
        "dropping the todo the task carried is a mismatch",
    );
    assert!(
        matches!(
            facade.scoped(without_todo).agent_report_completion(
                uncarried,
                Some(todo.id),
                "done".to_owned()
            ),
            Err(AgentMailboxError::UnknownTodo)
        ),
        "claiming a todo the task never carried is a mismatch",
    );
    assert_eq!(
        todo_state(&facade, lead_session, todo.id),
        (TodoStatus::Open, Vec::new()),
        "a refused report changes nothing",
    );
    assert!(
        lead_inbox(&facade, lead_session).is_empty(),
        "a refused report queues no notice",
    );
}

#[tokio::test]
async fn repeating_a_completion_that_carried_a_todo_adds_no_second_comment_or_notice() {
    let (facade, project) = facade();
    let (_, lead_session) = bound_lead(&facade, project);
    let todo = facade
        .scoped(lead_session)
        .todo_create(todo_doc("build"), None)
        .expect("create the todo")
        .view;
    let (_, worker_session, task) =
        worker_with_task(&facade, lead_session, Some(todo.id), TEST_PEER_PGID + 1);
    facade
        .scoped(worker_session)
        .agent_report_completion(task, Some(todo.id), "parser landed".to_owned())
        .expect("the first report");
    let mut rx = facade.subscribe();

    let repeat = facade
        .scoped(worker_session)
        .agent_report_completion(task, Some(todo.id), "parser landed".to_owned())
        .expect("the repeat succeeds");

    assert_eq!(repeat.occurrence, Some(TodoCompletionOccurrence::Existing));
    assert_eq!(
        todo_state(&facade, lead_session, todo.id),
        (TodoStatus::Done, vec!["parser landed".to_owned()]),
        "the repeat appends no second result comment",
    );
    let inbox = lead_inbox(&facade, lead_session);
    assert_eq!(inbox.len(), 1, "still one notice: {inbox:?}");
    assert!(matches!(
        repeat.notification,
        CompletionNotification::Pending { .. }
    ));
    assert!(
        drain(&mut rx)
            .iter()
            .all(|event| !matches!(event, DomainEvent::TodoChanged { .. })),
        "the repeat publishes no todo change",
    );

    // Once the lead has read the notice it is no longer pending, and a later repeat must still not
    // queue a replacement.
    facade
        .scoped(lead_session)
        .agent_message_acknowledge(inbox[0].message.id)
        .expect("the lead reads the notice");
    let after_read = facade
        .scoped(worker_session)
        .agent_report_completion(task, Some(todo.id), "parser landed".to_owned())
        .expect("the repeat after the lead read it");
    assert_eq!(
        after_read.notification,
        CompletionNotification::AlreadyQueued
    );
    assert!(
        lead_inbox(&facade, lead_session).is_empty(),
        "no replacement notice was queued",
    );
}

#[tokio::test]
async fn repeating_a_completion_that_carried_no_todo_queues_no_second_notice() {
    let (facade, project) = facade();
    let (_, lead_session) = bound_lead(&facade, project);
    let (_, worker_session, task) =
        worker_with_task(&facade, lead_session, None, TEST_PEER_PGID + 1);
    facade
        .scoped(worker_session)
        .agent_report_completion(task, None, "parser landed".to_owned())
        .expect("the first report");

    let repeat = facade
        .scoped(worker_session)
        .agent_report_completion(task, None, "parser landed".to_owned())
        .expect("the repeat succeeds");

    let inbox = lead_inbox(&facade, lead_session);
    assert_eq!(inbox.len(), 1, "still one notice: {inbox:?}");
    assert!(matches!(
        repeat.notification,
        CompletionNotification::Pending { .. }
    ));

    facade
        .scoped(lead_session)
        .agent_message_acknowledge(inbox[0].message.id)
        .expect("the lead reads the notice");
    let after_read = facade
        .scoped(worker_session)
        .agent_report_completion(task, None, "parser landed".to_owned())
        .expect("the repeat after the lead read it");
    assert_eq!(
        after_read.notification,
        CompletionNotification::AlreadyQueued
    );
    assert!(
        lead_inbox(&facade, lead_session).is_empty(),
        "no replacement notice was queued",
    );
}

#[tokio::test]
async fn a_completion_whose_lead_is_gone_still_records_the_durable_result() {
    let (facade, project) = facade();
    let (lead, lead_session) = bound_lead(&facade, project);
    let todo = facade
        .scoped(lead_session)
        .todo_create(todo_doc("build"), None)
        .expect("create the todo")
        .view;
    let (_, worker_session, task) =
        worker_with_task(&facade, lead_session, Some(todo.id), TEST_PEER_PGID + 1);
    facade
        .supervisor()
        .close(lead)
        .await
        .expect("the lead leaves while the worker is working");

    let report = facade
        .scoped(worker_session)
        .agent_report_completion(task, Some(todo.id), "parser landed".to_owned())
        .expect("a missing lead never fails the report");

    assert_eq!(report.occurrence, Some(TodoCompletionOccurrence::Recorded));
    assert_eq!(
        report.notification,
        CompletionNotification::Deferred {
            recipient: Some(lead)
        },
        "the undeliverable notice is reported, not raised",
    );
    let (status, comments) = todo_state(&facade, worker_session, todo.id);
    assert_eq!(status, TodoStatus::Done);
    assert_eq!(comments, vec!["parser landed".to_owned()]);
}

#[tokio::test]
async fn a_completion_reported_after_the_task_was_read_survives_the_leads_departure() {
    let (facade, project) = facade();
    let (lead, lead_session) = bound_lead(&facade, project);
    let todo = facade
        .scoped(lead_session)
        .todo_create(todo_doc("build"), None)
        .expect("create the todo")
        .view;
    let (_, worker_session, task) =
        worker_with_task(&facade, lead_session, Some(todo.id), TEST_PEER_PGID + 1);
    facade
        .scoped(worker_session)
        .agent_message_acknowledge(task)
        .expect("the worker reads its task before starting on it");
    facade
        .supervisor()
        .close(lead)
        .await
        .expect("the lead leaves while the worker is working");
    // The cleanup the mailbox reactor performs on the lead's removal.
    facade.mailbox.remove_process(lead);

    let report = facade
        .scoped(worker_session)
        .agent_report_completion(task, Some(todo.id), "parser landed".to_owned())
        .expect("a read task stays reportable once its lead has gone");

    assert_eq!(report.occurrence, Some(TodoCompletionOccurrence::Recorded));
    assert_eq!(
        report.notification,
        CompletionNotification::Deferred {
            recipient: Some(lead)
        },
    );
    let (status, comments) = todo_state(&facade, worker_session, todo.id);
    assert_eq!(status, TodoStatus::Done);
    assert_eq!(comments, vec!["parser landed".to_owned()]);
}

#[tokio::test]
async fn a_completion_whose_lead_inbox_is_full_still_records_the_durable_result() {
    let (facade, project) = facade();
    let (lead, lead_session) = bound_lead(&facade, project);
    let todo = facade
        .scoped(lead_session)
        .todo_create(todo_doc("build"), None)
        .expect("create the todo")
        .view;
    let (_, worker_session, task) =
        worker_with_task(&facade, lead_session, Some(todo.id), TEST_PEER_PGID + 1);
    for _ in 0..MAX_PENDING_MESSAGES_PER_RECIPIENT {
        facade
            .scoped(worker_session)
            .agent_message_send(lead, "status".to_owned(), None)
            .expect("fill the lead's inbox to its ceiling");
    }

    let report = facade
        .scoped(worker_session)
        .agent_report_completion(task, Some(todo.id), "parser landed".to_owned())
        .expect("a full mailbox never fails the report");

    assert_eq!(report.occurrence, Some(TodoCompletionOccurrence::Recorded));
    assert_eq!(
        report.notification,
        CompletionNotification::Deferred {
            recipient: Some(lead)
        },
    );
    assert_eq!(
        lead_inbox(&facade, lead_session).len(),
        MAX_PENDING_MESSAGES_PER_RECIPIENT,
        "the ceiling dropped nothing to make room",
    );
    let (status, comments) = todo_state(&facade, worker_session, todo.id);
    assert_eq!(status, TodoStatus::Done);
    assert_eq!(comments, vec!["parser landed".to_owned()]);
}

#[tokio::test]
async fn a_completion_whose_notice_flag_cannot_be_stored_still_reports_success() {
    let (facade, project) = facade_with(Arc::new(RefusesNoticeFlag::new()));
    let (_, lead_session) = bound_lead(&facade, project);
    let todo = facade
        .scoped(lead_session)
        .todo_create(todo_doc("build"), None)
        .expect("create the todo")
        .view;
    let (_, worker_session, task) =
        worker_with_task(&facade, lead_session, Some(todo.id), TEST_PEER_PGID + 1);

    let report = facade
        .scoped(worker_session)
        .agent_report_completion(task, Some(todo.id), "parser landed".to_owned())
        .expect("a refused notice flag never rolls back the committed completion");

    assert_eq!(report.occurrence, Some(TodoCompletionOccurrence::Recorded));
    assert!(matches!(
        report.notification,
        CompletionNotification::Enqueued { .. }
    ));
    assert_eq!(
        lead_inbox(&facade, lead_session).len(),
        1,
        "the notice really did reach the lead",
    );
    let (status, comments) = todo_state(&facade, lead_session, todo.id);
    assert_eq!(status, TodoStatus::Done);
    assert_eq!(comments, vec!["parser landed".to_owned()]);
}

/// A todo store that commits completions normally but refuses the follow-up notice-flag write —
/// the only durable failure that can land strictly after a committed completion, and one no
/// single-threaded call can otherwise produce.
struct RefusesNoticeFlag {
    inner: FakeTodoRepo,
}

impl RefusesNoticeFlag {
    fn new() -> Self {
        Self {
            inner: FakeTodoRepo::new(),
        }
    }
}

impl TodoRepo for RefusesNoticeFlag {
    fn apply_completion(
        &self,
        project: ProjectId,
        id: TodoId,
        policy: &mut dyn FnMut(&TodoCompletionContext) -> TodoCompletionDecision,
    ) -> Result<TodoCompletionAtomicResult, StoreError> {
        self.inner.apply_completion(project, id, policy)
    }

    fn compare_completion(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _expected: &TodoCompletion,
        _replacement: &TodoCompletion,
    ) -> Result<TodoCompletionCompareResult, StoreError> {
        Err(StoreError::Backend("notice flag refused".into()))
    }

    fn create(
        &self,
        project: ProjectId,
        doc: &TodoDoc,
        scratchpad: Option<ScratchpadId>,
    ) -> Result<StoredTodo, StoreError> {
        self.inner.create(project, doc, scratchpad)
    }

    fn read(&self, project: ProjectId, id: TodoId) -> Result<Option<StoredTodo>, StoreError> {
        self.inner.read(project, id)
    }

    fn list(&self, project: ProjectId) -> Result<Vec<StoredTodo>, StoreError> {
        self.inner.list(project)
    }

    fn write_doc(
        &self,
        project: ProjectId,
        id: TodoId,
        doc: &TodoDoc,
        scratchpad: ScratchpadLink<ScratchpadId>,
        expected: u64,
        gate: BlockerGate,
    ) -> Result<TodoWriteResult, StoreError> {
        self.inner
            .write_doc(project, id, doc, scratchpad, expected, gate)
    }

    fn delete(&self, project: ProjectId, id: TodoId) -> Result<bool, StoreError> {
        self.inner.delete(project, id)
    }

    fn tags(&self, project: ProjectId) -> Result<Vec<String>, StoreError> {
        self.inner.tags(project)
    }

    fn add_tag(
        &self,
        project: ProjectId,
        id: TodoId,
        tag: &str,
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.inner.add_tag(project, id, tag)
    }

    fn remove_tag(
        &self,
        project: ProjectId,
        id: TodoId,
        tag: &str,
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.inner.remove_tag(project, id, tag)
    }

    fn set_blockers(
        &self,
        project: ProjectId,
        id: TodoId,
        blockers: &[TodoId],
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.inner.set_blockers(project, id, blockers)
    }

    fn add_blocker(
        &self,
        project: ProjectId,
        id: TodoId,
        blocker: TodoId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.inner.add_blocker(project, id, blocker)
    }

    fn remove_blocker(
        &self,
        project: ProjectId,
        id: TodoId,
        blocker: TodoId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.inner.remove_blocker(project, id, blocker)
    }

    fn unmet_blockers(
        &self,
        project: ProjectId,
        blockers: &[TodoId],
    ) -> Result<Vec<TodoId>, StoreError> {
        self.inner.unmet_blockers(project, blockers)
    }

    fn lock(
        &self,
        project: ProjectId,
        id: TodoId,
        owner: ProcessId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.inner.lock(project, id, owner)
    }

    fn unlock(
        &self,
        project: ProjectId,
        id: TodoId,
        owner: ProcessId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.inner.unlock(project, id, owner)
    }

    fn comment_create(
        &self,
        project: ProjectId,
        id: TodoId,
        body: &str,
        author: Option<CommentAuthor>,
    ) -> Result<Option<(StoredTodo, u64)>, StoreError> {
        self.inner.comment_create(project, id, body, author)
    }

    fn comment_update(
        &self,
        project: ProjectId,
        id: TodoId,
        comment: u64,
        body: &str,
    ) -> Result<CommentEdit, StoreError> {
        self.inner.comment_update(project, id, comment, body)
    }

    fn comment_delete(
        &self,
        project: ProjectId,
        id: TodoId,
        comment: u64,
    ) -> Result<CommentEdit, StoreError> {
        self.inner.comment_delete(project, id, comment)
    }

    fn release_owner(&self, process: ProcessId) -> Result<usize, StoreError> {
        self.inner.release_owner(process)
    }

    fn clear_locks(&self) -> Result<usize, StoreError> {
        self.inner.clear_locks()
    }

    fn transfer(
        &self,
        from: ProjectId,
        to: ProjectId,
        id: TodoId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.inner.transfer(from, to, id)
    }
}
