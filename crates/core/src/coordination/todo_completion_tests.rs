use std::sync::Arc;

use super::*;
use crate::coordination::{CommentOutcome, ScratchpadLink, TodoDoc};
use crate::testing::FakeTodoRepo;

const PROJECT: ProjectId = ProjectId::from_raw(1);
const REPORTER: ProcessId = ProcessId::from_raw(7);
const TASK: AgentMessageId = AgentMessageId::from_raw(11);

fn doc(title: &str, status: TodoStatus) -> TodoDoc {
    TodoDoc {
        title: title.into(),
        body: "do it".into(),
        status,
    }
}

fn author() -> CommentAuthor {
    CommentAuthor::Process {
        id: REPORTER,
        label: "worker".into(),
    }
}

fn setup() -> (Arc<FakeTodoRepo>, Todos) {
    let repo = Arc::new(FakeTodoRepo::new());
    let todos = Todos::new(repo.clone());
    (repo, todos)
}

#[test]
fn blockers_refuse_the_whole_completion() {
    let (repo, todos) = setup();
    let blocker = todos
        .create(PROJECT, doc("dependency", TodoStatus::Open), None)
        .expect("create blocker");
    let todo = todos
        .create(PROJECT, doc("ship", TodoStatus::InProgress), None)
        .expect("create todo");
    todos
        .set_blockers(PROJECT, todo.id, vec![blocker.id])
        .expect("set blocker");

    let error = todos
        .report_completion(
            PROJECT,
            todo.id,
            TodoCompletionKey::for_test(REPORTER, TASK),
            "finished",
            author(),
        )
        .expect_err("an open blocker refuses completion");

    assert!(matches!(error, TodoError::Blocked { by } if by == vec![blocker.id]));
    let stored = repo.read(PROJECT, todo.id).expect("read").expect("todo");
    assert_eq!(stored.doc.status, TodoStatus::InProgress);
    assert!(stored.comments.is_empty());
    assert_eq!(stored.completion, None);
}

#[test]
fn retry_returns_the_same_record_and_notice_disposition() {
    let (repo, todos) = setup();
    let todo = todos
        .create(PROJECT, doc("ship", TodoStatus::InProgress), None)
        .expect("create todo");
    let key = TodoCompletionKey::for_test(REPORTER, TASK);

    let recorded = todos
        .report_completion(PROJECT, todo.id, key, "finished", author())
        .expect("record completion");
    assert_eq!(recorded.occurrence, TodoCompletionOccurrence::Recorded);
    assert_eq!(recorded.notice, TodoCompletionNotice::Required);

    let existing = todos
        .report_completion(PROJECT, todo.id, key, "a spoofed retry", author())
        .expect("idempotent retry");
    assert_eq!(existing.occurrence, TodoCompletionOccurrence::Existing);
    assert_eq!(existing.completion, recorded.completion);
    assert_eq!(existing.notice, TodoCompletionNotice::Required);

    assert_eq!(
        todos
            .mark_completion_notice_queued(PROJECT, &recorded.completion)
            .expect("mark queued"),
        TodoCompletionNoticeOutcome::Recorded
    );
    let after_notice = todos
        .report_completion(PROJECT, todo.id, key, "another retry", author())
        .expect("retry after notice");
    assert_eq!(after_notice.occurrence, TodoCompletionOccurrence::Existing);
    assert_eq!(after_notice.notice, TodoCompletionNotice::AlreadyQueued);
    assert_eq!(
        repo.read(PROJECT, todo.id)
            .expect("read")
            .expect("todo")
            .comments
            .len(),
        1
    );
}

#[test]
fn normal_writes_cannot_reopen_or_change_the_result_comment() {
    let (_repo, todos) = setup();
    let todo = todos
        .create(PROJECT, doc("ship", TodoStatus::InProgress), None)
        .expect("create todo");
    let completion = todos
        .report_completion(
            PROJECT,
            todo.id,
            TodoCompletionKey::for_test(REPORTER, TASK),
            "finished",
            author(),
        )
        .expect("complete")
        .completion;

    assert!(matches!(
        todos.update(
            PROJECT,
            todo.id,
            doc("ship", TodoStatus::Open),
            ScratchpadLink::Unchanged,
            2,
        ),
        Err(TodoError::CompletionProtected)
    ));
    assert_eq!(
        todos
            .comment_update(PROJECT, todo.id, completion.comment(), "changed")
            .expect("protected edit outcome"),
        CommentOutcome::CompletionProtected
    );
    assert_eq!(
        todos
            .comment_delete(PROJECT, todo.id, completion.comment())
            .expect("protected delete outcome"),
        CommentOutcome::CompletionProtected
    );
    let current = todos.get(PROJECT, todo.id).expect("read").expect("todo");
    assert_eq!(current.doc.status, TodoStatus::Done);
    assert_eq!(current.comments[0].body, "finished");
}

#[test]
fn another_task_cannot_claim_an_existing_completion() {
    let (_repo, todos) = setup();
    let todo = todos
        .create(PROJECT, doc("ship", TodoStatus::Open), None)
        .expect("create todo");
    todos
        .report_completion(
            PROJECT,
            todo.id,
            TodoCompletionKey::for_test(REPORTER, TASK),
            "finished",
            author(),
        )
        .expect("first report");

    assert!(matches!(
        todos.report_completion(
            PROJECT,
            todo.id,
            TodoCompletionKey::for_test(REPORTER, AgentMessageId::from_raw(12)),
            "claim",
            author(),
        ),
        Err(TodoError::CompletionConflict)
    ));
}
