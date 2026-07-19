//! In-memory [`TodoRepo`] fake for headless coordination tests, mirroring the SQLite store's todo
//! semantics (atomic revision-guarded doc write, tag/blocker/comment read-modify-write, conditional
//! lock, owner-close and launch lock clearing, and the scratchpad association resolved on read) —
//! no real database. Kept beside the other coordination
//! fakes ([`super::coordination`]) but in its own file to stay within the file-size smell.

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::coordination::FakeScratchpadRepo;

use crate::coordination::{
    Comment, CommentAuthor, CommentEdit, ScratchpadLink, ScratchpadRef, StoredTodo, TodoDoc,
    TodoRepo, TodoStatus, TodoWriteResult,
};
use crate::ids::{ProcessId, ProjectId, ScratchpadId, TodoId};
use crate::ports::StoreError;
use crate::sync::lock;

/// An in-memory [`TodoRepo`] for headless coordination tests. Keyed by todo id like the durable
/// table, assigns durable ids from a counter (never reused), and mirrors the store's atomic
/// revision-guarded doc write, tag/blocker/comment read-modify-write, conditional lock, and
/// owner-close/launch lock clearing under one lock.
#[derive(Default)]
pub struct FakeTodoRepo {
    rows: Mutex<HashMap<u64, StoredTodo>>,
    next_id: AtomicU64,
    scratchpads: Option<Arc<FakeScratchpadRepo>>,
}

impl FakeTodoRepo {
    /// A repo with no scratchpads to resolve against — every association reads back unlinked, which
    /// is all a test that never links one needs.
    pub fn new() -> Self {
        Self::default()
    }

    /// A repo that resolves associations against `scratchpads`, standing in for the durable
    /// adapter's join: a todo linked to a scratchpad in that store reads its current handle back
    /// (so a rename follows), and one whose scratchpad has since been deleted reads back unlinked.
    pub fn joined(scratchpads: Arc<FakeScratchpadRepo>) -> Self {
        Self {
            scratchpads: Some(scratchpads),
            ..Self::default()
        }
    }

    /// Re-resolves a row's association at read time, exactly where the adapter's outer join does.
    /// Only the id is really persisted, so the handle is always current and a scratchpad that is
    /// gone leaves the todo unlinked.
    fn projected(&self, mut todo: StoredTodo) -> StoredTodo {
        todo.scratchpad = todo.scratchpad.and_then(|link| {
            self.scratchpads
                .as_ref()?
                .name_of(link.id)
                .map(|name| ScratchpadRef { id: link.id, name })
        });
        todo
    }

    /// The stored form of a stated association: the id alone, with the handle filled in on read.
    fn stored_link(scratchpad: Option<ScratchpadId>) -> Option<ScratchpadRef> {
        scratchpad.map(|id| ScratchpadRef {
            id,
            name: String::new(),
        })
    }
}

impl TodoRepo for FakeTodoRepo {
    fn create(
        &self,
        project: ProjectId,
        doc: &TodoDoc,
        scratchpad: Option<ScratchpadId>,
    ) -> Result<StoredTodo, StoreError> {
        let id = TodoId::from_raw(self.next_id.fetch_add(1, Ordering::Relaxed) + 1);
        let stored = StoredTodo {
            id,
            project,
            doc: doc.clone(),
            tags: Vec::new(),
            blockers: Vec::new(),
            comments: Vec::new(),
            locked_by: None,
            scratchpad: Self::stored_link(scratchpad),
            revision: 1,
        };
        lock(&self.rows).insert(id.get(), stored.clone());
        Ok(self.projected(stored))
    }

    fn read(&self, project: ProjectId, id: TodoId) -> Result<Option<StoredTodo>, StoreError> {
        let found = lock(&self.rows)
            .get(&id.get())
            .filter(|todo| todo.project == project)
            .cloned();
        Ok(found.map(|todo| self.projected(todo)))
    }

    fn list(&self, project: ProjectId) -> Result<Vec<StoredTodo>, StoreError> {
        let mut found: Vec<StoredTodo> = lock(&self.rows)
            .values()
            .filter(|todo| todo.project == project)
            .cloned()
            .collect();
        found.sort_by_key(|todo| todo.id.get());
        Ok(found.into_iter().map(|todo| self.projected(todo)).collect())
    }

    fn write_doc(
        &self,
        project: ProjectId,
        id: TodoId,
        doc: &TodoDoc,
        scratchpad: ScratchpadLink<ScratchpadId>,
        expected: Option<u64>,
    ) -> Result<TodoWriteResult, StoreError> {
        let stated = match scratchpad {
            ScratchpadLink::Unchanged => None,
            ScratchpadLink::Cleared => Some(None),
            ScratchpadLink::Linked(id) => Some(Self::stored_link(Some(id))),
        };
        let mut rows = lock(&self.rows);
        match rows
            .get_mut(&id.get())
            .filter(|todo| todo.project == project)
        {
            Some(todo) => match expected {
                Some(rev) if rev != todo.revision => Ok(TodoWriteResult::Conflict {
                    actual: todo.revision,
                }),
                _ => {
                    todo.doc = doc.clone();
                    if let Some(link) = stated {
                        todo.scratchpad = link;
                    }
                    todo.revision += 1;
                    Ok(TodoWriteResult::Written(Box::new(
                        self.projected(todo.clone()),
                    )))
                }
            },
            None => Ok(TodoWriteResult::NotFound),
        }
    }

    fn delete(&self, project: ProjectId, id: TodoId) -> Result<bool, StoreError> {
        let mut rows = lock(&self.rows);
        if rows
            .get(&id.get())
            .is_some_and(|todo| todo.project == project)
        {
            rows.remove(&id.get());
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn tags(&self, project: ProjectId) -> Result<Vec<String>, StoreError> {
        let distinct: BTreeSet<String> = lock(&self.rows)
            .values()
            .filter(|todo| todo.project == project)
            .flat_map(|todo| todo.tags.iter().cloned())
            .collect();
        Ok(distinct.into_iter().collect())
    }

    fn add_tag(
        &self,
        project: ProjectId,
        id: TodoId,
        tag: &str,
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.mutate(project, id, |todo| {
            if !todo.tags.iter().any(|existing| existing == tag) {
                todo.tags.push(tag.to_owned());
            }
            todo.tags.sort();
        })
    }

    fn remove_tag(
        &self,
        project: ProjectId,
        id: TodoId,
        tag: &str,
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.mutate(project, id, |todo| {
            todo.tags.retain(|existing| existing != tag)
        })
    }

    fn set_blockers(
        &self,
        project: ProjectId,
        id: TodoId,
        blockers: &[TodoId],
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.mutate(project, id, |todo| todo.blockers = blockers.to_vec())
    }

    fn add_blocker(
        &self,
        project: ProjectId,
        id: TodoId,
        blocker: TodoId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.mutate(project, id, |todo| {
            if !todo.blockers.contains(&blocker) {
                todo.blockers.push(blocker);
            }
        })
    }

    fn remove_blocker(
        &self,
        project: ProjectId,
        id: TodoId,
        blocker: TodoId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.mutate(project, id, |todo| todo.blockers.retain(|b| *b != blocker))
    }

    fn unmet_blockers(
        &self,
        project: ProjectId,
        blockers: &[TodoId],
    ) -> Result<Vec<TodoId>, StoreError> {
        let rows = lock(&self.rows);
        Ok(blockers
            .iter()
            .copied()
            .filter(|blocker| {
                rows.get(&blocker.get())
                    .filter(|todo| todo.project == project)
                    .is_some_and(|todo| todo.doc.status != TodoStatus::Done)
            })
            .collect())
    }

    fn lock(
        &self,
        project: ProjectId,
        id: TodoId,
        owner: ProcessId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.mutate(project, id, |todo| {
            if todo.locked_by.is_none() || todo.locked_by == Some(owner) {
                todo.locked_by = Some(owner);
            }
        })
    }

    fn unlock(
        &self,
        project: ProjectId,
        id: TodoId,
        owner: ProcessId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        self.mutate(project, id, |todo| {
            if todo.locked_by == Some(owner) {
                todo.locked_by = None;
            }
        })
    }

    fn comment_create(
        &self,
        project: ProjectId,
        id: TodoId,
        body: &str,
        author: Option<CommentAuthor>,
    ) -> Result<Option<(StoredTodo, u64)>, StoreError> {
        let mut rows = lock(&self.rows);
        let Some(todo) = rows
            .get_mut(&id.get())
            .filter(|todo| todo.project == project)
        else {
            return Ok(None);
        };
        let comment = todo.comments.iter().map(|c| c.id).max().unwrap_or(0) + 1;
        todo.comments.push(Comment {
            id: comment,
            body: body.to_owned(),
            author,
        });
        Ok(Some((self.projected(todo.clone()), comment)))
    }

    fn comment_update(
        &self,
        project: ProjectId,
        id: TodoId,
        comment: u64,
        body: &str,
    ) -> Result<CommentEdit, StoreError> {
        self.edit_comment(project, id, |comments| {
            comments.iter_mut().find(|c| c.id == comment).map(|c| {
                c.body = body.to_owned();
            })
        })
    }

    fn comment_delete(
        &self,
        project: ProjectId,
        id: TodoId,
        comment: u64,
    ) -> Result<CommentEdit, StoreError> {
        self.edit_comment(project, id, |comments| {
            let before = comments.len();
            comments.retain(|c| c.id != comment);
            (comments.len() != before).then_some(())
        })
    }

    fn release_owner(&self, owner: ProcessId) -> Result<usize, StoreError> {
        let mut rows = lock(&self.rows);
        let mut released = 0;
        for todo in rows.values_mut() {
            if todo.locked_by == Some(owner) {
                todo.locked_by = None;
                released += 1;
            }
        }
        Ok(released)
    }

    fn clear_locks(&self) -> Result<usize, StoreError> {
        let mut rows = lock(&self.rows);
        let mut cleared = 0;
        for todo in rows.values_mut() {
            if todo.locked_by.take().is_some() {
                cleared += 1;
            }
        }
        Ok(cleared)
    }

    fn transfer(
        &self,
        from: ProjectId,
        to: ProjectId,
        id: TodoId,
    ) -> Result<Option<StoredTodo>, StoreError> {
        let mut rows = lock(&self.rows);
        match rows.get_mut(&id.get()).filter(|todo| todo.project == from) {
            Some(todo) => {
                // Re-key the project; blockers and the scratchpad association reference
                // source-project rows and the lock is per-run/process-owned, so all three are
                // cleared. Doc, tags, comments, revision, id stay.
                todo.project = to;
                todo.blockers.clear();
                todo.locked_by = None;
                todo.scratchpad = None;
                Ok(Some(self.projected(todo.clone())))
            }
            None => Ok(None),
        }
    }
}

impl FakeTodoRepo {
    /// Applies `change` to the todo `(project, id)` under one lock and returns the updated row, or
    /// `None` if it does not exist — the shared read-modify-write the tag/blocker/lock methods use.
    fn mutate(
        &self,
        project: ProjectId,
        id: TodoId,
        change: impl FnOnce(&mut StoredTodo),
    ) -> Result<Option<StoredTodo>, StoreError> {
        let mut rows = lock(&self.rows);
        match rows
            .get_mut(&id.get())
            .filter(|todo| todo.project == project)
        {
            Some(todo) => {
                change(todo);
                Ok(Some(self.projected(todo.clone())))
            }
            None => Ok(None),
        }
    }

    /// Applies `edit` to the todo's comment list; `edit` returns `Some(())` when it changed a comment
    /// and `None` when no comment matched, mapped to the [`CommentEdit`] outcome.
    fn edit_comment(
        &self,
        project: ProjectId,
        id: TodoId,
        edit: impl FnOnce(&mut Vec<Comment>) -> Option<()>,
    ) -> Result<CommentEdit, StoreError> {
        let mut rows = lock(&self.rows);
        let Some(todo) = rows
            .get_mut(&id.get())
            .filter(|todo| todo.project == project)
        else {
            return Ok(CommentEdit::NoTodo);
        };
        match edit(&mut todo.comments) {
            Some(()) => Ok(CommentEdit::Edited(Box::new(self.projected(todo.clone())))),
            None => Ok(CommentEdit::NoComment),
        }
    }
}
