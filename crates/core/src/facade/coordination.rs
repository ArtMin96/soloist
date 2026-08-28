//! Session-scoped coordination actions (context C8 → C6): the lease surface a remote caller (MCP
//! today) drives within its effective project, plus the shared [`CoordinationError`] taxonomy and
//! project/owner resolution the sibling lease, timer, scratchpad, diagram, todo, and template
//! surfaces route through.
//!
//! A lease is project-scoped and process-owned, so each method resolves two things in the core —
//! the session's **effective project** (what the record belongs to) and its **bound process** (who
//! owns it) — before routing to the one [`Leases`](crate::coordination::Leases) aggregate. Both are
//! resolved here, not in any adapter, so every remote surface inherits the identical scope and
//! ownership rules. The bound process must be authentic (it was checked at bind time), which is
//! also what lets the supervisor auto-release a lease when that process closes.

use std::time::Duration;

use super::scoped::ScopedFacade;
use super::Facade;
use crate::coordination::{AcquireOutcome, LeaseView};
use crate::events::DomainEvent;
use crate::ids::{ProcessId, ProjectId, SessionId, TodoId};
use crate::ports::StoreError;

/// Why a coordination action was refused. Mapped by the wire adapters to their own error type, so
/// the taxonomy is defined once here.
#[derive(Debug, thiserror::Error)]
pub enum CoordinationError {
    /// The session has no project in scope to act within (none selected, bound, or singular).
    #[error("no project is in scope; select one first")]
    NoProjectScope,
    /// The session is not bound to a process, so it has no owner to attribute the record to. An
    /// agent binds via its injected `SOLOIST_PROCESS_ID`; an unbound external caller cannot own a
    /// process-owned coordination record — a lease or a timer (nothing would deliver a timer's body
    /// or auto-release a lease on close).
    #[error("not bound to a process; bind a session before owning a timer or lease")]
    NoBoundProcess,
    /// A scratchpad write was malformed — a blank name or an over-cap body; the message names every
    /// problem so the caller can fix it in one revision.
    #[error("scratchpad is not well-formed: {0}")]
    InvalidScratchpad(String),
    /// A scratchpad write expected a different revision than the one on record — a concurrent edit
    /// landed first, so the write was refused rather than clobbering it. `expected` is `None` for a
    /// create; `actual` is `None` when no scratchpad exists under that name.
    #[error("scratchpad revision conflict (expected {expected:?}, found {actual:?})")]
    RevisionConflict {
        expected: Option<u64>,
        actual: Option<u64>,
    },
    /// A scratchpad action named one that does not exist in the session's effective project.
    #[error("no scratchpad under that name")]
    UnknownScratchpad,
    /// A scratchpad rename targeted a name already used by another scratchpad in the project.
    #[error("a scratchpad with that name already exists")]
    ScratchpadNameTaken,
    /// A diagram write was malformed — a blank name or an over-cap source; the message names every
    /// problem so the caller can fix it in one revision.
    #[error("diagram is not well-formed: {0}")]
    InvalidDiagram(String),
    /// A diagram write expected a different revision than the one on record — a concurrent edit
    /// landed first, so the write was refused rather than clobbering it. `expected` is `None` for a
    /// create; `actual` is `None` when no diagram exists under that name.
    #[error("diagram revision conflict (expected {expected:?}, found {actual:?})")]
    DiagramRevisionConflict {
        expected: Option<u64>,
        actual: Option<u64>,
    },
    /// A diagram action named one that does not exist in the session's effective project.
    #[error("no diagram under that name")]
    UnknownDiagram,
    /// A diagram rename targeted a name already used by another diagram in the project.
    #[error("a diagram with that name already exists")]
    DiagramNameTaken,
    /// A todo write was malformed — a blank title or an over-cap body; the message names every
    /// problem so the caller can fix it in one revision.
    #[error("todo is not well-formed: {0}")]
    InvalidTodo(String),
    /// A todo update expected a different revision than the one on record — a concurrent edit landed
    /// first, so the write was refused rather than clobbering it.
    #[error("todo revision conflict (expected {expected:?}, found {actual:?})")]
    TodoRevisionConflict {
        expected: Option<u64>,
        actual: Option<u64>,
    },
    /// A todo action named one that does not exist in the session's effective project.
    #[error("no todo under that id")]
    UnknownTodo,
    /// Completing a todo was refused because it still has unmet blockers (the gate). `by` lists the
    /// blockers that are not yet done.
    #[error("todo is blocked by {by:?}")]
    TodoBlocked { by: Vec<TodoId> },
    /// A blocker referenced a todo that does not exist in the session's effective project.
    #[error("no todo under that id to block on")]
    UnknownBlocker,
    /// A todo cannot block itself.
    #[error("a todo cannot block itself")]
    SelfBlocker,
    /// A comment action named one that does not exist on the todo.
    #[error("no comment under that id on that todo")]
    UnknownComment,
    /// A template write carried malformed content; the message names every problem so the caller
    /// can fix it in one revision.
    #[error("template is not well-formed: {0}")]
    InvalidTemplate(String),
    /// A template update expected a different revision than the one on record — a concurrent edit
    /// landed first, so the write was refused rather than clobbering it.
    #[error("template revision conflict (expected {expected:?}, found {actual:?})")]
    TemplateRevisionConflict {
        expected: Option<u64>,
        actual: Option<u64>,
    },
    /// A template action named one that does not exist in the addressed scope.
    #[error("no template under that name")]
    UnknownTemplate,
    /// A template create named one that already exists in the addressed scope and kind.
    #[error("a template with that name already exists")]
    TemplateNameTaken,
    /// A `solo://` link could not be parsed — it is not in the
    /// `solo://proj/<project>/scratchpad|todo/<id>` shape.
    #[error("not a valid solo:// link")]
    MalformedLink,
    /// A `solo://` link named a project other than the caller's effective one, so it is refused
    /// rather than resolved to another project's content (the never-leak scope discipline).
    #[error("that link points outside your effective project")]
    ForeignScopeLink,
    /// A cross-project transfer named a target project the caller is not authenticated to — its
    /// connecting peer does not run there — so it is refused rather than moving content into a
    /// project the caller cannot reach (the never-widen-scope discipline). Over MCP a session
    /// authenticates to a single project, so a genuine cross-project transfer is refused here; the
    /// reachable path is the local/trusted surface.
    #[error("that project is outside your authenticated scope")]
    ForeignProject,
    /// A transfer named a target project that is not loaded, so it is refused rather than re-keying
    /// the aggregate to a project that does not exist (which would orphan it). The session-scoped
    /// surface never hits this — [`authentic_scope`](Facade::authentic_scope) already proved the
    /// target loaded — so it can only arise on the local/trusted `*_transfer_in` path.
    #[error("no such project is loaded")]
    UnknownProject,
    /// A write carried a payload larger than its kind allows, so it was refused rather than
    /// letting one write grow the durable store without bound. `what` names the payload (a kv
    /// value, a timer body); `max_bytes` is the cap it exceeded.
    #[error("{what} exceeds the {max_bytes} byte cap")]
    PayloadTooLarge {
        what: &'static str,
        max_bytes: usize,
    },
    /// A durable read or write failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Refuses a coordination payload larger than `max_bytes`, so a single write can never grow the
/// durable store without bound. `what` names the payload in the error. Shared by the coordination
/// write surfaces (timer bodies, kv values) so the one bounded-write rule lives in one place.
pub(crate) fn check_payload_size(
    len: usize,
    max_bytes: usize,
    what: &'static str,
) -> Result<(), CoordinationError> {
    if len > max_bytes {
        Err(CoordinationError::PayloadTooLarge { what, max_bytes })
    } else {
        Ok(())
    }
}

impl Facade {
    /// Clears every stale lease on launch — see [`Leases::reconcile`](crate::coordination::Leases::reconcile).
    /// Not session-scoped; the composition root calls it once at startup.
    pub fn reconcile_leases(&self) -> Result<usize, StoreError> {
        self.leases.reconcile()
    }

    /// The session's effective project, or [`CoordinationError::NoProjectScope`]. Shared with the
    /// sibling scratchpad surface ([`super::scratchpad`]), so every coordination action resolves
    /// project scope in one place.
    pub(in crate::facade) fn coordination_scope(
        &self,
        session: SessionId,
    ) -> Result<ProjectId, CoordinationError> {
        self.effective_project(session)
            .ok_or(CoordinationError::NoProjectScope)
    }

    /// The session's bound process — the owner a lease, timer, or todo lock is attributed to — or
    /// [`CoordinationError::NoBoundProcess`]. Shared with the sibling todo surface
    /// ([`super::todo`]), so process ownership resolves in one place.
    pub(in crate::facade) fn coordination_owner(
        &self,
        session: SessionId,
    ) -> Result<ProcessId, CoordinationError> {
        self.identity
            .origin(session)
            .process()
            .ok_or(CoordinationError::NoBoundProcess)
    }
}

impl ScopedFacade<'_> {
    /// Acquires the lease `key` in the session's effective project, owned by its bound process,
    /// for `ttl` (the aggregate's default when `None`, bounded by it otherwise). Non-blocking: if
    /// the key is already held by another process, returns [`AcquireOutcome::Held`] with the
    /// holder rather than waiting. Re-acquiring a key the caller already holds renews it.
    pub fn lock_acquire(
        &self,
        key: &str,
        ttl: Option<Duration>,
    ) -> Result<AcquireOutcome, CoordinationError> {
        let project = self.coordination_scope()?;
        let owner = self.coordination_owner()?;
        let outcome = self.inner.leases.acquire(project, key, owner, ttl)?;
        // Only a grant or renewal changed the lease; a `Held` outcome left another owner's lease
        // untouched, so it raises no change.
        if matches!(outcome, AcquireOutcome::Acquired(_)) {
            self.inner.bus.publish(DomainEvent::LeaseChanged {
                project,
                key: key.to_owned(),
            });
        }
        Ok(outcome)
    }

    /// The current holder of the lease `key` in the session's effective project, or `None` if it
    /// is free or has expired. A read — it needs the project scope but not a bound process.
    pub fn lock_status(&self, key: &str) -> Result<Option<LeaseView>, CoordinationError> {
        let project = self.coordination_scope()?;
        Ok(self.inner.leases.status(project, key)?)
    }

    /// Releases the lease `key` in the session's effective project if it is held by the caller's
    /// bound process, returning whether the caller's lease was released. A caller cannot release a
    /// lease another process holds.
    pub fn lock_release(&self, key: &str) -> Result<bool, CoordinationError> {
        let project = self.coordination_scope()?;
        let owner = self.coordination_owner()?;
        let released = self.inner.leases.release(project, key, owner)?;
        if released {
            self.inner.bus.publish(DomainEvent::LeaseChanged {
                project,
                key: key.to_owned(),
            });
        }
        Ok(released)
    }
}

#[cfg(test)]
#[path = "coordination_tests.rs"]
mod tests;
