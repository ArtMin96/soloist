use soloist_core::{
    ProjectId, StoreError, TodoCompletion, TodoCompletionAtomicResult, TodoCompletionCompareResult,
    TodoCompletionContext, TodoCompletionDecision, TodoId,
};

use crate::todo_json::{serialize_comments, serialize_doc};
use crate::todo_rows::read_one;
use crate::{sql_err, SqliteStore};

pub(super) fn apply_completion(
    store: &SqliteStore,
    project: ProjectId,
    id: TodoId,
    policy: &mut dyn FnMut(&TodoCompletionContext) -> TodoCompletionDecision,
) -> Result<TodoCompletionAtomicResult, StoreError> {
    let mut conn = store.lock();
    let transaction = conn.transaction().map_err(sql_err)?;
    let Some(mut stored) = read_one(&transaction, project, id)? else {
        return Ok(TodoCompletionAtomicResult::NotFound);
    };
    let mut blockers = Vec::new();
    for blocker in &stored.blockers {
        if let Some(row) = read_one(&transaction, project, *blocker)? {
            blockers.push(row);
        }
    }
    match policy(&TodoCompletionContext::from_storage(
        stored.clone(),
        blockers,
    )) {
        TodoCompletionDecision::Existing => {
            Ok(TodoCompletionAtomicResult::Existing(Box::new(stored)))
        }
        TodoCompletionDecision::Refuse(error) => Ok(TodoCompletionAtomicResult::Refused(error)),
        TodoCompletionDecision::Record(intent) => {
            stored.doc = intent.doc().clone();
            stored.comments.push(intent.comment().clone());
            stored.completion = Some(intent.completion().clone());
            stored.revision += 1;
            transaction
                .execute(
                    "UPDATE todos SET doc = ?3, comments = ?4, revision = ?5, completion = ?6
                     WHERE project_id = ?1 AND id = ?2",
                    (
                        project.get() as i64,
                        id.get() as i64,
                        serialize_doc(&stored.doc)?,
                        serialize_comments(&stored.comments)?,
                        stored.revision as i64,
                        serialize_completion(intent.completion())?,
                    ),
                )
                .map_err(sql_err)?;
            transaction.commit().map_err(sql_err)?;
            Ok(TodoCompletionAtomicResult::Recorded(Box::new(stored)))
        }
    }
}

pub(super) fn compare_completion(
    store: &SqliteStore,
    project: ProjectId,
    id: TodoId,
    expected: &TodoCompletion,
    replacement: &TodoCompletion,
) -> Result<TodoCompletionCompareResult, StoreError> {
    let mut conn = store.lock();
    let transaction = conn.transaction().map_err(sql_err)?;
    let Some(mut stored) = read_one(&transaction, project, id)? else {
        return Ok(TodoCompletionCompareResult::NotFound);
    };
    if stored.completion.as_ref() != Some(expected) {
        return Ok(TodoCompletionCompareResult::Mismatch(Box::new(stored)));
    }
    transaction
        .execute(
            "UPDATE todos SET completion = ?3 WHERE project_id = ?1 AND id = ?2",
            (
                project.get() as i64,
                id.get() as i64,
                serialize_completion(replacement)?,
            ),
        )
        .map_err(sql_err)?;
    transaction.commit().map_err(sql_err)?;
    stored.completion = Some(replacement.clone());
    Ok(TodoCompletionCompareResult::Written(Box::new(stored)))
}

fn serialize_completion(completion: &TodoCompletion) -> Result<String, StoreError> {
    serde_json::to_string(completion)
        .map_err(|error| StoreError::Backend(format!("serialize todo completion: {error}")))
}

#[cfg(test)]
#[path = "completion_tests.rs"]
mod tests;
