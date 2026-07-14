use std::sync::Arc;

use super::*;
use crate::ids::{ProcessId, ProjectId};
use crate::testing::FakeTodoRepo;

const PROJECT: ProjectId = ProjectId::from_raw(1);

fn todos() -> Todos {
    Todos::new(Arc::new(FakeTodoRepo::new()))
}

fn doc(title: &str, status: TodoStatus) -> TodoDoc {
    TodoDoc {
        title: title.into(),
        description: "do the thing".into(),
        acceptance_criteria: vec!["it works".into()],
        risks: vec!["none identified".into()],
        status,
    }
}

#[test]
fn create_then_get_round_trips_the_document() {
    let todos = todos();
    let created = todos
        .create(PROJECT, doc("ship", TodoStatus::Open))
        .expect("create succeeds");
    assert_eq!(created.revision, 1);
    assert_eq!(created.doc.title, "ship");
    assert!(!created.blocked);

    let fetched = todos
        .get(PROJECT, created.id)
        .expect("get succeeds")
        .expect("the todo exists");
    assert_eq!(fetched, created);
}

#[test]
fn a_blank_required_field_is_rejected_naming_every_problem() {
    let todos = todos();
    let bad = TodoDoc {
        title: "  ".into(),
        description: String::new(),
        acceptance_criteria: vec!["  ".into()],
        risks: Vec::new(),
        status: TodoStatus::Open,
    };
    let err = todos
        .create(PROJECT, bad)
        .expect_err("a blank doc is refused");
    let TodoError::Invalid(message) = err else {
        panic!("expected an Invalid error, got {err:?}");
    };
    assert!(message.contains("title"), "{message}");
    assert!(message.contains("description"), "{message}");
    assert!(message.contains("acceptance_criteria"), "{message}");
    assert!(message.contains("risks"), "{message}");
}

#[test]
fn a_document_over_the_byte_cap_is_rejected() {
    let todos = todos();
    let mut oversized = doc("ship", TodoStatus::Open);
    oversized.description = "x".repeat(MAX_TODO_DOC_BYTES + 1);
    let err = todos
        .create(PROJECT, oversized)
        .expect_err("a document past the cap is refused");
    let TodoError::Invalid(message) = err else {
        panic!("expected an Invalid error, got {err:?}");
    };
    assert!(message.contains("exceeds"), "{message}");
}

#[test]
fn update_is_revision_guarded() {
    let todos = todos();
    let todo = todos
        .create(PROJECT, doc("v1", TodoStatus::Open))
        .expect("create");

    // A stale revision is refused and changes nothing.
    let stale = todos
        .update(PROJECT, todo.id, doc("v2", TodoStatus::InProgress), 99)
        .expect_err("a stale update is refused");
    assert!(matches!(
        stale,
        TodoError::Conflict {
            expected: Some(99),
            actual: Some(1)
        }
    ));

    // The current revision applies and bumps.
    let updated = todos
        .update(PROJECT, todo.id, doc("v2", TodoStatus::InProgress), 1)
        .expect("a current-revision update applies");
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.doc.title, "v2");
}

#[test]
fn a_todo_stays_gated_until_its_blocker_completes() {
    let todos = todos();
    let blocker = todos
        .create(PROJECT, doc("dependency", TodoStatus::Open))
        .expect("create blocker");
    let gated = todos
        .create(PROJECT, doc("dependent", TodoStatus::Open))
        .expect("create dependent");
    todos
        .set_blockers(PROJECT, gated.id, vec![blocker.id])
        .expect("set the blocker");

    // The dependent reads as blocked, and completion is refused while the blocker is open.
    let view = todos.get(PROJECT, gated.id).unwrap().unwrap();
    assert!(view.blocked);
    assert_eq!(view.blocked_by, vec![blocker.id]);
    let refused = todos
        .complete(PROJECT, gated.id)
        .expect_err("a blocked todo cannot complete");
    assert!(matches!(refused, TodoError::Blocked { by } if by == vec![blocker.id]));

    // Completing the blocker lifts the gate.
    todos
        .complete(PROJECT, blocker.id)
        .expect("complete blocker");
    let now = todos.get(PROJECT, gated.id).unwrap().unwrap();
    assert!(!now.blocked, "the gate lifts once the blocker is done");
    let done = todos
        .complete(PROJECT, gated.id)
        .expect("the dependent can now complete");
    assert_eq!(done.doc.status, TodoStatus::Done);
}

#[test]
fn a_deleted_blocker_does_not_deadlock_a_todo() {
    let todos = todos();
    let blocker = todos
        .create(PROJECT, doc("gone", TodoStatus::Open))
        .unwrap();
    let gated = todos
        .create(PROJECT, doc("free", TodoStatus::Open))
        .unwrap();
    todos
        .set_blockers(PROJECT, gated.id, vec![blocker.id])
        .unwrap();

    assert!(todos.delete(PROJECT, blocker.id).unwrap());

    // A blocker that no longer exists counts as met — the todo is not stuck forever.
    let view = todos.get(PROJECT, gated.id).unwrap().unwrap();
    assert!(!view.blocked);
    todos
        .complete(PROJECT, gated.id)
        .expect("a todo with only deleted blockers can complete");
}

#[test]
fn a_blocker_must_exist_and_not_be_the_todo_itself() {
    let todos = todos();
    let todo = todos
        .create(PROJECT, doc("self", TodoStatus::Open))
        .unwrap();

    let self_block = todos
        .add_blocker(PROJECT, todo.id, todo.id)
        .expect_err("a todo cannot block itself");
    assert!(matches!(self_block, TodoError::SelfBlocker));

    let missing = todos
        .add_blocker(PROJECT, todo.id, TodoId::from_raw(9999))
        .expect_err("a non-existent blocker is refused");
    assert!(matches!(missing, TodoError::UnknownBlocker));
}

#[test]
fn tags_are_idempotent_and_listed_distinct() {
    let todos = todos();
    let todo = todos
        .create(PROJECT, doc("tagged", TodoStatus::Open))
        .unwrap();
    todos.add_tag(PROJECT, todo.id, "p1").unwrap();
    todos.add_tag(PROJECT, todo.id, "p1").unwrap();
    let view = todos.add_tag(PROJECT, todo.id, "ui").unwrap().unwrap();
    assert_eq!(view.tags, vec!["p1".to_string(), "ui".to_string()]);

    assert_eq!(
        todos.tags(PROJECT).unwrap(),
        vec!["p1".to_string(), "ui".to_string()]
    );

    let removed = todos.remove_tag(PROJECT, todo.id, "p1").unwrap().unwrap();
    assert_eq!(removed.tags, vec!["ui".to_string()]);
}

#[test]
fn a_lock_is_a_signal_and_releases_when_the_owner_closes() {
    let todos = todos();
    let todo = todos
        .create(PROJECT, doc("locked", TodoStatus::Open))
        .unwrap();
    let alice = ProcessId::from_raw(10);
    let bob = ProcessId::from_raw(20);

    let held = todos.lock(PROJECT, todo.id, alice).unwrap().unwrap();
    assert_eq!(held.locked_by, Some(alice));

    // Another process cannot steal it — the view reports the current holder.
    let contested = todos.lock(PROJECT, todo.id, bob).unwrap().unwrap();
    assert_eq!(contested.locked_by, Some(alice));

    // The owner closing releases it (the supervisor's close hook).
    assert_eq!(todos.release_owner(alice).unwrap(), 1);
    let freed = todos.get(PROJECT, todo.id).unwrap().unwrap();
    assert_eq!(freed.locked_by, None);
}

#[test]
fn reconcile_clears_locks_but_keeps_the_todos() {
    let todos = todos();
    let todo = todos
        .create(PROJECT, doc("survivor", TodoStatus::Open))
        .unwrap();
    todos
        .lock(PROJECT, todo.id, ProcessId::from_raw(7))
        .unwrap();

    assert_eq!(todos.reconcile().unwrap(), 1);
    let after = todos
        .get(PROJECT, todo.id)
        .unwrap()
        .expect("the todo survives reconcile");
    assert_eq!(after.locked_by, None, "only the lock is cleared");
}

#[test]
fn comments_create_update_delete_and_list() {
    let todos = todos();
    let todo = todos
        .create(PROJECT, doc("discuss", TodoStatus::Open))
        .unwrap();

    let (_, first) = todos
        .comment_create(PROJECT, todo.id, "looks good", None)
        .unwrap()
        .unwrap();
    let (_, second) = todos
        .comment_create(PROJECT, todo.id, "ship it", None)
        .unwrap()
        .unwrap();
    assert_ne!(first, second);

    let edited = todos
        .comment_update(PROJECT, todo.id, first, "looks great")
        .unwrap();
    assert!(matches!(edited, CommentOutcome::Edited(_)));

    let missing = todos
        .comment_update(PROJECT, todo.id, 999, "ghost")
        .unwrap();
    assert!(matches!(missing, CommentOutcome::NoComment));

    todos.comment_delete(PROJECT, todo.id, second).unwrap();
    let remaining = todos
        .comment_list(PROJECT, todo.id)
        .unwrap()
        .expect("the todo exists");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].body, "looks great");
}

#[test]
fn a_comment_carries_the_author_it_was_stamped_with() {
    let todos = todos();
    let todo = todos
        .create(PROJECT, doc("discuss", TodoStatus::Open))
        .unwrap();
    let author = CommentAuthor::Process {
        id: ProcessId::from_raw(2),
        label: "Web".into(),
    };
    let (view, _) = todos
        .comment_create(PROJECT, todo.id, "looks good", Some(author.clone()))
        .unwrap()
        .expect("the todo exists");
    assert_eq!(view.comments[0].author, Some(author));
}

#[test]
fn a_comment_persisted_before_authorship_reads_back_unattributed() {
    // The author field is `#[serde(default)]`, so a comment written before authorship existed
    // deserializes with no author rather than failing — no migration is needed.
    let legacy: Comment =
        serde_json::from_str(r#"{"id":1,"body":"shipped"}"#).expect("legacy json");
    assert_eq!(legacy.author, None);
    assert_eq!(legacy.body, "shipped");
}
