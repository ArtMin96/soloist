//! Session-scoped scratchpad actions (context C8 → C6): the durable shared-document surface a
//! remote caller (MCP today) drives within its effective project.
//!
//! Scratchpads are project-scoped durable content — not process-owned — so each method resolves only
//! the session's **effective project** (reusing [`coordination_scope`](Facade::coordination_scope),
//! shared with the lease and timer surface) and routes to the one
//! [`Scratchpads`](crate::coordination::Scratchpads) aggregate. Scope is resolved here, in the core,
//! so every remote surface inherits the identical rules; an external single-project caller can use
//! scratchpads without binding a process, since there is no owner to attribute. The aggregate owns
//! the disciplined-document validation and the revision guard; this surface maps its typed outcomes
//! to the shared [`CoordinationError`].

use super::Facade;
use crate::coordination::{
    RenameError, ScratchpadDoc, ScratchpadSummary, ScratchpadView, WriteError,
};
use crate::events::DomainEvent;
use crate::facade::CoordinationError;
use crate::ids::{ProjectId, SessionId};

impl Facade {
    /// Creates or replaces the scratchpad `name` in the session's effective project with the
    /// disciplined `doc`, **revision-guarded**: `expected` is `None` to create or the current
    /// revision to update. Returns the written scratchpad at its new revision; a malformed document
    /// is [`CoordinationError::InvalidScratchpad`] and a stale revision
    /// [`CoordinationError::RevisionConflict`], neither of which changes anything.
    pub fn scratchpad_write(
        &self,
        session: SessionId,
        name: &str,
        doc: ScratchpadDoc,
        expected: Option<u64>,
    ) -> Result<ScratchpadView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.emit_scratchpad(
            project,
            self.scratchpads
                .write(project, name, doc, expected)
                .map_err(|err| match err {
                    WriteError::Invalid(message) => CoordinationError::InvalidScratchpad(message),
                    WriteError::Conflict { expected, actual } => {
                        CoordinationError::RevisionConflict { expected, actual }
                    }
                    WriteError::Store(err) => CoordinationError::Store(err),
                }),
        )
    }

    /// The scratchpad `name` in the session's effective project, or
    /// [`CoordinationError::UnknownScratchpad`] if there is none.
    pub fn scratchpad_read(
        &self,
        session: SessionId,
        name: &str,
    ) -> Result<ScratchpadView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.scratchpads
            .read(project, name)?
            .ok_or(CoordinationError::UnknownScratchpad)
    }

    /// Every scratchpad in the session's effective project as a one-line summary.
    pub fn scratchpad_list(
        &self,
        session: SessionId,
    ) -> Result<Vec<ScratchpadSummary>, CoordinationError> {
        let project = self.coordination_scope(session)?;
        Ok(self.scratchpads.list(project)?)
    }

    /// Renames the scratchpad `from` to `to` in the session's effective project (its durable id is
    /// unchanged), returning the renamed scratchpad.
    pub fn scratchpad_rename(
        &self,
        session: SessionId,
        from: &str,
        to: &str,
    ) -> Result<ScratchpadView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.emit_scratchpad(
            project,
            self.scratchpads
                .rename(project, from, to)
                .map_err(|err| match err {
                    RenameError::NotFound => CoordinationError::UnknownScratchpad,
                    RenameError::NameTaken => CoordinationError::ScratchpadNameTaken,
                    RenameError::Store(err) => CoordinationError::Store(err),
                }),
        )
    }

    /// Adds `tags` to the scratchpad `name` in the session's effective project, returning the
    /// updated scratchpad, or [`CoordinationError::UnknownScratchpad`] if there is none.
    pub fn scratchpad_add_tags(
        &self,
        session: SessionId,
        name: &str,
        tags: &[String],
    ) -> Result<ScratchpadView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.emit_scratchpad(
            project,
            self.scratchpads
                .add_tags(project, name, tags)?
                .ok_or(CoordinationError::UnknownScratchpad),
        )
    }

    /// Removes `tags` from the scratchpad `name` in the session's effective project, returning the
    /// updated scratchpad, or [`CoordinationError::UnknownScratchpad`] if there is none.
    pub fn scratchpad_remove_tags(
        &self,
        session: SessionId,
        name: &str,
        tags: &[String],
    ) -> Result<ScratchpadView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.emit_scratchpad(
            project,
            self.scratchpads
                .remove_tags(project, name, tags)?
                .ok_or(CoordinationError::UnknownScratchpad),
        )
    }

    /// The distinct tags used across the session's effective project's scratchpads, sorted.
    pub fn scratchpad_tags_list(
        &self,
        session: SessionId,
    ) -> Result<Vec<String>, CoordinationError> {
        let project = self.coordination_scope(session)?;
        Ok(self.scratchpads.tags(project)?)
    }

    /// Archives or restores the scratchpad `name` in the session's effective project, returning the
    /// updated scratchpad, or [`CoordinationError::UnknownScratchpad`] if there is none. Archiving
    /// keeps the document — it is a listing flag, not a delete.
    pub fn scratchpad_archive(
        &self,
        session: SessionId,
        name: &str,
        archived: bool,
    ) -> Result<ScratchpadView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.emit_scratchpad(
            project,
            self.scratchpads
                .set_archived(project, name, archived)?
                .ok_or(CoordinationError::UnknownScratchpad),
        )
    }

    /// Deletes the scratchpad `name` in the session's effective project, returning whether one was
    /// removed.
    pub fn scratchpad_delete(
        &self,
        session: SessionId,
        name: &str,
    ) -> Result<bool, CoordinationError> {
        let project = self.coordination_scope(session)?;
        let removed = self.scratchpads.delete(project, name)?;
        if removed {
            self.bus.publish(DomainEvent::ScratchpadChanged {
                project,
                name: name.to_owned(),
            });
        }
        Ok(removed)
    }

    /// Publishes a [`DomainEvent::ScratchpadChanged`] for the scratchpad a successful mutation
    /// returned (keyed by its `name` handle), then passes the result through — the single emission
    /// seam every scratchpad write routes through. A failed write emits nothing.
    fn emit_scratchpad(
        &self,
        project: ProjectId,
        result: Result<ScratchpadView, CoordinationError>,
    ) -> Result<ScratchpadView, CoordinationError> {
        if let Ok(view) = &result {
            self.bus.publish(DomainEvent::ScratchpadChanged {
                project,
                name: view.name.clone(),
            });
        }
        result
    }
}

#[cfg(test)]
#[path = "scratchpad_tests.rs"]
mod tests;
