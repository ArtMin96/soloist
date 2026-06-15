//! The in-memory process registry: the supervisor's authoritative map of managed
//! processes plus, while one is running, the handle used to message its actor.
//!
//! The registry's `Mutex` guards only the lookup map. Each entry has exactly one
//! writer for its lifecycle state — the actor that owns it — so there is no shared
//! mutable domain state behind a lock beyond the map itself.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::hash::Hash;
use crate::ids::{ProcessId, ProjectId};
use crate::ports::SpawnSpec;
use crate::process::{ProcStatus, ProcessKind, ProcessView};
use crate::sync::lock;

use super::actor::ActorMsg;

/// A live actor's control surface: a bounded mailbox to message it and the join
/// handle awaited at shutdown to confirm its child was reaped.
pub(crate) struct ActorHandle {
    pub(crate) mailbox: mpsc::Sender<ActorMsg>,
    pub(crate) join: JoinHandle<()>,
}

/// One managed process: its read-model view, everything needed to (re)launch it, and
/// — while running — its actor handle.
struct Managed {
    view: ProcessView,
    launch: SpawnSpec,
    trust_variant: Option<Hash>,
    auto_start: bool,
    handle: Option<ActorHandle>,
}

/// A cloned read of one entry's launch-relevant fields, taken under the lock so the
/// supervisor can make trust and eligibility decisions without holding it.
pub(crate) struct EntryInfo {
    pub(crate) project: ProjectId,
    pub(crate) status: ProcStatus,
    pub(crate) trust_variant: Option<Hash>,
    pub(crate) launch: SpawnSpec,
}

/// A start candidate: its id, trust variant (if trust-gated), and launch spec.
pub(crate) struct Candidate {
    pub(crate) id: ProcessId,
    pub(crate) trust_variant: Option<Hash>,
    pub(crate) launch: SpawnSpec,
}

/// The process registry. Cloneable; all clones share one map.
#[derive(Clone, Default)]
pub struct Registry {
    inner: Arc<Mutex<HashMap<ProcessId, Managed>>>,
}

impl Registry {
    /// Records a freshly registered (stopped) process.
    pub(crate) fn add(
        &self,
        view: ProcessView,
        launch: SpawnSpec,
        trust_variant: Option<Hash>,
        auto_start: bool,
    ) {
        let mut guard = lock(&self.inner);
        guard.insert(
            view.id,
            Managed {
                view,
                launch,
                trust_variant,
                auto_start,
                handle: None,
            },
        );
    }

    /// The launch-relevant fields for `id`, if present.
    pub(crate) fn describe(&self, id: ProcessId) -> Option<EntryInfo> {
        let guard = lock(&self.inner);
        let entry = guard.get(&id)?;
        Some(EntryInfo {
            project: entry.view.project,
            status: entry.view.status,
            trust_variant: entry.trust_variant,
            launch: entry.launch.clone(),
        })
    }

    /// The current cached status for `id`, if present.
    pub(crate) fn status(&self, id: ProcessId) -> Option<ProcStatus> {
        lock(&self.inner).get(&id).map(|entry| entry.view.status)
    }

    /// Updates the cached status and exit code for `id` (no-op once removed).
    pub(crate) fn set_status(&self, id: ProcessId, status: ProcStatus, exit_code: Option<i32>) {
        let mut guard = lock(&self.inner);
        if let Some(entry) = guard.get_mut(&id) {
            entry.view.status = status;
            entry.view.exit_code = exit_code;
        }
    }

    /// Atomically claims the right to launch a resting process: if it is not already
    /// active, transitions it to `Starting` under the lock and returns its prior
    /// status; if it is already active (another launch won, or it is running), returns
    /// `None`. Setting the status under the same lock as the check is what makes the
    /// supervisor's start path race-free without holding the lock across the spawn.
    pub(crate) fn begin_launch(&self, id: ProcessId) -> Option<ProcStatus> {
        let mut guard = lock(&self.inner);
        let entry = guard.get_mut(&id)?;
        let from = entry.view.status;
        if from.is_active() {
            return None;
        }
        let next = from.transition(ProcStatus::Starting).ok()?;
        entry.view.status = next;
        entry.view.exit_code = None;
        Some(from)
    }

    /// Stores the handle of a freshly launched actor.
    pub(crate) fn set_handle(&self, id: ProcessId, handle: ActorHandle) {
        let mut guard = lock(&self.inner);
        if let Some(entry) = guard.get_mut(&id) {
            entry.handle = Some(handle);
        }
    }

    /// Clones a live actor's mailbox to message it without removing its handle, so a
    /// restart keeps the same actor task.
    pub(crate) fn mailbox(&self, id: ProcessId) -> Option<mpsc::Sender<ActorMsg>> {
        let guard = lock(&self.inner);
        guard
            .get(&id)
            .and_then(|entry| entry.handle.as_ref().map(|h| h.mailbox.clone()))
    }

    /// Removes and returns a live actor's handle, used at shutdown to message then
    /// await it.
    pub(crate) fn take_handle(&self, id: ProcessId) -> Option<ActorHandle> {
        let mut guard = lock(&self.inner);
        guard.get_mut(&id).and_then(|entry| entry.handle.take())
    }

    /// Every process that still holds an actor handle — the shutdown set. Messaging
    /// then awaiting each reaps any child still alive; it is a harmless no-op for an
    /// actor that already finished but whose handle was not reclaimed.
    pub(crate) fn with_live_actor(&self) -> Vec<ProcessId> {
        let guard = lock(&self.inner);
        guard
            .values()
            .filter(|entry| entry.handle.is_some())
            .map(|entry| entry.view.id)
            .collect()
    }

    /// Active processes within `project` — the targets of `stop_all`.
    pub(crate) fn live_in(&self, project: ProjectId) -> Vec<ProcessId> {
        let guard = lock(&self.inner);
        guard
            .values()
            .filter(|entry| entry.view.project == project && entry.view.status.is_active())
            .map(|entry| entry.view.id)
            .collect()
    }

    /// Running processes within `project` — the targets of `restart_running`.
    pub(crate) fn running_in(&self, project: ProjectId) -> Vec<ProcessId> {
        let guard = lock(&self.inner);
        guard
            .values()
            .filter(|entry| {
                entry.view.project == project && entry.view.status == ProcStatus::Running
            })
            .map(|entry| entry.view.id)
            .collect()
    }

    /// Stopped, `auto_start` commands within `project` — the candidates `start_all`
    /// trust-checks before launching.
    pub(crate) fn auto_start_candidates(&self, project: ProjectId) -> Vec<Candidate> {
        let guard = lock(&self.inner);
        guard
            .values()
            .filter(|entry| {
                entry.view.project == project
                    && entry.view.kind == ProcessKind::Command
                    && entry.auto_start
                    && entry.view.status == ProcStatus::Stopped
            })
            .map(|entry| Candidate {
                id: entry.view.id,
                trust_variant: entry.trust_variant,
                launch: entry.launch.clone(),
            })
            .collect()
    }

    /// A cloned snapshot of every process view — the read model adapters render.
    pub fn snapshot(&self) -> Vec<ProcessView> {
        lock(&self.inner)
            .values()
            .map(|entry| entry.view.clone())
            .collect()
    }
}
