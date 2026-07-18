//! Templates settings (global Templates tab): the default template each free-form kind seeds a new
//! document from.
//!
//! Global-only in v1 (a per-project override lands later, like the editor resolver). `None` means
//! "seed an empty document"; a stale id — its template was since deleted — also seeds empty, since
//! the seeding read resolves the id off the live cache and finds nothing. Prompts are applied by
//! name, never seeded, so they have no default here.

use serde::{Deserialize, Serialize};

use crate::ids::TemplateId;
use crate::template::TemplateKind;

/// The Templates tab document: the selected default template per seedable kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TemplateDefaults {
    /// The template a new scratchpad's body is seeded from when created empty, or `None`.
    pub scratchpad: Option<TemplateId>,
    /// The template a new todo's body is seeded from when created empty, or `None`.
    pub todo: Option<TemplateId>,
}

impl TemplateDefaults {
    /// The default template selected for `kind`, or `None` — including for [`TemplateKind::Prompt`],
    /// which is applied by name rather than seeded.
    pub fn get(&self, kind: TemplateKind) -> Option<TemplateId> {
        match kind {
            TemplateKind::Scratchpad => self.scratchpad,
            TemplateKind::Todo => self.todo,
            TemplateKind::Prompt => None,
        }
    }

    /// Selects `template` as the default for `kind` in place. Prompt has no seed default, so a set
    /// for it is a no-op — the exhaustive match keeps a new kind honest.
    pub fn set(&mut self, kind: TemplateKind, template: Option<TemplateId>) {
        match kind {
            TemplateKind::Scratchpad => self.scratchpad = template,
            TemplateKind::Todo => self.todo = template,
            TemplateKind::Prompt => {}
        }
    }
}
