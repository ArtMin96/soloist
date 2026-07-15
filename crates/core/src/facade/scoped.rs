//! [`ScopedFacade`] — the seam a session-scoped caller (MCP today, any future remote surface) acts
//! through: the type, the refusal taxonomy, and the scope guard every scoped method spends.
//!
//! The methods themselves live with the domain they act on (`scoped_process`, `output`, `todo`,
//! `scratchpad`, `coordination`, `kv`, `link`, `session`, `prompt_template`, `support`), each
//! resolving the calling session's **effective-project scope** through the guard here before
//! routing to the one core behaviour: an action can touch only a process within its project, and
//! the trust gate in C2 still refuses an untrusted command. The rule is written once, in the core,
//! so every remote adapter inherits the identical guarantee instead of re-checking it per adapter.
//!
//! Scope is a **type**, not a convention. [`Facade`] hands out whole contexts by reference
//! (`supervisor()`, `projects()`, …) because the local user driving the Tauri UI is not
//! scope-limited; a caller who *is* limited must not be able to reach those, and here it cannot:
//! `ScopedFacade` exposes no accessor, so there is no ungated door to pick by accident. Binding
//! the session once also takes it out of every signature — a caller cannot pass the wrong one.

use super::{Facade, LaunchAgentError};
use crate::ids::{ProcessId, ProjectId, SessionId};
use crate::ports::StoreError;
use crate::process::ProcessView;
use crate::supervisor::SupervisorError;

/// The session-scoped view of the core: one caller's authority, bound to its session.
///
/// Borrows the [`Facade`] rather than sharing it, so an adapter already holding one (`&Facade`,
/// `Arc<Facade>`) makes a scoped view per request for free.
///
/// The guarantee is that this type has no way out. [`Facade`] hands whole contexts to the local
/// user (`supervisor()`, `projects()`, `trust()`, `agents()`, `config()`); a scope-limited caller
/// holding only a `ScopedFacade` cannot reach any of them, so it cannot route around its own
/// guard. That is enforced by the compiler, not by reviewers noticing:
///
/// ```compile_fail
/// # fn probe(scoped: soloist_core::ScopedFacade<'_>) {
/// scoped.supervisor();
/// # }
/// ```
///
/// ```compile_fail
/// # fn probe(scoped: soloist_core::ScopedFacade<'_>) {
/// scoped.projects();
/// # }
/// ```
///
/// ```compile_fail
/// # fn probe(scoped: soloist_core::ScopedFacade<'_>) {
/// scoped.trust();
/// # }
/// ```
///
/// ```compile_fail
/// # fn probe(scoped: soloist_core::ScopedFacade<'_>) {
/// scoped.agents();
/// # }
/// ```
///
/// ```compile_fail
/// # fn probe(scoped: soloist_core::ScopedFacade<'_>) {
/// scoped.config();
/// # }
/// ```
pub struct ScopedFacade<'a> {
    // Visible to the sibling modules that carry the rest of this type's methods, and no wider:
    // outside `facade` there is no way to reach the unscoped core through this view.
    pub(in crate::facade) inner: &'a Facade,
    pub(in crate::facade) session: SessionId,
}

impl Facade {
    /// This core as `session` may act on it: only that session's scoped surface, with no
    /// accessor onto an ungated context.
    pub fn scoped(&self, session: SessionId) -> ScopedFacade<'_> {
        ScopedFacade {
            inner: self,
            session,
        }
    }
}

impl ScopedFacade<'_> {
    /// The session this view acts as — for an adapter that must name it on the wire.
    pub fn session(&self) -> SessionId {
        self.session
    }

    /// The session's effective project, or the coordination refusal when it has none.
    pub(in crate::facade) fn coordination_scope(
        &self,
    ) -> Result<ProjectId, super::CoordinationError> {
        self.inner.coordination_scope(self.session)
    }

    /// The process this session owns coordination state as, or the coordination refusal when it
    /// is not bound to one.
    pub(in crate::facade) fn coordination_owner(
        &self,
    ) -> Result<ProcessId, super::CoordinationError> {
        self.inner.coordination_owner(self.session)
    }
}

/// Why a session-scoped process action was refused. The wire adapters map this to their
/// own error type, so the taxonomy is defined once here.
#[derive(Debug, thiserror::Error)]
pub enum ScopedActionError {
    /// No process is registered under that id.
    #[error("no such process")]
    UnknownProcess,
    /// The session has no project in scope to act within (none selected, bound, or singular).
    #[error("no project is in scope; select one first")]
    NoProjectScope,
    /// The process exists but belongs to a different project than the session's scope.
    #[error("that process belongs to a different project")]
    OutOfScope,
    /// The command is not trusted to run in this project (the C2 trust gate refused it).
    #[error("command is not trusted to run in this project")]
    Untrusted,
    /// A durable read failed while resolving scope.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Why spawning a worker agent over a scoped session failed: no project is in scope, the
/// caller is itself a spawned worker, or the underlying launch failed (unknown tool, unknown
/// project, store, or supervisor).
#[derive(Debug, thiserror::Error)]
pub enum SpawnAgentError {
    /// The session has no project in scope to spawn the worker into.
    #[error("no project is in scope; select one first")]
    NoProjectScope,
    /// The calling session is bound to a process that was itself spawned as a worker this
    /// run — delegation is one level deep, so a worker may not spawn its own workers.
    #[error("a worker agent cannot spawn agents; report back to the lead that spawned it")]
    WorkerMayNotSpawn,
    /// The launch itself failed — see [`LaunchAgentError`].
    #[error(transparent)]
    Launch(#[from] LaunchAgentError),
}

impl From<SupervisorError> for ScopedActionError {
    /// Projects a supervisor refusal onto the scoped taxonomy. The scope guard runs first, so
    /// a `NotFound` here means the process was forgotten between checks — reported as unknown.
    fn from(err: SupervisorError) -> Self {
        match err {
            SupervisorError::NotFound(_) => ScopedActionError::UnknownProcess,
            SupervisorError::Untrusted => ScopedActionError::Untrusted,
            SupervisorError::Store(err) => ScopedActionError::Store(err),
            // Resume is a local-only affordance (the UI's "Resume last session"), never a
            // scoped session action, so a scoped call cannot produce this — map it to the
            // closest scoped refusal for exhaustiveness.
            SupervisorError::NotResumable(_) => ScopedActionError::UnknownProcess,
        }
    }
}

impl ScopedFacade<'_> {
    /// Resolves the session's effective project for a project-wide action, or
    /// `NoProjectScope` when none is selected, bound, or singular.
    pub(in crate::facade) fn scope(&self) -> Result<ProjectId, ScopedActionError> {
        self.inner
            .effective_project(self.session)
            .ok_or(ScopedActionError::NoProjectScope)
    }

    /// The scope guard, returning the in-scope process's view: the process must exist and
    /// belong to the session's effective project, else `UnknownProcess`/`OutOfScope`. The
    /// scoped actions and reads share this one resolution, so the rule lives in a single place.
    pub(in crate::facade) fn resolve_in_scope(
        &self,
        process: ProcessId,
    ) -> Result<ProcessView, ScopedActionError> {
        let view = self
            .inner
            .process_view(process)
            .ok_or(ScopedActionError::UnknownProcess)?;
        if view.project != self.scope()? {
            return Err(ScopedActionError::OutOfScope);
        }
        Ok(view)
    }

    /// The scope guard when the caller needs only the pass/fail, not the view. Public for the
    /// one remote read whose own return shape differs from the scoped reads — the async
    /// `wait_for_bound_port`, which confirms scope, then awaits — so its cross-project
    /// port-bind probe is refused like the other reads. The scope *rule* still lives here.
    pub fn require_in_scope(&self, process: ProcessId) -> Result<(), ScopedActionError> {
        self.resolve_in_scope(process).map(|_| ())
    }
}

#[cfg(test)]
#[path = "scoped_tests.rs"]
mod tests;
