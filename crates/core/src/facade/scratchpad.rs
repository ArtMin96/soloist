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
        self.scratchpad_write_in(project, name, doc, expected)
    }

    /// The scratchpad `name` in the session's effective project, or
    /// [`CoordinationError::UnknownScratchpad`] if there is none.
    pub fn scratchpad_read(
        &self,
        session: SessionId,
        name: &str,
    ) -> Result<ScratchpadView, CoordinationError> {
        let project = self.coordination_scope(session)?;
        self.scratchpad_read_in(project, name)
    }

    /// [`scratchpad_write`](Self::scratchpad_write) scoped to `project` directly — the local-UI path
    /// (the panels), mirroring [`orchestration_snapshot`](Self::orchestration_snapshot): it trusts the
    /// caller to be entitled to `project`, so it must never take a `project` from an untrusted surface.
    pub fn scratchpad_write_in(
        &self,
        project: ProjectId,
        name: &str,
        doc: ScratchpadDoc,
        expected: Option<u64>,
    ) -> Result<ScratchpadView, CoordinationError> {
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

    /// [`scratchpad_read`](Self::scratchpad_read) scoped to `project` directly — the local-UI path
    /// (trusts the caller to be entitled to `project`; see [`scratchpad_write_in`](Self::scratchpad_write_in)).
    pub fn scratchpad_read_in(
        &self,
        project: ProjectId,
        name: &str,
    ) -> Result<ScratchpadView, CoordinationError> {
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

    /// Moves the scratchpad `name` into project `to` for a scoped session (context C8 → C6, O10).
    /// Authorized only when the caller is authenticated to **both** its own effective project (the
    /// source) and `to` (the target, via [`authentic_scope`](Facade::authentic_scope)); else
    /// [`CoordinationError::ForeignProject`]. Because an MCP session authenticates to a single
    /// project, a genuine cross-project transfer is refused here — the reachable path is the local
    /// [`scratchpad_transfer_in`](Self::scratchpad_transfer_in). Keeps the document/revision/tags/id.
    pub fn scratchpad_transfer(
        &self,
        session: SessionId,
        name: &str,
        to: ProjectId,
    ) -> Result<ScratchpadView, CoordinationError> {
        let from = self.coordination_scope(session)?;
        if !self.authentic_scope(session, to) {
            return Err(CoordinationError::ForeignProject);
        }
        self.scratchpad_transfer_in(from, name, to)
    }

    /// [`scratchpad_transfer`](Self::scratchpad_transfer) scoped to `from`/`to` directly (local-UI
    /// path — never takes a project from an untrusted surface). Moves the scratchpad `name` from
    /// `from` to `to`, keeping its document, revision, tags, archived flag, and id. Emits
    /// `ScratchpadChanged` for **both** boards — the source drops it, the target shows it — or
    /// [`CoordinationError::UnknownScratchpad`] / [`CoordinationError::ScratchpadNameTaken`].
    pub fn scratchpad_transfer_in(
        &self,
        from: ProjectId,
        name: &str,
        to: ProjectId,
    ) -> Result<ScratchpadView, CoordinationError> {
        let result = self
            .scratchpads
            .transfer(from, name, to)
            .map_err(|err| match err {
                RenameError::NotFound => CoordinationError::UnknownScratchpad,
                RenameError::NameTaken => CoordinationError::ScratchpadNameTaken,
                RenameError::Store(err) => CoordinationError::Store(err),
            });
        if let Ok(view) = &result {
            self.bus.publish(DomainEvent::ScratchpadChanged {
                project: from,
                name: view.name.clone(),
            });
            self.bus.publish(DomainEvent::ScratchpadChanged {
                project: to,
                name: view.name.clone(),
            });
        }
        result
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
