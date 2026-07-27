//! The session-scoped seed-template peek (context C8 → C6): the read a remote caller (MCP today)
//! makes to learn the shape a new scratchpad or todo would be seeded with.
//!
//! Read-only by construction. A session-scoped caller may look at the template its own create would
//! start from, so it can follow that shape when it writes content of its own — but authoring
//! templates and choosing which one is the default stay the local user's authority on
//! [`Facade`](crate::Facade) and are not reachable from here.
//!
//! The peek resolves through the same [`Facade::seed_template`](crate::Facade::seed_template) the
//! create path uses, so a caller is never shown a shape a create would not actually apply — and it
//! answers with [`SeedTemplate`], the two fields seeding consumes, so it cannot show more of the
//! template than a create already does.

use super::scoped::ScopedFacade;
use crate::coordination::SeedTemplate;
use crate::facade::CoordinationError;
use crate::template::TemplateKind;

impl ScopedFacade<'_> {
    /// The template a new empty document of `kind` would be seeded from, or `None` when the local
    /// user has selected no default for it in this session's project.
    ///
    /// The selection is the project's, so resolving it needs the session's scope — the same scope
    /// the create this describes runs in, which keeps the peek available exactly where the create
    /// is and no wider.
    pub fn seed_template(
        &self,
        kind: TemplateKind,
    ) -> Result<Option<SeedTemplate>, CoordinationError> {
        let project = self.coordination_scope()?;
        self.inner.seed_template(kind, project)
    }
}

#[cfg(test)]
#[path = "seed_template_tests.rs"]
mod tests;
