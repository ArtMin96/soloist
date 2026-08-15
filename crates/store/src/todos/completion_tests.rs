use std::path::Path;
use std::sync::{Arc, Barrier};

use soloist_core::{
    AgentMessageId, CommentAuthor, ProcessId, ProjectId, ProjectRepo, TodoCompletionKey,
    TodoCompletionOccurrence, TodoDoc, TodoError, TodoRepo, TodoStatus, Todos,
};

use super::*;

const REPORTER: ProcessId = ProcessId::from_raw(7);
const TASK: AgentMessageId = AgentMessageId::from_raw(11);

fn setup() -> (Arc<SqliteStore>, ProjectId, Todos) {
    let store = Arc::new(SqliteStore::open_in_memory().expect("open"));
    let project = store
        .upsert(Path::new("/completion"), None, None)
        .expect("project")
        .id;
    let todos = Todos::new(store.clone());
    (store, project, todos)
}

fn author() -> CommentAuthor {
    CommentAuthor::Process {
        id: REPORTER,
        label: "worker".into(),
    }
}

fn create(todos: &Todos, project: ProjectId) -> TodoId {
    todos
        .create(
            project,
            TodoDoc {
                title: "ship".into(),
                body: "do it".into(),
                status: TodoStatus::InProgress,
            },
            None,
        )
        .expect("create")
        .id
}

#[test]
fn concurrent_retries_record_one_result() {
    let (store, project, todos) = setup();
    let todo = create(&todos, project);
    let todos = Arc::new(todos);
    const WRITERS: usize = 12;
    let barrier = Arc::new(Barrier::new(WRITERS));
    let handles: Vec<_> = (0..WRITERS)
        .map(|_| {
            let todos = todos.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                todos
                    .report_completion(
                        project,
                        todo,
                        TodoCompletionKey::for_test(REPORTER, TASK),
                        "finished",
                        author(),
                    )
                    .expect("report")
                    .occurrence
            })
        })
        .collect();
    let outcomes: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("join"))
        .collect();
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| **outcome == TodoCompletionOccurrence::Recorded)
            .count(),
        1
    );
    let stored = store.read(project, todo).expect("read").expect("todo");
    assert_eq!(stored.comments.len(), 1);
    assert_eq!(stored.revision, 2);
}

#[test]
fn persistence_failure_rolls_back_status_comment_and_record() {
    let (store, project, todos) = setup();
    let todo = create(&todos, project);
    store
        .lock()
        .execute_batch(
            "CREATE TRIGGER refuse_completion BEFORE UPDATE OF completion ON todos
             BEGIN SELECT RAISE(FAIL, 'injected completion failure'); END;",
        )
        .expect("install failure");

    assert!(matches!(
        todos.report_completion(
            project,
            todo,
            TodoCompletionKey::for_test(REPORTER, TASK),
            "finished",
            author(),
        ),
        Err(TodoError::Store(_))
    ));
    let stored = store.read(project, todo).expect("read").expect("todo");
    assert_eq!(stored.doc.status, TodoStatus::InProgress);
    assert!(stored.comments.is_empty());
    assert_eq!(stored.completion, None);
    assert_eq!(stored.revision, 1);
}
