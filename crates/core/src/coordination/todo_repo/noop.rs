use super::{
    CommentAuthor, CommentEdit, ProcessId, ProjectId, ScratchpadId, ScratchpadLink, StoreError,
    StoredTodo, TodoCompletion, TodoCompletionAtomicResult, TodoCompletionCompareResult,
    TodoCompletionContext, TodoCompletionDecision, TodoDoc, TodoId, TodoRepo, TodoWriteResult,
};

/// A [`TodoRepo`] that stores nothing — the default until the durable adapter is wired, so the core
/// runs (todos simply never persist) without it. A create echoes a placeholder row back and every
/// read is empty.
#[derive(Clone, Copy, Default)]
pub struct NoopTodoRepo;

impl TodoRepo for NoopTodoRepo {
    fn apply_completion(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _policy: &mut dyn FnMut(&TodoCompletionContext) -> TodoCompletionDecision,
    ) -> Result<TodoCompletionAtomicResult, StoreError> {
        Ok(TodoCompletionAtomicResult::NotFound)
    }

    fn compare_completion(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _expected: &TodoCompletion,
        _replacement: &TodoCompletion,
    ) -> Result<TodoCompletionCompareResult, StoreError> {
        Ok(TodoCompletionCompareResult::NotFound)
    }

    fn create(
        &self,
        project: ProjectId,
        doc: &TodoDoc,
        _scratchpad: Option<ScratchpadId>,
    ) -> Result<StoredTodo, StoreError> {
        Ok(StoredTodo {
            id: TodoId::from_raw(0),
            project,
            doc: doc.clone(),
            tags: Vec::new(),
            blockers: Vec::new(),
            comments: Vec::new(),
            completion: None,
            locked_by: None,
            scratchpad: None,
            revision: 1,
        })
    }

    fn read(&self, _project: ProjectId, _id: TodoId) -> Result<Option<StoredTodo>, StoreError> {
        Ok(None)
    }

    fn list(&self, _project: ProjectId) -> Result<Vec<StoredTodo>, StoreError> {
        Ok(Vec::new())
    }

    fn write_doc(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _doc: &TodoDoc,
        _scratchpad: ScratchpadLink<ScratchpadId>,
        _expected: Option<u64>,
    ) -> Result<TodoWriteResult, StoreError> {
        Ok(TodoWriteResult::NotFound)
    }

    fn delete(&self, _project: ProjectId, _id: TodoId) -> Result<bool, StoreError> {
        Ok(false)
    }

    fn tags(&self, _project: ProjectId) -> Result<Vec<String>, StoreError> {
        Ok(Vec::new())
    }

    fn add_tag(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _tag: &str,
    ) -> Result<Option<StoredTodo>, StoreError> {
        Ok(None)
    }

    fn remove_tag(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _tag: &str,
    ) -> Result<Option<StoredTodo>, StoreError> {
        Ok(None)
    }

    fn set_blockers(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _blockers: &[TodoId],
    ) -> Result<Option<StoredTodo>, StoreError> {
        Ok(None)
    }

    fn add_blocker(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _blocker: TodoId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        Ok(None)
    }

    fn remove_blocker(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _blocker: TodoId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        Ok(None)
    }

    fn unmet_blockers(
        &self,
        _project: ProjectId,
        _blockers: &[TodoId],
    ) -> Result<Vec<TodoId>, StoreError> {
        Ok(Vec::new())
    }

    fn lock(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _owner: ProcessId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        Ok(None)
    }

    fn unlock(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _owner: ProcessId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        Ok(None)
    }

    fn comment_create(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _body: &str,
        _author: Option<CommentAuthor>,
    ) -> Result<Option<(StoredTodo, u64)>, StoreError> {
        Ok(None)
    }

    fn comment_update(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _comment: u64,
        _body: &str,
    ) -> Result<CommentEdit, StoreError> {
        Ok(CommentEdit::NoTodo)
    }

    fn comment_delete(
        &self,
        _project: ProjectId,
        _id: TodoId,
        _comment: u64,
    ) -> Result<CommentEdit, StoreError> {
        Ok(CommentEdit::NoTodo)
    }

    fn release_owner(&self, _process: ProcessId) -> Result<usize, StoreError> {
        Ok(0)
    }

    fn clear_locks(&self) -> Result<usize, StoreError> {
        Ok(0)
    }

    fn transfer(
        &self,
        _from: ProjectId,
        _to: ProjectId,
        _id: TodoId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        Ok(None)
    }
}
