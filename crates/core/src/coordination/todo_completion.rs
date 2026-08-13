//! Atomic, identity-bound completion reporting for coordination todos.

use serde::{Deserialize, Serialize};

use super::todo_comment::{Comment, CommentAuthor};
use super::todo_doc::TodoStatus;
use super::todo_repo::{
    StoredTodo, TodoCompletionAtomicResult, TodoCompletionContext, TodoCompletionDecision,
    TodoCompletionIntent, TodoRepo,
};
use super::{TodoError, Todos};
use crate::ids::{AgentMessageId, ProcessId, ProjectId, TodoId};
use crate::ports::StoreError;

/// Authenticated identity and task correlation that make one completion report idempotent.
///
/// Construction is crate-private: a wire caller supplies only the task-message id, while the
/// façade supplies the reporter from its authenticated bound session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoCompletionKey {
    reporter: ProcessId,
    task_message: AgentMessageId,
}

impl TodoCompletionKey {
    pub(crate) const fn new(reporter: ProcessId, task_message: AgentMessageId) -> Self {
        Self {
            reporter,
            task_message,
        }
    }

    /// Constructs an authenticated key in a test composition root.
    #[cfg(any(test, feature = "testing"))]
    pub const fn for_test(reporter: ProcessId, task_message: AgentMessageId) -> Self {
        Self::new(reporter, task_message)
    }

    /// The authenticated process that reported the result.
    pub const fn reporter(self) -> ProcessId {
        self.reporter
    }

    /// The addressed task message this report resolves.
    pub const fn task_message(self) -> AgentMessageId {
        self.task_message
    }
}

/// Durable proof that one authenticated task completed one todo with one result comment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoCompletion {
    todo_id: TodoId,
    key: TodoCompletionKey,
    summary: String,
    comment: u64,
    notice_queued: bool,
}

impl TodoCompletion {
    /// The todo changed by this report.
    pub const fn todo_id(&self) -> TodoId {
        self.todo_id
    }

    /// The authenticated idempotency key for this report.
    pub const fn key(&self) -> TodoCompletionKey {
        self.key
    }

    /// The recorded result text.
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// The protected result comment created with the completion.
    pub const fn comment(&self) -> u64 {
        self.comment
    }

    /// Whether a parent completion notice has previously entered the ephemeral mailbox.
    pub const fn notice_queued(&self) -> bool {
        self.notice_queued
    }

    fn with_notice_queued(&self) -> Self {
        Self {
            notice_queued: true,
            ..self.clone()
        }
    }
}

/// Whether this call created the durable result or found the same authenticated report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoCompletionOccurrence {
    Recorded,
    Existing,
}

/// Whether the caller should attempt the best-effort parent notification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoCompletionNotice {
    Required,
    AlreadyQueued,
}

/// Core-owned outcome of an atomic completion report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoCompletionOutcome {
    pub completion: TodoCompletion,
    pub occurrence: TodoCompletionOccurrence,
    pub notice: TodoCompletionNotice,
}

/// Outcome of durably recording that the ephemeral parent notice was queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TodoCompletionNoticeOutcome {
    Recorded,
    AlreadyQueued,
}

impl Todos {
    /// Atomically evaluates blockers and the legal status transition, then records exactly one
    /// identity-authored result for `key`. A retry with the same key returns the existing record;
    /// a different task cannot claim a todo whose completion is already recorded.
    pub fn report_completion(
        &self,
        project: ProjectId,
        id: TodoId,
        key: TodoCompletionKey,
        summary: &str,
        author: CommentAuthor,
    ) -> Result<TodoCompletionOutcome, TodoError> {
        let mut policy = |context: &TodoCompletionContext| {
            completion_decision(context, id, key, summary, &author)
        };
        let (stored, occurrence) = match self.repo.apply_completion(project, id, &mut policy)? {
            TodoCompletionAtomicResult::Recorded(stored) => {
                (*stored, TodoCompletionOccurrence::Recorded)
            }
            TodoCompletionAtomicResult::Existing(stored) => {
                (*stored, TodoCompletionOccurrence::Existing)
            }
            TodoCompletionAtomicResult::Refused(error) => return Err(error),
            TodoCompletionAtomicResult::NotFound => return Err(TodoError::NotFound),
        };
        let completion = stored
            .completion
            .ok_or_else(|| StoreError::Backend("completion write returned no record".into()))?;
        let notice = if completion.notice_queued() {
            TodoCompletionNotice::AlreadyQueued
        } else {
            TodoCompletionNotice::Required
        };
        Ok(TodoCompletionOutcome {
            completion,
            occurrence,
            notice,
        })
    }

    /// Marks a previously returned completion as having queued its parent notice. This is a
    /// compare-write, so a stale or unrelated completion can never mark another task's record.
    pub fn mark_completion_notice_queued(
        &self,
        project: ProjectId,
        completion: &TodoCompletion,
    ) -> Result<TodoCompletionNoticeOutcome, TodoError> {
        if completion.notice_queued() {
            return Ok(TodoCompletionNoticeOutcome::AlreadyQueued);
        }
        let replacement = completion.with_notice_queued();
        match self.repo.compare_completion(
            project,
            completion.todo_id(),
            completion,
            &replacement,
        )? {
            super::TodoCompletionCompareResult::Written(_) => {
                Ok(TodoCompletionNoticeOutcome::Recorded)
            }
            super::TodoCompletionCompareResult::Mismatch(stored) => match stored.completion {
                Some(current) if current.key() == completion.key() && current.notice_queued() => {
                    Ok(TodoCompletionNoticeOutcome::AlreadyQueued)
                }
                _ => Err(TodoError::CompletionConflict),
            },
            super::TodoCompletionCompareResult::NotFound => Err(TodoError::NotFound),
        }
    }
}

fn completion_decision(
    context: &TodoCompletionContext,
    id: TodoId,
    key: TodoCompletionKey,
    summary: &str,
    author: &CommentAuthor,
) -> TodoCompletionDecision {
    if let Some(existing) = &context.todo().completion {
        return if existing.key() == key {
            TodoCompletionDecision::Existing
        } else {
            TodoCompletionDecision::Refuse(TodoError::CompletionConflict)
        };
    }
    let blocked_by: Vec<_> = context
        .blockers()
        .iter()
        .filter(|blocker| blocker.doc.status != TodoStatus::Done)
        .map(|blocker| blocker.id)
        .collect();
    if !blocked_by.is_empty() {
        return TodoCompletionDecision::Refuse(TodoError::Blocked { by: blocked_by });
    }
    let mut doc = context.todo().doc.clone();
    doc.status = doc.status.completed();
    let comment = context
        .todo()
        .comments
        .iter()
        .map(|comment| comment.id)
        .max()
        .unwrap_or(0)
        + 1;
    let result = Comment {
        id: comment,
        body: summary.to_owned(),
        author: Some(author.clone()),
    };
    let completion = TodoCompletion {
        todo_id: id,
        key,
        summary: summary.to_owned(),
        comment,
        notice_queued: false,
    };
    TodoCompletionDecision::Record(TodoCompletionIntent::new(doc, result, completion))
}

#[cfg(test)]
#[path = "todo_completion_tests.rs"]
mod tests;
