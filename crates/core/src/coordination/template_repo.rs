//! The durable store of templates and the port over it (context C6).
//!
//! One row per template, addressed by name within a `(kind, scope)`: `project = None` is the
//! global scope, `Some(id)` a project's, and the [`TemplateKind`] separates a prompt from a
//! scratchpad or todo starting shape. Writes are revision-guarded like scratchpads —
//! `expected = None` creates, `Some(rev)` updates — and the whole check-and-write is one atomic
//! step in the adapter, so two concurrent writers can never both win.

use serde::{Deserialize, Serialize};

use crate::ids::{ProjectId, TemplateId};
use crate::ports::StoreError;
use crate::template::TemplateKind;

/// A persisted template row, exactly as stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredTemplate {
    pub id: TemplateId,
    pub kind: TemplateKind,
    /// The owning project, or `None` for the global scope.
    pub project: Option<ProjectId>,
    pub name: String,
    pub description: Option<String>,
    pub body: String,
    pub revision: u64,
}

/// How a revision-guarded template write resolved: applied, or refused with the revision actually
/// on record (`None` when no template exists under that `(kind, scope, name)`).
#[derive(Debug)]
pub enum TemplateWriteResult {
    Written(Box<StoredTemplate>),
    Conflict { actual: Option<u64> },
}

/// Durable template repository — one focused trait, SQLite behind it. Every method carries the
/// [`TemplateKind`], so one table serves prompts, scratchpad shapes, and todo shapes without a
/// parallel store per kind.
pub trait TemplateRepo: Send + Sync {
    /// Creates (`expected = None`) or updates (`expected = Some(rev)`) the template `name` in the
    /// `(kind, scope)`, atomically checking the revision; a mismatch changes nothing.
    fn write(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
        description: Option<&str>,
        body: &str,
        expected: Option<u64>,
    ) -> Result<TemplateWriteResult, StoreError>;

    /// The template `name` in the `(kind, scope)`, or `None`.
    fn read(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
    ) -> Result<Option<StoredTemplate>, StoreError>;

    /// Every template of `kind` in the scope, ordered by name.
    fn list(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
    ) -> Result<Vec<StoredTemplate>, StoreError>;

    /// How many templates of `kind` the scope holds. Distinct from [`TemplateRepo::list`] because
    /// this answer gates every create, and loading every body to reach it is the growth the count
    /// cap exists to refuse.
    fn count(&self, kind: TemplateKind, project: Option<ProjectId>) -> Result<usize, StoreError>;

    /// Removes the template `name` from the `(kind, scope)`, returning whether one was present.
    fn delete(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
    ) -> Result<bool, StoreError>;
}

/// A [`TemplateRepo`] that stores nothing — the default until the durable adapter is wired. A
/// create echoes a placeholder row; reads are empty.
#[derive(Clone, Copy, Default)]
pub struct NoopTemplateRepo;

impl TemplateRepo for NoopTemplateRepo {
    fn write(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
        description: Option<&str>,
        body: &str,
        expected: Option<u64>,
    ) -> Result<TemplateWriteResult, StoreError> {
        if expected.is_some() {
            // Nothing is ever stored, so an update can only be stale.
            return Ok(TemplateWriteResult::Conflict { actual: None });
        }
        Ok(TemplateWriteResult::Written(Box::new(StoredTemplate {
            id: TemplateId::from_raw(0),
            kind,
            project,
            name: name.to_owned(),
            description: description.map(str::to_owned),
            body: body.to_owned(),
            revision: 1,
        })))
    }

    fn read(
        &self,
        _kind: TemplateKind,
        _project: Option<ProjectId>,
        _name: &str,
    ) -> Result<Option<StoredTemplate>, StoreError> {
        Ok(None)
    }

    fn list(
        &self,
        _kind: TemplateKind,
        _project: Option<ProjectId>,
    ) -> Result<Vec<StoredTemplate>, StoreError> {
        Ok(Vec::new())
    }

    fn count(&self, _kind: TemplateKind, _project: Option<ProjectId>) -> Result<usize, StoreError> {
        Ok(0)
    }

    fn delete(
        &self,
        _kind: TemplateKind,
        _project: Option<ProjectId>,
        _name: &str,
    ) -> Result<bool, StoreError> {
        Ok(false)
    }
}
