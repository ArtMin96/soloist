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
//! a default template never scans SQLite per creation. Because that cache holds whole bodies, what
//! bounds it is [`MAX_TEMPLATES_PER_SCOPE`] against the per-row caps. A
//! [`crate::events::DomainEvent::TemplateChanged`] is published by the façade after a write for UI
//! freshness; the cache invalidation lives here.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use super::template_repo::{StoredTemplate, TemplateRepo, TemplateWriteResult};
use super::template_scan::{scan, Token};
use crate::ids::{ProjectId, TemplateId};
use crate::ports::StoreError;
use crate::sync::{read_lock, write_lock};
use crate::template::{TemplateKind, TemplateScope};

/// The longest accepted template body, in bytes. A template is a starting shape, not a document
/// store; together with the name and description caps this bounds one row.
pub const MAX_TEMPLATE_BODY: usize = 64 * 1024;

/// The longest accepted template name, in characters — an addressing handle, not content.
pub const MAX_TEMPLATE_NAME: usize = 200;

/// The longest accepted template description, in characters — a one-line summary; the body carries
/// the content.
pub const MAX_TEMPLATE_DESCRIPTION: usize = 1_000;

/// The most templates one `(kind, scope)` may hold. The per-row caps bound a row but not how many
/// rows a caller may add, and this aggregate mirrors a whole group in memory including every body,
/// so without this ceiling a well-behaved-looking caller grows both the table and RSS one valid row
/// at a time. A curated library is tens of entries; this leaves an order of magnitude of headroom
/// while bounding a group at a size the process can hold.
pub const MAX_TEMPLATES_PER_SCOPE: usize = 500;

/// The most distinct `{{name}}` placeholders one body may declare. Placeholder names are derived
/// from the body on every read rather than stored, so each name costs a fresh allocation per row per
/// listing; a body that is nothing but markers would otherwise make listing a scope cost far more
/// than its bodies do. A template a person fills in by hand has a handful of slots.
pub const MAX_PLACEHOLDERS_PER_BODY: usize = 100;

/// The longest rendered prompt, in bytes. [`MAX_TEMPLATE_BODY`] bounds the stored *template*, but
/// substituting its placeholders with caller-supplied values is unbounded on its own — N markers
/// times a large value each — so the rendered result carries its own ceiling.
pub const MAX_RENDERED_PROMPT: usize = 256 * 1024;

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

/// The placeholder names a body declares with `{{name}}` markers, deduplicated, in
/// first-occurrence order — what a caller must fill in before applying a prompt (kind-agnostic:
/// derived from any template's body).
///
/// This is one reading of `template_scan::scan`; substitution is another, so a name reported here
/// is always one substitution fills, and an escaped `\{{name}}` is reported by neither. The
/// grammar the scan applies — trimming, malformed candidates, escapes — is documented there.
pub fn placeholders(body: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for token in scan(body) {
        let Token::Placeholder { name, .. } = token else {
            continue;
        };
        if !names.iter().any(|seen| seen == name) {
            names.push(name.to_owned());
        }
    }
    names
}

/// Whether `body` declares more than `limit` distinct placeholder names. Stops counting at the
/// first name past the limit, so refusing a pathological body never costs what naming every
/// placeholder in it would.
fn declares_more_placeholders_than(body: &str, limit: usize) -> bool {
    let mut seen: Vec<&str> = Vec::new();
    for token in scan(body) {
        let Token::Placeholder { name, .. } = token else {
            continue;
        };
        if seen.contains(&name) {
            continue;
        }
        seen.push(name);
        if seen.len() > limit {
            return true;
        }
    }
    false
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
        // Only a create adds a row to the scope; an update replaces one, so it answers to the
        // content caps alone and an edit is never wedged by a full scope.
        let occupied = match expected {
            None => Some(self.repo.count(kind, project)?),
            Some(_) => None,
        };
        validate(name, description, body, occupied).map_err(TemplateWriteError::Invalid)?;
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
        self.with_cached_rows(kind, project, |rows| {
            rows.iter().map(TemplateSummary::of).collect()
        })
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
            .with_cached_rows(kind, project, |rows| {
                rows.iter().find(|row| row.id == id).cloned()
            })?
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

    /// Reads the stored rows for a `(kind, scope)` through `read` — served from the cache when warm,
    /// else scanned once and cached. Handing the reader the rows rather than returning them keeps a
    /// caller that wants only a count, or one row, from cloning every cached body to get it.
    fn with_cached_rows<T>(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        read: impl FnOnce(&[StoredTemplate]) -> T,
    ) -> Result<T, StoreError> {
        let key = (kind, project);
        if let Some(rows) = read_lock(&self.cache).get(&key) {
            return Ok(read(rows));
        }
        let rows = self.repo.list(kind, project)?;
        let value = read(&rows);
        write_lock(&self.cache).insert(key, rows);
        Ok(value)
    }

    /// Drops the cached rows for a `(kind, scope)` after a write to it, so the next read repopulates
    /// from the store.
    fn invalidate(&self, kind: TemplateKind, project: Option<ProjectId>) {
        write_lock(&self.cache).remove(&(kind, project));
    }

    /// Drops every cached entry belonging to `project`, of any kind — for when its rows are deleted
    /// underneath this aggregate rather than through it. Removing a project cascades its templates
    /// away in the store, which no write here observes, so without this the entries would outlive
    /// the project they describe.
    pub(super) fn forget_project(&self, project: ProjectId) {
        write_lock(&self.cache).retain(|(_, scope), _| *scope != Some(project));
    }

    /// Drops every cached entry, so the next read of any `(kind, scope)` repopulates from the
    /// store. The recovery path for when this aggregate cannot know what changed underneath it.
    pub(super) fn forget_all(&self) {
        write_lock(&self.cache).clear();
    }
}

/// Every problem with a template's content, named at once so the caller fixes it in one revision.
///
/// `occupied` is how many templates the target scope already holds, for a write that would add a
/// row to it, and `None` for one that replaces a row and so cannot grow the scope.
fn validate(
    name: &str,
    description: Option<&str>,
    body: &str,
    occupied: Option<usize>,
) -> Result<(), String> {
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
    if declares_more_placeholders_than(body, MAX_PLACEHOLDERS_PER_BODY) {
        problems.push(format!(
            "the body declares more than {MAX_PLACEHOLDERS_PER_BODY} distinct placeholders"
        ));
    }
    if occupied.is_some_and(|held| held >= MAX_TEMPLATES_PER_SCOPE) {
        problems.push(format!(
            "the scope already holds the maximum of {MAX_TEMPLATES_PER_SCOPE} templates"
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
