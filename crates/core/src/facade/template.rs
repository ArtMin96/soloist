//! The local-UI template surface (context C8 → C6): the template libraries the Settings manager
//! reads and writes, plus the seeding seam that fills a new document's empty body from the selected
//! default template of its kind.
//!
//! Two responsibilities, one file because both are the local user's authority over templates:
//!
//! - **Management** ([`templates`](Facade::templates) and siblings) is the trusted local surface the
//!   Settings tab drives. The local user is not scope-limited, so every method takes the scope
//!   outright — `None` for the global library, `Some(project)` for that project's — and the manager
//!   addresses the same two libraries a session-scoped caller reaches. Every write announces
//!   [`DomainEvent::TemplateChanged`] carrying the scope it changed, so a surface re-reads the list
//!   that actually moved.
//! - **Seeding** ([`seed_body`](Facade::seed_body)) is the one core path both the local UI and MCP
//!   create paths route through, so no adapter grows a domain `if`. The default selection is read per
//!   call from settings (never cached alongside the template list, so a changed default takes effect
//!   at once); the template body is resolved off the aggregate's in-memory cache.

use super::Facade;
use crate::coordination::{
    RenderError, RenderRequest, RenderedPrompt, TemplateSummary, TemplateView, TemplateWriteError,
};
use crate::events::DomainEvent;
use crate::facade::CoordinationError;
use crate::ids::ProjectId;
use crate::template::TemplateKind;

/// The result of seeding a new document's body: the effective body to write, and the name of the
/// template it came from (for a create response), or `None` when nothing seeded.
pub struct Seeded {
    pub body: String,
    pub from: Option<String>,
}

impl Facade {
    /// Every template of `kind` in `project`'s scope, ordered by name — the Settings manager's list
    /// for one kind and scope. `None` lists the global library.
    pub fn templates(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
    ) -> Result<Vec<TemplateSummary>, CoordinationError> {
        Ok(self.templates.list(kind, project)?)
    }

    /// The full template `name` of `kind` in `project`'s scope — its body, description, and
    /// revision — for the manager to open and edit, or [`CoordinationError::UnknownTemplate`] if
    /// there is none.
    pub fn template_read(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
    ) -> Result<TemplateView, CoordinationError> {
        self.templates
            .read(kind, project, name)?
            .ok_or(CoordinationError::UnknownTemplate)
    }

    /// Creates the template `name` of `kind` in `project`'s scope with `body` (and an optional
    /// one-line description), announcing the change. A taken name is
    /// [`CoordinationError::TemplateNameTaken`]; a blank name or body is
    /// [`CoordinationError::InvalidTemplate`].
    pub fn template_create(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
        description: Option<&str>,
        body: &str,
    ) -> Result<TemplateView, CoordinationError> {
        let view = self
            .templates
            .create(kind, project, name, description, body)
            .map_err(create_error)?;
        self.bus
            .publish(DomainEvent::TemplateChanged { kind, project });
        Ok(view)
    }

    /// Replaces the template `name`'s body and description in `project`'s scope, guarded by
    /// `expected_revision`, announcing the change. A stale revision is
    /// [`CoordinationError::TemplateRevisionConflict`] and changes nothing.
    pub fn template_update(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
        description: Option<&str>,
        body: &str,
        expected_revision: u64,
    ) -> Result<TemplateView, CoordinationError> {
        let view = self
            .templates
            .update(kind, project, name, description, body, expected_revision)
            .map_err(update_error)?;
        self.bus
            .publish(DomainEvent::TemplateChanged { kind, project });
        Ok(view)
    }

    /// Removes the template `name` of `kind` from `project`'s scope, returning whether one existed.
    /// A deleted template can no longer be a default, so a selection pointing at it is cleared here
    /// — the seeding read already falls back to an empty body for a stale id, but the settings
    /// surface should reflect the removal at once rather than dangle. Announces the change on
    /// removal.
    pub fn template_delete(
        &self,
        kind: TemplateKind,
        project: Option<ProjectId>,
        name: &str,
    ) -> Result<bool, CoordinationError> {
        // Learn the id before deleting, so a default that pointed at this template can be cleared.
        let doomed = self
            .templates
            .read(kind, project, name)?
            .map(|view| view.id);
        let removed = self.templates.delete(kind, project, name)?;
        if removed {
            if let Some(id) = doomed {
                if self.template_defaults()?.get(kind) == Some(id) {
                    self.set_default_template(kind, None)?;
                }
            }
            self.bus
                .publish(DomainEvent::TemplateChanged { kind, project });
        }
        Ok(removed)
    }

    /// The prompt template `request.name` rendered with `request.values` — what the local user's
    /// preview shows, and the same core behaviour a session-scoped caller reaches through
    /// [`ScopedFacade::prompt_template_render`](crate::ScopedFacade::prompt_template_render).
    ///
    /// The scope is stated outright rather than resolved from a session, because the local user is
    /// not scope-limited: `None` renders from the global library, `Some(project)` from that
    /// project's. Rendering is a query — nothing is written and no event is published.
    pub fn template_render(
        &self,
        project: Option<ProjectId>,
        request: &RenderRequest,
    ) -> Result<RenderedPrompt, RenderError> {
        self.templates.render(project, request)
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
