//! The template aggregate (context C6): durable reusable documents, global or project-scoped,
//! generalized over [`TemplateKind`] so one aggregate serves prompts, scratchpad shapes, and todo
//! shapes rather than a parallel vertical per kind.
//!
//! Templates are shared mutable content like scratchpads, so writes are revision-guarded — a stale
//! update is refused, never a silent clobber. Unlike every other C6 aggregate a template may live
//! in the **global scope** (`project = None`), shared across projects; names are unique per
//! `(kind, scope)`, so the same name may exist as a prompt and a scratchpad shape, or globally and
//! in a project. Placeholders are **derived** from the body on read, never stored, so they can
//! never disagree with the text.
//!
//! The aggregate owns an in-memory cache of each `(kind, scope)`'s rows, populated on first read
//! and invalidated by its own writes (single-writer per aggregate), so seeding a new document from
//! a default template never scans SQLite per creation. A [`crate::events::DomainEvent::TemplateChanged`]
//! is published by the façade after a write for UI freshness; the cache invalidation lives here.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use super::template_repo::{StoredTemplate, TemplateRepo, TemplateWriteResult};
use crate::ids::{ProjectId, TemplateId};
use crate::ports::StoreError;
use crate::sync::{read_lock, write_lock};
use crate::template::{TemplateKind, TemplateScope};

/// The longest accepted template body, in bytes. A template is a starting shape, not a document
/// store; together with the name and description caps this bounds the row, so a runaway caller
/// cannot grow the table without bound.
pub const MAX_TEMPLATE_BODY: usize = 64 * 1024;

/// The longest accepted template name, in characters — an addressing handle, not content.
pub const MAX_TEMPLATE_NAME: usize = 200;

/// The longest accepted template description, in characters — a one-line summary; the body carries
/// the content.
pub const MAX_TEMPLATE_DESCRIPTION: usize = 1_000;

/// A template's full read model: the stored fields plus the placeholders derived from the body.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateView {
    pub id: TemplateId,
    pub kind: TemplateKind,
    pub name: String,
    pub description: Option<String>,
    pub body: String,
    pub placeholders: Vec<String>,
    pub scope: TemplateScope,
    pub revision: u64,
}

impl TemplateView {
    fn of(stored: StoredTemplate) -> Self {
        Self {
            placeholders: placeholders(&stored.body),
            scope: scope_of(stored.project),
            id: stored.id,
            kind: stored.kind,
            name: stored.name,
            description: stored.description,
            body: stored.body,
            revision: stored.revision,
        }
    }
}

/// A template's one-line listing: everything but the body.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateSummary {
    pub id: TemplateId,
    pub kind: TemplateKind,
    pub name: String,
    pub description: Option<String>,
    pub placeholders: Vec<String>,
    pub scope: TemplateScope,
    pub revision: u64,
}

impl TemplateSummary {
    fn of(stored: &StoredTemplate) -> Self {
        Self {
            id: stored.id,
            kind: stored.kind,
            name: stored.name.clone(),
            description: stored.description.clone(),
            placeholders: placeholders(&stored.body),
            scope: scope_of(stored.project),
            revision: stored.revision,
        }
    }
}

/// The portable export envelope: enough to re-create the template anywhere via a create call.
/// Deliberately scope-free — an export is content, not placement — and the `format` tag carries
/// the kind ([`TemplateKind::export_format`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportedTemplate {
    pub format: String,
    pub name: String,
    pub description: Option<String>,
    pub body: String,
}

impl ExportedTemplate {
    fn of(stored: &StoredTemplate) -> Self {
        Self {
            format: stored.kind.export_format().to_owned(),
            name: stored.name.clone(),
            description: stored.description.clone(),
            body: stored.body.clone(),
        }
    }
}

fn scope_of(project: Option<ProjectId>) -> TemplateScope {
    match project {
        None => TemplateScope::Global,
        Some(_) => TemplateScope::Project,
    }
}

/// Why a template write was refused.
#[derive(Debug, thiserror::Error)]
pub enum TemplateWriteError {
    /// The name or body failed validation; the message says why.
    #[error("template is not well-formed: {0}")]
    Invalid(String),
    /// The write expected a different revision than the one on record. `expected` is `None`
    /// for a create; `actual` is `None` when no template exists under that name.
    #[error("template revision conflict (expected {expected:?}, found {actual:?})")]
    Conflict {
        expected: Option<u64>,
        actual: Option<u64>,
    },
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Opens a placeholder marker.
const PLACEHOLDER_OPEN: &str = "{{";
/// Closes a placeholder marker.
const PLACEHOLDER_CLOSE: &str = "}}";

/// The placeholder names a body declares with `{{name}}` markers, deduplicated, in
/// first-occurrence order — what a caller must fill in before applying a prompt (kind-agnostic:
/// derived from any template's body).
///
/// The scan is left-to-right and the first `}}` closes a candidate. Inner text is trimmed
/// (`{{ name }}` names `name`); a candidate that trims to empty or still contains a brace or
/// newline is not a placeholder — its span is consumed as plain text, not rescanned. A stray `{{`
/// with no closing `}}` is plain text.
pub fn placeholders(body: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find(PLACEHOLDER_OPEN) {
        let candidate = &rest[start + PLACEHOLDER_OPEN.len()..];
        let Some(end) = candidate.find(PLACEHOLDER_CLOSE) else {
            break;
        };
        let name = candidate[..end].trim();
        if !name.is_empty()
            && !name.contains(['{', '}', '\n'])
            && !names.iter().any(|seen| seen == name)
        {
            names.push(name.to_owned());
        }
        rest = &candidate[end + PLACEHOLDER_CLOSE.len()..];
    }
    names
}

/// The cache key: rows are grouped by the `(kind, scope)` a list reads, so a write to one group
/// invalidates only its own entry.
type CacheKey = (TemplateKind, Option<ProjectId>);

/// The template aggregate. Validates content, guards revisions, and caches each `(kind, scope)`'s
/// rows in memory; persistence is the [`TemplateRepo`] port's.
pub struct Templates {
    repo: Arc<dyn TemplateRepo>,
    /// Rows per `(kind, scope)`, populated on first read and dropped on this aggregate's own
    /// writes — the single-writer rule keeps it coherent without a shared lock across aggregates.
    cache: RwLock<HashMap<CacheKey, Vec<StoredTemplate>>>,
}

impl Templates {
    pub fn new(repo: Arc<dyn TemplateRepo>) -> Self {
        Self {
            repo,
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Creates the template `name` in the `(kind, scope)`. An existing name is a conflict, reported
    /// with the revision on record.
    pub fn create(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
        description: Option<&str>,
        body: &str,
    ) -> Result<TemplateView, TemplateWriteError> {
        self.write(kind, project, name, description, body, None)
    }

    /// Replaces the template `name`'s body, guarded by the revision the caller read. An omitted
    /// description keeps the stored one; a blank description clears it. A concurrent edit landing
    /// between the keep-read and the write is caught by the revision guard.
    pub fn update(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
        description: Option<&str>,
        body: &str,
        expected_revision: u64,
    ) -> Result<TemplateView, TemplateWriteError> {
        let kept;
        let description = match description {
            Some(text) => Some(text),
            None => {
                kept = self
                    .repo
                    .read(kind, project, name.trim())?
                    .and_then(|stored| stored.description);
                kept.as_deref()
            }
        };
        self.write(
            kind,
            project,
            name,
            description,
            body,
            Some(expected_revision),
        )
    }

    fn write(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
        description: Option<&str>,
        body: &str,
        expected: Option<u64>,
    ) -> Result<TemplateView, TemplateWriteError> {
        let description = description.map(str::trim).filter(|text| !text.is_empty());
        validate(name, description, body).map_err(TemplateWriteError::Invalid)?;
        match self
            .repo
            .write(kind, project, name.trim(), description, body, expected)?
        {
            TemplateWriteResult::Written(stored) => {
                self.invalidate(kind, project);
                Ok(TemplateView::of(*stored))
            }
            TemplateWriteResult::Conflict { actual } => {
                Err(TemplateWriteError::Conflict { expected, actual })
            }
        }
    }

    /// The template `name` in the `(kind, scope)`, or `None`.
    pub fn read(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
    ) -> Result<Option<TemplateView>, StoreError> {
        Ok(self
            .repo
            .read(kind, project, name.trim())?
            .map(TemplateView::of))
    }

    /// Every template of `kind` in the scope as summaries, ordered by name. Reads through the
    /// aggregate's cache — the first read for a `(kind, scope)` populates it, later reads hit it.
    pub fn list(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
    ) -> Result<Vec<TemplateSummary>, StoreError> {
        Ok(self
            .cached_rows(kind, project)?
            .iter()
            .map(TemplateSummary::of)
            .collect())
    }

    /// The template of `kind` in the scope whose durable id is `id`, or `None`. The seeding read:
    /// resolves a selected-default id to its content off the cache, never a per-creation scan.
    pub fn resolve(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        id: TemplateId,
    ) -> Result<Option<TemplateView>, StoreError> {
        Ok(self
            .cached_rows(kind, project)?
            .into_iter()
            .find(|row| row.id == id)
            .map(TemplateView::of))
    }

    /// Removes the template `name` from the `(kind, scope)`, returning whether one was present.
    pub fn delete(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
    ) -> Result<bool, StoreError> {
        let removed = self.repo.delete(kind, project, name.trim())?;
        if removed {
            self.invalidate(kind, project);
        }
        Ok(removed)
    }

    /// The template `name` as a portable export envelope, or `None`.
    pub fn export(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
    ) -> Result<Option<ExportedTemplate>, StoreError> {
        Ok(self
            .repo
            .read(kind, project, name.trim())?
            .map(|stored| ExportedTemplate::of(&stored)))
    }

    /// The stored rows for a `(kind, scope)`, from the cache when warm, else read once and cached.
    fn cached_rows(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
    ) -> Result<Vec<StoredTemplate>, StoreError> {
        let key = (kind, project);
        if let Some(rows) = read_lock(&self.cache).get(&key) {
            return Ok(rows.clone());
        }
        let rows = self.repo.list(kind, project)?;
        write_lock(&self.cache).insert(key, rows.clone());
        Ok(rows)
    }

    /// Drops the cached rows for a `(kind, scope)` after a write to it, so the next read repopulates
    /// from the store.
    fn invalidate(&self, kind: TemplateKind, project: Option<ProjectId>) {
        write_lock(&self.cache).remove(&(kind, project));
    }
}

/// Every problem with a template's content, named at once so the caller fixes it in one revision.
fn validate(name: &str, description: Option<&str>, body: &str) -> Result<(), String> {
    let mut problems = Vec::new();
    if name.trim().is_empty() {
        problems.push("the name is empty".to_owned());
    } else if name.trim().chars().count() > MAX_TEMPLATE_NAME {
        problems.push(format!("the name exceeds {MAX_TEMPLATE_NAME} characters"));
    }
    if let Some(description) = description {
        if description.chars().count() > MAX_TEMPLATE_DESCRIPTION {
            problems.push(format!(
                "the description exceeds {MAX_TEMPLATE_DESCRIPTION} characters"
            ));
        }
    }
    if body.trim().is_empty() {
        problems.push("the body is empty".to_owned());
    }
    if body.len() > MAX_TEMPLATE_BODY {
        problems.push(format!(
            "the body exceeds the {} KiB cap",
            MAX_TEMPLATE_BODY / 1024
        ));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

#[cfg(test)]
#[path = "template_tests.rs"]
mod tests;
