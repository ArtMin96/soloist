//! The template seeding seam (context C8 → C6): the one core path that fills a new document's
//! empty body from the selected default template of its kind.
//!
//! Both the local UI and MCP create paths route through the coordination write methods
//! ([`scratchpad_write_in`](Facade::scratchpad_write_in),
//! [`todo_create_in`](Facade::todo_create_in)), and those call [`seed_body`](Facade::seed_body)
//! here — so seeding is single-sourced in the core and no adapter grows a domain `if`. The default
//! selection is read per call from settings (never cached alongside the template list, so a changed
//! default takes effect at once); the template body is resolved off the aggregate's in-memory cache.

use super::Facade;
use crate::facade::CoordinationError;
use crate::template::TemplateKind;

/// The result of seeding a new document's body: the effective body to write, and the name of the
/// template it came from (for a create response), or `None` when nothing seeded.
pub struct Seeded {
    pub body: String,
    pub from: Option<String>,
}

impl Facade {
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

#[cfg(test)]
#[path = "template_tests.rs"]
mod tests;
