//! Resolving a `solo://` link to its content (context C8 → C6): the human-handoff path where an
//! agent is given a copied scratchpad/todo link and reads the target.
//!
//! Scope is enforced **here, in the core**, not in any adapter: a link is resolved only within the
//! caller's effective project, and a link naming another project is refused rather than resolved —
//! so a pasted link can never leak another project's content. The link's durable id is mapped to the
//! current content through the existing aggregate reads, so a renamed scratchpad still resolves.

use super::Facade;
use crate::coordination::{Link, LinkContent, LinkTarget};
use crate::facade::CoordinationError;
use crate::ids::SessionId;

impl Facade {
    /// Resolves a `solo://proj/<project>/scratchpad|todo/<id>` link to its content within the
    /// session's effective project. A malformed link is [`CoordinationError::MalformedLink`]; a link
    /// to another project is [`CoordinationError::ForeignScopeLink`] (refused, never resolved); an id
    /// with no live content is [`CoordinationError::UnknownScratchpad`]/[`UnknownTodo`](CoordinationError::UnknownTodo).
    pub fn resolve_link(
        &self,
        session: SessionId,
        link: &str,
    ) -> Result<LinkContent, CoordinationError> {
        let parsed = Link::parse(link).map_err(|_| CoordinationError::MalformedLink)?;
        let project = self.coordination_scope(session)?;
        if parsed.project != project {
            return Err(CoordinationError::ForeignScopeLink);
        }
        match parsed.target {
            LinkTarget::Scratchpad(id) => {
                // Scratchpads are addressed by name; map the durable id to the current name, then read.
                let name = self
                    .scratchpads
                    .list(project)?
                    .into_iter()
                    .find(|summary| summary.id == id)
                    .map(|summary| summary.name)
                    .ok_or(CoordinationError::UnknownScratchpad)?;
                let view = self
                    .scratchpads
                    .read(project, &name)?
                    .ok_or(CoordinationError::UnknownScratchpad)?;
                Ok(LinkContent::Scratchpad(view))
            }
            LinkTarget::Todo(id) => {
                let view = self
                    .todos
                    .get(project, id)?
                    .ok_or(CoordinationError::UnknownTodo)?;
                Ok(LinkContent::Todo(view))
            }
        }
    }
}

#[cfg(test)]
#[path = "link_tests.rs"]
mod tests;
