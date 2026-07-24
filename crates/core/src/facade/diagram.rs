//! Session-scoped diagram actions (context C8 → C6): the durable Mermaid-source document surface a
//! remote caller (MCP today) drives within its effective project.
//!
//! Diagrams are project-scoped durable content — not process-owned — so each method resolves only the
//! session's **effective project** (reusing [`coordination_scope`](Facade::coordination_scope),
//! shared with the lease, timer, and scratchpad surface) and routes to the one
//! [`Diagrams`](crate::coordination::Diagrams) aggregate. Scope is resolved here, in the core, so
//! every remote surface inherits the identical rules; an external single-project caller can use
//! diagrams without binding a process, since there is no owner to attribute. The aggregate owns the
//! source validation and the revision guard; this surface maps its typed outcomes to the shared
//! [`CoordinationError`]. Unlike scratchpads, a diagram has no template seed and no cross-project
//! transfer — its body is a raw Mermaid source string the core never renders.

use super::scoped::ScopedFacade;
use super::Facade;
use crate::coordination::{DiagramRenameError, DiagramSummary, DiagramView, DiagramWriteError};
use crate::events::DomainEvent;
use crate::facade::CoordinationError;
use crate::ids::ProjectId;

impl Facade {
    /// Creates or replaces the diagram `name` in `project` with the Mermaid `source`,
    /// **revision-guarded** (`expected` is `None` to create or the current revision to update), the
    /// local-UI path — it trusts the caller to be entitled to `project`, so it must never take a
    /// `project` from an untrusted surface (see [`orchestration_snapshot`](Self::orchestration_snapshot)).
    /// Emits `DiagramChanged` on success. A malformed write is
    /// [`CoordinationError::InvalidDiagram`] and a stale revision
    /// [`CoordinationError::DiagramRevisionConflict`], neither of which changes anything.
    pub fn diagram_write_in(
        &self,
        project: ProjectId,
        name: &str,
        source: String,
        expected: Option<u64>,
    ) -> Result<DiagramView, CoordinationError> {
        self.emit_diagram(
            project,
            self.diagrams
                .write(project, name, source, expected)
                .map_err(|err| match err {
                    DiagramWriteError::Invalid(message) => {
                        CoordinationError::InvalidDiagram(message)
                    }
                    DiagramWriteError::Conflict { expected, actual } => {
                        CoordinationError::DiagramRevisionConflict { expected, actual }
                    }
                    DiagramWriteError::Store(err) => CoordinationError::Store(err),
                }),
        )
    }

    /// The diagram `name` in `project`, or [`CoordinationError::UnknownDiagram`] if there is none —
    /// the local-UI path (trusts the caller to be entitled to `project`; see
    /// [`diagram_write_in`](Self::diagram_write_in)).
    pub fn diagram_read_in(
        &self,
        project: ProjectId,
        name: &str,
    ) -> Result<DiagramView, CoordinationError> {
        self.diagrams
            .read(project, name)?
            .ok_or(CoordinationError::UnknownDiagram)
    }

    /// Archives or restores the diagram `name` in `project`, emitting `DiagramChanged`, or
    /// [`CoordinationError::UnknownDiagram`] if there is none — the local-UI path (see
    /// [`diagram_write_in`](Self::diagram_write_in)). Archiving keeps the source — it is a listing
    /// flag, not a delete.
    pub fn diagram_archive_in(
        &self,
        project: ProjectId,
        name: &str,
        archived: bool,
    ) -> Result<DiagramView, CoordinationError> {
        self.emit_diagram(
            project,
            self.diagrams
                .set_archived(project, name, archived)?
                .ok_or(CoordinationError::UnknownDiagram),
        )
    }

    /// Renames the diagram `from` to `to` in `project`, keeping its durable id, source, tags,
    /// archived flag, and revision, and emitting `DiagramChanged` under the new name — the local-UI
    /// path (see [`diagram_write_in`](Self::diagram_write_in)).
    /// [`CoordinationError::UnknownDiagram`] if `from` has no such diagram and
    /// [`CoordinationError::DiagramNameTaken`] if `to` is already in use, neither of which changes
    /// anything.
    pub fn diagram_rename_in(
        &self,
        project: ProjectId,
        from: &str,
        to: &str,
    ) -> Result<DiagramView, CoordinationError> {
        self.emit_diagram(
            project,
            self.diagrams
                .rename(project, from, to)
                .map_err(map_rename_error),
        )
    }

    /// Publishes a [`DomainEvent::DiagramChanged`] for the diagram a successful mutation returned
    /// (keyed by its `name` handle), then passes the result through — the single emission seam every
    /// diagram write routes through. A failed write emits nothing.
    fn emit_diagram(
        &self,
        project: ProjectId,
        result: Result<DiagramView, CoordinationError>,
    ) -> Result<DiagramView, CoordinationError> {
        if let Ok(view) = &result {
            self.bus.publish(DomainEvent::DiagramChanged {
                project,
                name: view.name.clone(),
            });
        }
        result
    }
}

/// Maps a diagram [`RenameError`](DiagramRenameError) to the shared [`CoordinationError`]. Shared by
/// the local and scoped rename surfaces so both report a missing/taken diagram identically.
fn map_rename_error(err: DiagramRenameError) -> CoordinationError {
    match err {
        DiagramRenameError::NotFound => CoordinationError::UnknownDiagram,
        DiagramRenameError::NameTaken => CoordinationError::DiagramNameTaken,
        DiagramRenameError::Store(err) => CoordinationError::Store(err),
    }
}

impl ScopedFacade<'_> {
    /// Creates or replaces the diagram `name` in the session's effective project with the Mermaid
    /// `source`, **revision-guarded**: `expected` is `None` to create or the current revision to
    /// update. A malformed write is [`CoordinationError::InvalidDiagram`] and a stale revision
    /// [`CoordinationError::DiagramRevisionConflict`], neither of which changes anything.
    pub fn diagram_write(
        &self,
        name: &str,
        source: String,
        expected: Option<u64>,
    ) -> Result<DiagramView, CoordinationError> {
        let project = self.coordination_scope()?;
        self.inner.diagram_write_in(project, name, source, expected)
    }

    /// The diagram `name` in the session's effective project, or
    /// [`CoordinationError::UnknownDiagram`] if there is none.
    pub fn diagram_read(&self, name: &str) -> Result<DiagramView, CoordinationError> {
        let project = self.coordination_scope()?;
        self.inner.diagram_read_in(project, name)
    }

    /// Every diagram in the session's effective project as a one-line summary.
    pub fn diagram_list(&self) -> Result<Vec<DiagramSummary>, CoordinationError> {
        let project = self.coordination_scope()?;
        Ok(self.inner.diagrams.list(project)?)
    }

    /// Renames the diagram `from` to `to` in the session's effective project (its durable id is
    /// unchanged), returning the renamed diagram.
    pub fn diagram_rename(&self, from: &str, to: &str) -> Result<DiagramView, CoordinationError> {
        let project = self.coordination_scope()?;
        self.inner.diagram_rename_in(project, from, to)
    }

    /// Adds `tags` to the diagram `name` in the session's effective project, returning the updated
    /// diagram, or [`CoordinationError::UnknownDiagram`] if there is none.
    pub fn diagram_add_tags(
        &self,
        name: &str,
        tags: &[String],
    ) -> Result<DiagramView, CoordinationError> {
        let project = self.coordination_scope()?;
        self.inner.emit_diagram(
            project,
            self.inner
                .diagrams
                .add_tags(project, name, tags)?
                .ok_or(CoordinationError::UnknownDiagram),
        )
    }

    /// Removes `tags` from the diagram `name` in the session's effective project, returning the
    /// updated diagram, or [`CoordinationError::UnknownDiagram`] if there is none.
    pub fn diagram_remove_tags(
        &self,
        name: &str,
        tags: &[String],
    ) -> Result<DiagramView, CoordinationError> {
        let project = self.coordination_scope()?;
        self.inner.emit_diagram(
            project,
            self.inner
                .diagrams
                .remove_tags(project, name, tags)?
                .ok_or(CoordinationError::UnknownDiagram),
        )
    }

    /// The distinct tags used across the session's effective project's diagrams, sorted.
    pub fn diagram_tags_list(&self) -> Result<Vec<String>, CoordinationError> {
        let project = self.coordination_scope()?;
        Ok(self.inner.diagrams.tags(project)?)
    }

    /// Archives or restores the diagram `name` in the session's effective project, returning the
    /// updated diagram, or [`CoordinationError::UnknownDiagram`] if there is none. Archiving keeps
    /// the source — it is a listing flag, not a delete.
    pub fn diagram_archive(
        &self,
        name: &str,
        archived: bool,
    ) -> Result<DiagramView, CoordinationError> {
        let project = self.coordination_scope()?;
        self.inner.diagram_archive_in(project, name, archived)
    }

    /// Deletes the diagram `name` in the session's effective project, returning whether one was
    /// removed.
    pub fn diagram_delete(&self, name: &str) -> Result<bool, CoordinationError> {
        let project = self.coordination_scope()?;
        let removed = self.inner.diagrams.delete(project, name)?;
        if removed {
            self.inner.bus.publish(DomainEvent::DiagramChanged {
                project,
                name: name.to_owned(),
            });
        }
        Ok(removed)
    }
}

#[cfg(test)]
#[path = "diagram_tests.rs"]
mod tests;
