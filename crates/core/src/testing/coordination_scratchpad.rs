//! In-memory [`ScratchpadRepo`] fake for headless coordination tests, mirroring the SQLite store's
//! scratchpad semantics (atomic revision-guarded write, rename uniqueness, tag read-modify-write,
//! and the cross-project transfer's cascade into derived todos) — no real database. Kept beside the
//! other coordination fakes ([`super::coordination`]) but in its own file to stay within the
//! file-size smell.

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::coordination_todo::FakeTodoRows;

use crate::coordination::{
    RenameResult, ScratchpadRepo, StoredScratchpad, TransferResult, TransferredScratchpad,
    WriteResult,
};
use crate::ids::{ProjectId, ScratchpadId, TodoId};
use crate::ports::StoreError;
use crate::sync::lock;

/// An in-memory [`ScratchpadRepo`] for headless coordination tests. Keyed by `(project, name)` like
/// the durable table, assigns durable ids from a counter, and mirrors the store's atomic
/// revision-guarded write, rename uniqueness, tag read-modify-write, and the transfer's cascade
/// into derived todos under one lock.
#[derive(Default)]
pub struct FakeScratchpadRepo {
    rows: Mutex<HashMap<(u64, String), StoredScratchpad>>,
    next_id: AtomicU64,
    todos: FakeTodoRows,
}

impl FakeScratchpadRepo {
    pub fn new() -> Self {
        Self::default()
    }

    /// The current handle of the scratchpad `id`, or `None` when no such scratchpad exists — the
    /// lookup [`FakeTodoRepo`](super::FakeTodoRepo) resolves a todo's association through, standing
    /// in for the durable adapter's join.
    pub fn name_of(&self, id: ScratchpadId) -> Option<String> {
        lock(&self.rows)
            .values()
            .find(|stored| stored.id == id)
            .map(|stored| stored.name.clone())
    }

    /// The todo rows a [`transfer`](ScratchpadRepo::transfer) cascades into. Handed to the
    /// [`FakeTodoRepo`](super::FakeTodoRepo) built beside this one so both fakes see one set of
    /// todos, as the durable adapter's two tables do.
    pub fn todo_rows(&self) -> FakeTodoRows {
        Arc::clone(&self.todos)
    }

    /// Moves the todos derived from `scratchpad` out of `from` and into `to`, mirroring the durable
    /// adapter's cascade: the association is kept (both ends move), the process-owned lock is
    /// dropped, and a blocker naming a todo left behind is dropped while one naming a todo that
    /// moves too survives. Returns the moved ids in id order.
    fn cascade_todos(
        &self,
        from: ProjectId,
        to: ProjectId,
        scratchpad: ScratchpadId,
    ) -> Vec<TodoId> {
        let mut todos = lock(&self.todos);
        let mut moved: Vec<TodoId> = todos
            .values()
            .filter(|todo| {
                todo.project == from
                    && todo
                        .scratchpad
                        .as_ref()
                        .is_some_and(|it| it.id == scratchpad)
            })
            .map(|todo| todo.id)
            .collect();
        moved.sort_by_key(|id| id.get());
        for id in &moved {
            if let Some(todo) = todos.get_mut(&id.get()) {
                todo.project = to;
                todo.locked_by = None;
                todo.blockers.retain(|blocker| moved.contains(blocker));
            }
        }
        moved
    }
}

impl ScratchpadRepo for FakeScratchpadRepo {
    fn write(
        &self,
        project: ProjectId,
        name: &str,
        body: &str,
        expected: Option<u64>,
        now: u64,
    ) -> Result<WriteResult, StoreError> {
        let mut rows = lock(&self.rows);
        let slot = (project.get(), name.to_owned());
        match rows.get(&slot) {
            Some(existing) => match expected {
                Some(rev) if rev == existing.revision => {
                    let mut updated = existing.clone();
                    updated.body = body.to_owned();
                    updated.revision = existing.revision + 1;
                    updated.updated_at = now;
                    rows.insert(slot, updated.clone());
                    Ok(WriteResult::Written(Box::new(updated)))
                }
                _ => Ok(WriteResult::Conflict {
                    actual: Some(existing.revision),
                }),
            },
            None => match expected {
                None => {
                    let id =
                        ScratchpadId::from_raw(self.next_id.fetch_add(1, Ordering::Relaxed) + 1);
                    let stored = StoredScratchpad {
                        id,
                        project,
                        name: name.to_owned(),
                        body: body.to_owned(),
                        tags: Vec::new(),
                        archived: false,
                        revision: 1,
                        updated_at: now,
                    };
                    rows.insert(slot, stored.clone());
                    Ok(WriteResult::Written(Box::new(stored)))
                }
                Some(_) => Ok(WriteResult::Conflict { actual: None }),
            },
        }
    }

    fn read(&self, project: ProjectId, name: &str) -> Result<Option<StoredScratchpad>, StoreError> {
        Ok(lock(&self.rows)
            .get(&(project.get(), name.to_owned()))
            .cloned())
    }

    fn list(&self, project: ProjectId) -> Result<Vec<StoredScratchpad>, StoreError> {
        let mut found: Vec<StoredScratchpad> = lock(&self.rows)
            .values()
            .filter(|row| row.project == project)
            .cloned()
            .collect();
        found.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(found)
    }

    fn rename(&self, project: ProjectId, from: &str, to: &str) -> Result<RenameResult, StoreError> {
        let mut rows = lock(&self.rows);
        let to_slot = (project.get(), to.to_owned());
        if from != to && rows.contains_key(&to_slot) {
            return Ok(RenameResult::NameTaken);
        }
        match rows.remove(&(project.get(), from.to_owned())) {
            Some(mut stored) => {
                stored.name = to.to_owned();
                rows.insert(to_slot, stored.clone());
                Ok(RenameResult::Renamed(Box::new(stored)))
            }
            None => Ok(RenameResult::NotFound),
        }
    }

    fn transfer(
        &self,
        from: ProjectId,
        name: &str,
        to: ProjectId,
    ) -> Result<TransferResult, StoreError> {
        // The scratchpad rows are released before the todo rows are taken, so this fake never
        // inverts the lock order a read-side association lookup takes them in.
        let stored = {
            let mut rows = lock(&self.rows);
            let to_slot = (to.get(), name.to_owned());
            if rows.contains_key(&to_slot) {
                return Ok(TransferResult::NameTaken);
            }
            match rows.remove(&(from.get(), name.to_owned())) {
                // Re-key the project only; the durable id, name, body, tags, archived, revision stay.
                Some(mut stored) => {
                    stored.project = to;
                    rows.insert(to_slot, stored.clone());
                    stored
                }
                None => return Ok(TransferResult::NotFound),
            }
        };
        let todos = self.cascade_todos(from, to, stored.id);
        Ok(TransferResult::Transferred(Box::new(
            TransferredScratchpad {
                scratchpad: stored,
                todos,
            },
        )))
    }

    fn add_tags(
        &self,
        project: ProjectId,
        name: &str,
        tags: &[String],
    ) -> Result<Option<StoredScratchpad>, StoreError> {
        let mut rows = lock(&self.rows);
        match rows.get_mut(&(project.get(), name.to_owned())) {
            Some(stored) => {
                for tag in tags {
                    if !stored.tags.contains(tag) {
                        stored.tags.push(tag.clone());
                    }
                }
                stored.tags.sort();
                Ok(Some(stored.clone()))
            }
            None => Ok(None),
        }
    }

    fn remove_tags(
        &self,
        project: ProjectId,
        name: &str,
        tags: &[String],
    ) -> Result<Option<StoredScratchpad>, StoreError> {
        let mut rows = lock(&self.rows);
        match rows.get_mut(&(project.get(), name.to_owned())) {
            Some(stored) => {
                stored.tags.retain(|tag| !tags.contains(tag));
                stored.tags.sort();
                Ok(Some(stored.clone()))
            }
            None => Ok(None),
        }
    }

    fn tags(&self, project: ProjectId) -> Result<Vec<String>, StoreError> {
        let distinct: BTreeSet<String> = lock(&self.rows)
            .values()
            .filter(|row| row.project == project)
            .flat_map(|row| row.tags.iter().cloned())
            .collect();
        Ok(distinct.into_iter().collect())
    }

    fn set_archived(
        &self,
        project: ProjectId,
        name: &str,
        archived: bool,
    ) -> Result<Option<StoredScratchpad>, StoreError> {
        let mut rows = lock(&self.rows);
        match rows.get_mut(&(project.get(), name.to_owned())) {
            Some(stored) => {
                stored.archived = archived;
                Ok(Some(stored.clone()))
            }
            None => Ok(None),
        }
    }

    fn delete(&self, project: ProjectId, name: &str) -> Result<bool, StoreError> {
        Ok(lock(&self.rows)
            .remove(&(project.get(), name.to_owned()))
            .is_some())
    }
}
