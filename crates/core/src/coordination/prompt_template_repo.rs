//! The durable store of prompt templates and the port over it (context C6).
//!
//! One row per template, addressed by name within a scope: `project = None` is the global
//! scope, `Some(id)` a project's. Writes are revision-guarded like scratchpads —
//! `expected = None` creates, `Some(rev)` updates — and the whole check-and-write is one
//! atomic step in the adapter, so two concurrent writers can never both win.

use serde::{Deserialize, Serialize};

use crate::ids::{ProjectId, PromptTemplateId};
use crate::ports::StoreError;

/// A persisted prompt template row, exactly as stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredPromptTemplate {
    pub id: PromptTemplateId,
    /// The owning project, or `None` for the global scope.
    pub project: Option<ProjectId>,
    pub name: String,
    pub description: Option<String>,
    pub body: String,
    pub revision: u64,
}

/// How a revision-guarded template write resolved: applied, or refused with the revision
/// actually on record (`None` when no template exists under that name).
#[derive(Debug)]
pub enum PromptTemplateWriteResult {
    Written(Box<StoredPromptTemplate>),
    Conflict { actual: Option<u64> },
}

/// Durable prompt-template repository — one focused trait, SQLite behind it.
pub trait PromptTemplateRepo: Send + Sync {
    /// Creates (`expected = None`) or updates (`expected = Some(rev)`) the template `name` in
    /// the scope, atomically checking the revision; a mismatch changes nothing.
    fn write(
        &self,
        project: Option<ProjectId>,
        name: &str,
        description: Option<&str>,
        body: &str,
        expected: Option<u64>,
    ) -> Result<PromptTemplateWriteResult, StoreError>;

    /// The template `name` in the scope, or `None`.
    fn read(
        &self,
        project: Option<ProjectId>,
        name: &str,
    ) -> Result<Option<StoredPromptTemplate>, StoreError>;

    /// Every template in the scope, ordered by name.
    fn list(&self, project: Option<ProjectId>) -> Result<Vec<StoredPromptTemplate>, StoreError>;

    /// Removes the template `name` from the scope, returning whether one was present.
    fn delete(&self, project: Option<ProjectId>, name: &str) -> Result<bool, StoreError>;
}

/// A [`PromptTemplateRepo`] that stores nothing — the default until the durable adapter is
/// wired. A create echoes a placeholder row; reads are empty.
#[derive(Clone, Copy, Default)]
pub struct NoopPromptTemplateRepo;

impl PromptTemplateRepo for NoopPromptTemplateRepo {
    fn write(
        &self,
        project: Option<ProjectId>,
        name: &str,
        description: Option<&str>,
        body: &str,
        expected: Option<u64>,
    ) -> Result<PromptTemplateWriteResult, StoreError> {
        if expected.is_some() {
            // Nothing is ever stored, so an update can only be stale.
            return Ok(PromptTemplateWriteResult::Conflict { actual: None });
        }
        Ok(PromptTemplateWriteResult::Written(Box::new(
            StoredPromptTemplate {
                id: PromptTemplateId::from_raw(0),
                project,
                name: name.to_owned(),
                description: description.map(str::to_owned),
                body: body.to_owned(),
                revision: 1,
            },
        )))
    }

    fn read(
        &self,
        _project: Option<ProjectId>,
        _name: &str,
    ) -> Result<Option<StoredPromptTemplate>, StoreError> {
        Ok(None)
    }

    fn list(&self, _project: Option<ProjectId>) -> Result<Vec<StoredPromptTemplate>, StoreError> {
        Ok(Vec::new())
    }

    fn delete(&self, _project: Option<ProjectId>, _name: &str) -> Result<bool, StoreError> {
        Ok(false)
    }
}
