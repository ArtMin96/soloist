//! Closing a process when its run ends (part of context C2): the opt-in policy that frees a
//! one-shot worker's registry row and terminal buffers the moment it finishes.
//!
//! Nothing else removes a process from the registry on its own — a process that exits rests
//! there so its output stays readable, which is what the user wants for a dev server and what a
//! lead orchestrating short-lived workers does not. So this is armed per launch and never by
//! default: an armed process is [closed](super::Supervisor::close) — reaped, forgotten, buffers
//! freed — on the terminal transition that ends its run, exactly as an explicit close would.
//!
//! Two pieces, mirroring the crash policy in [`restart`](super::restart): [`AutoClose`], the
//! shared per-process arming state, and the reactor
//! ([`Supervisor::auto_close_loop`]) that watches the process event stream and spends it.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast::error::RecvError;

use crate::events::DomainEvent;
use crate::ids::ProcessId;
use crate::sync::lock;

use super::Supervisor;

/// How far through its run an armed process is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Armed, but not yet seen active. It is resting because it has not launched yet, so this
    /// resting status is the run it has not made rather than the end of one.
    BeforeFirstRun,
    /// Seen active, so the resting status that follows ends its run.
    Started,
}

/// Which processes are to be closed when their run ends, and how far through it each is.
/// Cloneable; all clones share one state so the supervisor and its reactor agree. Bounded by
/// the live process set — an entry is dropped as soon as its process leaves the registry.
#[derive(Clone, Default)]
pub(crate) struct AutoClose {
    armed: Arc<Mutex<HashMap<ProcessId, Phase>>>,
}

impl AutoClose {
    /// Arms `id` to be closed when the run it is about to make ends.
    fn arm(&self, id: ProcessId) {
        lock(&self.armed).insert(id, Phase::BeforeFirstRun);
    }

    /// Records that `id` is active, so the resting status that follows ends its run. A no-op
    /// for a process that was never armed.
    fn observe_active(&self, id: ProcessId) {
        if let Some(phase) = lock(&self.armed).get_mut(&id) {
            *phase = Phase::Started;
        }
    }

    /// Whether `id` is armed and has started, so a resting status now ends its run.
    fn run_has_ended(&self, id: ProcessId) -> bool {
        lock(&self.armed).get(&id) == Some(&Phase::Started)
    }

    /// Drops `id`'s arming — it has been closed, or has otherwise left the registry.
    fn forget(&self, id: ProcessId) {
        lock(&self.armed).remove(&id);
    }

    /// Every armed process that has started, so the reactor can re-check whose run has since
    /// ended when its event stream lags.
    fn started(&self) -> Vec<ProcessId> {
        lock(&self.armed)
            .iter()
            .filter(|(_, phase)| **phase == Phase::Started)
            .map(|(id, _)| *id)
            .collect()
    }
}

impl Supervisor {
    /// Arms `id` to be closed once the run it is about to make ends — stopped, exited, or
    /// crashed alike, since each is that run finishing. Called before the launch, so a run that
    /// ends immediately is still caught. Nothing is armed by default: closing a process discards
    /// its output, which is only ever the caller's own call to make.
    pub(crate) fn close_when_done(&self, id: ProcessId) {
        self.auto_close.arm(id);
    }

    /// Closes `id` if it is armed and its run has just ended. Disarmed first, so the
    /// [`ProcessRemoved`](DomainEvent::ProcessRemoved) the close announces cannot drive a second
    /// attempt. A close that finds nothing registered is already what was wanted.
    async fn close_if_run_ended(&self, id: ProcessId) {
        if !self.auto_close.run_has_ended(id) {
            return;
        }
        self.auto_close.forget(id);
        let _ = self.close(id).await;
    }

    /// Re-drives the policy from the current registry after the reactor's event stream lagged: a
    /// finished process emits nothing further, so without this a dropped terminal delta would
    /// strand an armed process in the registry forever — the very leak the arming asked to
    /// avoid. Only a started process is considered, so one still waiting to launch is left alone.
    async fn rescan_finished(&self) {
        for id in self.auto_close.started() {
            match self.registry.status(id) {
                // Gone from the registry already: there is nothing to close, only to forget.
                None => self.auto_close.forget(id),
                Some(status) if !status.is_active() => self.close_if_run_ended(id).await,
                Some(_) => {}
            }
        }
    }

    /// The auto-close reactor loop: watch the process event stream and close each armed process
    /// as its run ends. Returned as a future for the composition root to spawn on its runtime.
    /// Holds only a [`std::sync::Weak`] reference, so it ends when the supervisor is dropped
    /// (app shutdown) instead of keeping it alive; start it once.
    pub(crate) fn auto_close_loop(self: &Arc<Self>) -> impl Future<Output = ()> + Send + 'static {
        let weak = Arc::downgrade(self);
        let mut events = self.bus.subscribe();
        async move {
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    Err(RecvError::Lagged(_)) => {
                        match weak.upgrade() {
                            Some(sup) => sup.rescan_finished().await,
                            None => break,
                        }
                        continue;
                    }
                    Err(RecvError::Closed) => break,
                };
                let Some(sup) = weak.upgrade() else { break };
                match event {
                    DomainEvent::ProcessStatusChanged { id, to, .. } if to.is_active() => {
                        sup.auto_close.observe_active(id);
                    }
                    DomainEvent::ProcessStatusChanged { id, .. } => {
                        sup.close_if_run_ended(id).await;
                    }
                    // Closed by someone else, or dropped with its project: nothing left to close.
                    DomainEvent::ProcessRemoved { id } => sup.auto_close.forget(id),
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "autoclose_tests.rs"]
mod tests;
