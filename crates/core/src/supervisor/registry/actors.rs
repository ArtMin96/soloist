//! Launching, signalling, and retiring a process's actor (context C2) — the registry half of the
//! single-writer rule.
//!
//! A process's actor solely owns its child, PTY, and exit watcher; everything else reaches it by
//! message through the control surface held here. Claiming a launch installs that surface under the
//! same lock as the status check, so a stop or shutdown arriving in the launch window still finds a
//! mailbox to message — the command is neither lost nor falsely reported delivered.

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::super::actor::ActorMsg;
use super::{lock, ActorHandle, Registry};
use crate::ids::ProcessId;
use crate::process::ProcStatus;

impl Registry {
    /// Atomically claims the right to launch a resting process: if it is not already
    /// active, transitions it to `Starting` under the lock, installs the actor `mailbox`,
    /// and returns its prior status; if it is already active (another launch won, or it is
    /// running), returns `None` and drops the mailbox. Setting the status *and* the control
    /// surface under the same lock as the check is what makes the supervisor's start path
    /// race-free without holding the lock across the spawn: from the instant the process is
    /// `Starting`, a concurrent stop or shutdown finds a mailbox to message, so a command in
    /// the launch window is neither lost nor falsely reported delivered. The join handle is
    /// [`attached`](Self::attach_join) once the task is spawned.
    pub(crate) fn begin_launch(
        &self,
        id: ProcessId,
        mailbox: mpsc::Sender<ActorMsg>,
    ) -> Option<ProcStatus> {
        let mut guard = lock(&self.inner);
        let entry = guard.get_mut(&id)?;
        let from = entry.view.status;
        if from.is_active() {
            return None;
        }
        let next = from.transition(ProcStatus::Starting).ok()?;
        entry.view.status = next;
        entry.view.exit_code = None;
        entry.handle = Some(ActorHandle {
            mailbox,
            join: None,
        });
        Some(from)
    }

    /// Atomically holds a still-crashed process in [`ProcStatus::RestartExhausted`]: if it
    /// is currently `Crashed`, transitions it and returns `true`; otherwise leaves it
    /// untouched and returns `false`. Because the FSM permits `RestartExhausted` from no
    /// state but `Crashed`, a concurrent user start/restart (now `Starting` or running) is
    /// never clobbered — the transition simply fails. The caller publishes the status
    /// delta after this returns, as with [`Registry::begin_launch`].
    pub(crate) fn exhaust_if_crashed(&self, id: ProcessId) -> bool {
        let mut guard = lock(&self.inner);
        let Some(entry) = guard.get_mut(&id) else {
            return false;
        };
        match entry.view.status.transition(ProcStatus::RestartExhausted) {
            Ok(next) => {
                entry.view.status = next;
                entry.view.exit_code = None;
                true
            }
            Err(_) => false,
        }
    }

    /// Attaches the join handle of a freshly spawned actor to the mailbox
    /// [`begin_launch`](Self::begin_launch) already installed. If the process was closed or
    /// removed in the launch window (its handle is gone), the launch has been superseded:
    /// abort the orphaned task rather than leak one that would spawn a child no one owns.
    pub(crate) fn attach_join(&self, id: ProcessId, join: JoinHandle<()>) {
        let mut guard = lock(&self.inner);
        match guard.get_mut(&id).and_then(|entry| entry.handle.as_mut()) {
            Some(handle) => handle.join = Some(join),
            None => join.abort(),
        }
    }

    /// Removes and returns a *fully attached* actor handle (mailbox + join) for shutdown to
    /// message then await. Returns `None` — leaving the entry intact — when the launch has
    /// installed the mailbox but not yet attached the join (the launch window), so the caller
    /// stops it in place via [`signal`](Self::signal) and retries once the join attaches,
    /// rather than taking a handle it cannot await.
    pub(crate) fn take_handle(&self, id: ProcessId) -> Option<ActorHandle> {
        let mut guard = lock(&self.inner);
        let entry = guard.get_mut(&id)?;
        // Only take it once the join is attached; a mid-launch entry keeps its mailbox so a
        // retry (or an in-place signal) still reaches the actor.
        match &entry.handle {
            Some(handle) if handle.join.is_some() => entry.handle.take(),
            _ => None,
        }
    }

    /// Best-effort messages a live actor without removing its handle — the single "tell the actor"
    /// primitive behind [`Supervisor::stop`](super::Supervisor::stop),
    /// [`Supervisor::restart`](super::Supervisor::restart), and shutdown's in-place stop for a
    /// mid-launch actor whose join is not yet attached (so it stops before it can spawn). Since the
    /// mailbox is installed as the launch is claimed, an active process always has one here — even
    /// in its launch window. A full mailbox or a resting process is a harmless no-op; the `try_send`
    /// never blocks, so holding the lock across it is a tiny, bounded critical section.
    pub(crate) fn signal(&self, id: ProcessId, message: ActorMsg) {
        let guard = lock(&self.inner);
        if let Some(handle) = guard.get(&id).and_then(|entry| entry.handle.as_ref()) {
            let _ = handle.mailbox.try_send(message);
        }
    }

    /// Removes a process from the registry entirely and hands back its actor handle if it held
    /// one — the one path that forgets a managed process, unlike a stop, which leaves it
    /// resting. Returns [`None`] if it was not registered. Taking the entry and its handle
    /// under a single lock lets [`Supervisor::close`] forget the process *before* it reaps, so
    /// a concurrent crash auto-restart finds no entry to relaunch and cannot leave a child
    /// orphaned behind the removal.
    pub(crate) fn remove_returning_handle(&self, id: ProcessId) -> Option<Option<ActorHandle>> {
        lock(&self.inner).remove(&id).map(|entry| entry.handle)
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
}
