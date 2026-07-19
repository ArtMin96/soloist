//! The local-UI template surface (context C8 → C6): the global template library the Settings
//! manager reads and writes, plus the seeding seam that fills a new document's empty body from the
//! selected default template of its kind.
//!
//! Two responsibilities, one file because both are the local user's authority over templates:
//!
//! - **Management** ([`templates`](Facade::templates) and siblings) is the trusted local surface the
//!   Settings tab drives. Defaults are global-only in v1, and the seeding read resolves a selected
//!   id off the global scope, so the manager addresses the **global** library (`project = None`);
//!   the per-project scope stays reachable through the session-scoped MCP surface. Every write
//!   announces [`DomainEvent::TemplateChanged`] so the manager (and any prompt-template surface)
//!   re-reads.
//! - **Seeding** ([`seed_body`](Facade::seed_body)) is the one core path both the local UI and MCP
//!   create paths route through, so no adapter grows a domain `if`. The default selection is read per
//!   call from settings (never cached alongside the template list, so a changed default takes effect
//!   at once); the template body is resolved off the aggregate's in-memory cache.

use super::Facade;
use crate::coordination::{TemplateSummary, TemplateView, TemplateWriteError};
use crate::events::DomainEvent;
use crate::facade::CoordinationError;
use crate::template::TemplateKind;

/// The result of seeding a new document's body: the effective body to write, and the name of the
/// template it came from (for a create response), or `None` when nothing seeded.
pub struct Seeded {
    pub body: String,
    pub from: Option<String>,
}

impl Facade {
    /// Every global template of `kind`, ordered by name — the Settings manager's list for one kind.
    /// Global-scoped, so a listing never depends on a project being in scope.
    pub fn templates(&self, kind: TemplateKind) -> Result<Vec<TemplateSummary>, CoordinationError> {
        Ok(self.templates.list(kind, None)?)
    }

    /// The full global template `name` of `kind` — its body, description, and revision — for the
    /// manager to open and edit, or [`CoordinationError::UnknownTemplate`] if there is none.
    pub fn template_read(
        &self,
        kind: TemplateKind,
        name: &str,
    ) -> Result<TemplateView, CoordinationError> {
        self.templates
            .read(kind, None, name)?
            .ok_or(CoordinationError::UnknownTemplate)
    }

    /// Creates the global template `name` of `kind` with `body` (and an optional one-line
    /// description), announcing the change. A taken name is [`CoordinationError::TemplateNameTaken`];
    /// a blank name or body is [`CoordinationError::InvalidTemplate`].
    pub fn template_create(
        &self,
        kind: TemplateKind,
        name: &str,
        description: Option<&str>,
        body: &str,
    ) -> Result<TemplateView, CoordinationError> {
        let view = self
            .templates
            .create(kind, None, name, description, body)
            .map_err(create_error)?;
        self.bus.publish(DomainEvent::TemplateChanged { kind });
        Ok(view)
    }

    /// Replaces the global template `name`'s body and description, guarded by `expected_revision`,
    /// announcing the change. A stale revision is [`CoordinationError::TemplateRevisionConflict`]
    /// and changes nothing.
    pub fn template_update(
        &self,
        kind: TemplateKind,
        name: &str,
        description: Option<&str>,
        body: &str,
        expected_revision: u64,
    ) -> Result<TemplateView, CoordinationError> {
        let view = self
            .templates
            .update(kind, None, name, description, body, expected_revision)
            .map_err(update_error)?;
        self.bus.publish(DomainEvent::TemplateChanged { kind });
        Ok(view)
    }

    /// Removes the global template `name` of `kind`, returning whether one existed. A deleted
    /// template can no longer be a default, so a selection pointing at it is cleared here — the
    /// seeding read already falls back to an empty body for a stale id, but the settings surface
    /// should reflect the removal at once rather than dangle. Announces the change on removal.
    pub fn template_delete(
        &self,
        kind: TemplateKind,
        name: &str,
    ) -> Result<bool, CoordinationError> {
        // Learn the id before deleting, so a default that pointed at this template can be cleared.
        let doomed = self.templates.read(kind, None, name)?.map(|view| view.id);
        let removed = self.templates.delete(kind, None, name)?;
        if removed {
            if let Some(id) = doomed {
                if self.template_defaults()?.get(kind) == Some(id) {
                    self.set_default_template(kind, None)?;
                }
            }
            self.bus.publish(DomainEvent::TemplateChanged { kind });
        }
        Ok(removed)
    }

    /// The body a new document of `kind` should be created with: the caller's `body` when it has
    /// content, otherwise the selected default template's body (global scope), or the empty body
    /// when no default is set or it no longer exists (a blank document is valid). `Seeded::from`
    /// names the seeding template so a create response can report it.
    pub(crate) fn seed_body(
        &self,
        kind: TemplateKind,
        body: String,
    ) -> Result<Seeded, CoordinationError> {
        if !body.trim().is_empty() {
            return Ok(Seeded { body, from: None });
        }
        let Some(default) = self.template_defaults()?.get(kind) else {
            return Ok(Seeded { body, from: None });
        };
        // Defaults are global-only in v1; resolve the selected id off the global cache. A stale id
        // (its template was deleted) resolves to nothing and falls back to the empty body.
        match self.templates.resolve(kind, None, default)? {
            Some(template) => Ok(Seeded {
                body: template.body,
                from: Some(template.name),
            }),
            None => Ok(Seeded { body, from: None }),
        }
    }
}

/// A create's conflict means the name is taken — the actionable message for a caller that did not
/// expect the template to exist. Shared with the session-scoped prompt-template surface so the one
/// `TemplateWriteError` → [`CoordinationError`] mapping lives here.
pub(super) fn create_error(err: TemplateWriteError) -> CoordinationError {
    match err {
        TemplateWriteError::Conflict { .. } => CoordinationError::TemplateNameTaken,
        other => update_error(other),
    }
}

/// Maps a template write's typed failure to the shared [`CoordinationError`] taxonomy — the update
/// path, where an existing name is expected and a revision guards the write.
pub(super) fn update_error(err: TemplateWriteError) -> CoordinationError {
    match err {
        TemplateWriteError::Invalid(message) => CoordinationError::InvalidTemplate(message),
        // No revision on record means no template — "re-read and retry" would mislead a caller
        // whose target was deleted.
        TemplateWriteError::Conflict { actual: None, .. } => CoordinationError::UnknownTemplate,
        TemplateWriteError::Conflict { expected, actual } => {
            CoordinationError::TemplateRevisionConflict { expected, actual }
        }
        TemplateWriteError::Store(err) => CoordinationError::Store(err),
    }
}

#[cfg(test)]
#[path = "template_tests.rs"]
mod tests;
