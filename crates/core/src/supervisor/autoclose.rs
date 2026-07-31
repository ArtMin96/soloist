//! Closing a process when its run ends (part of context C2): the opt-in policy that frees a
//! one-shot worker's registry row and terminal buffers once its run is over and nothing is left
//! to lose by forgetting it.
//!
//! Nothing else removes a process from the registry on its own — a process that exits rests
//! there so its output stays readable, which is what the user wants for a dev server and what a
//! lead orchestrating short-lived workers does not. So this is armed per launch and never by
//! default: an armed process is [closed](super::Supervisor::close) — reaped, forgotten, buffers
//! freed — once its run has ended *and* every condition its [`ClosePolicy`] carries is met.
//!
//! Closing discards output, so the conditions are deliberately narrow. A run a caller stopped is
//! not a run that ended on its own, and neither is a crash: both leave something someone wanted
//! to look at. The registry's current status is re-read at close time rather than taken from the
//! event that woke the reactor, so a queued exit can never reap the live child of a run that has
//! since replaced it.
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
use crate::process::ProcStatus;
use crate::sync::lock;

use super::Supervisor;

/// When a launched process's registry row — and the terminal buffers behind it — are dropped
/// once it finishes. Closing throws output away, so nothing closes by default and the two
/// closing forms differ in what has to be true besides the run being over.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClosePolicy {
    /// Never: the finished process rests in the registry with its output readable, which is what
    /// the user opening a pane for it expects.
    Keep,
    /// Once its run ends on its own — a run it finished itself, not one a caller stopped and not
    /// one that crashed.
    WhenRunEnds,
    /// Once its run ends on its own **and** the handover it owes has been made, so a run whose
    /// result never reached anyone keeps its row and its output. The handover has to land before
    /// the run ends; one made afterwards simply leaves the row in place.
    WhenRunEndsAndHandedOver,
}

/// How far through its run an armed process is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Armed, but its launch has not been claimed yet. It is resting because it has not started,
    /// so this resting status is the run it has not made rather than the end of one.
    BeforeFirstRun,
    /// Its launch has been claimed, so the resting status that follows ends its run.
    Started,
}

/// One armed process: how far through its run it is, and what still stands between it and being
/// forgotten.
#[derive(Clone, Copy)]
struct Armed {
    phase: Phase,
    /// Set when a caller asked this process to stop, so the run it is ending was ended *for* it.
    /// Its row and scrollback stay: a stop is someone wanting to see what happened.
    stop_requested: bool,
    /// Whether the handover this arming waits on has been made. True from the start under a
    /// policy that waits on none.
    handed_over: bool,
}

impl Armed {
    /// Whether everything this arming waits on has happened, leaving only the registry's own
    /// account of the process to confirm.
    fn is_satisfied(&self) -> bool {
        self.phase == Phase::Started && !self.stop_requested && self.handed_over
    }
}

/// Which processes are to be closed when their run ends, and what each is still waiting on.
/// Cloneable; all clones share one state so the supervisor and its reactor agree. Bounded by
/// the live process set — an entry is dropped as soon as its process leaves the registry.
#[derive(Clone, Default)]
pub(crate) struct AutoClose {
    armed: Arc<Mutex<HashMap<ProcessId, Armed>>>,
}

impl AutoClose {
    /// Arms `id` under `policy` for the run it is about to make. [`ClosePolicy::Keep`] arms
    /// nothing, so the finished process simply rests.
    fn arm(&self, id: ProcessId, policy: ClosePolicy) {
        let handed_over = match policy {
            ClosePolicy::Keep => return,
            ClosePolicy::WhenRunEnds => true,
            ClosePolicy::WhenRunEndsAndHandedOver => false,
        };
        lock(&self.armed).insert(
            id,
            Armed {
                phase: Phase::BeforeFirstRun,
                stop_requested: false,
                handed_over,
            },
        );
    }

    /// Records that `id`'s launch has been claimed, so the resting status that follows ends its
    /// run. Read from the launch path rather than the event stream: a dropped status delta would
    /// otherwise leave an armed process looking as though it had never run, and its row would be
    /// stranded for the rest of the session. A no-op for a process that was never armed.
    pub(super) fn observe_launch(&self, id: ProcessId) {
        if let Some(armed) = lock(&self.armed).get_mut(&id) {
            armed.phase = Phase::Started;
        }
    }

    /// Records that a caller asked `id` to stop, so the run it is ending did not end on its own.
    /// A no-op for a process that was never armed.
    pub(super) fn observe_stop_request(&self, id: ProcessId) {
        if let Some(armed) = lock(&self.armed).get_mut(&id) {
            armed.stop_requested = true;
        }
    }

    /// Records that `id` has made the handover its arming waits on. A no-op for a process armed
    /// under a policy that waits on none, or never armed at all.
    fn record_handover(&self, id: ProcessId) {
        if let Some(armed) = lock(&self.armed).get_mut(&id) {
            armed.handed_over = true;
        }
    }

    /// Whether `id`'s arming is satisfied, so only its current registry status stands between it
    /// and being closed.
    fn is_satisfied(&self, id: ProcessId) -> bool {
        lock(&self.armed).get(&id).is_some_and(Armed::is_satisfied)
    }

    /// Drops `id`'s arming — it has been closed, or has otherwise left the registry.
    fn forget(&self, id: ProcessId) {
        lock(&self.armed).remove(&id);
    }

    /// Every armed process whose launch has been claimed, so the reactor can re-check whose run
    /// has since ended when its event stream lags.
    fn started(&self) -> Vec<ProcessId> {
        lock(&self.armed)
            .iter()
            .filter(|(_, armed)| armed.phase == Phase::Started)
            .map(|(id, _)| *id)
            .collect()
    }
}

impl Supervisor {
    /// Arms `id` under `policy` for the run it is about to make. Called before the launch, so a
    /// run that ends immediately is still caught. Nothing is armed by default: closing a process
    /// discards its output, which is only ever the caller's own call to make.
    pub(crate) fn close_when_done(&self, id: ProcessId, policy: ClosePolicy) {
        self.auto_close.arm(id, policy);
    }

    /// Records that `id` has handed over what its arming waits on, releasing the last condition
    /// on forgetting it. A no-op unless it was armed under
    /// [`ClosePolicy::WhenRunEndsAndHandedOver`].
    pub(crate) fn record_handover(&self, id: ProcessId) {
        self.auto_close.record_handover(id);
    }

    /// Closes `id` if its arming is satisfied and the registry still says its run ended on its
    /// own. The status is re-read here rather than carried on the event that woke the reactor:
    /// a queued exit can describe a run a restart has already replaced, and acting on it would
    /// reap a live child mid-run. Disarmed first, so the
    /// [`ProcessRemoved`](DomainEvent::ProcessRemoved) the close announces cannot drive a second
    /// attempt. A close that finds nothing registered is already what was wanted.
    async fn close_if_run_ended(&self, id: ProcessId) {
        if !self.auto_close.is_satisfied(id) {
            return;
        }
        // Only a clean self-exit rests at `Stopped`: a crash and a restart-exhausted command both
        // leave a terminal status of their own, and their output is the reason to keep the row.
        if self.registry.status(id) != Some(ProcStatus::Stopped) {
            return;
        }
        self.auto_close.forget(id);
        let _ = self.close(id).await;
    }

    /// Re-drives the policy from the current registry after the reactor's event stream lagged: a
    /// finished process emits nothing further, so without this a dropped terminal delta would
    /// strand an armed process in the registry forever — the very leak the arming asked to
    /// avoid. Only a process whose launch was claimed is considered, so one still waiting to
    /// start is left alone.
    async fn rescan_finished(&self) {
        for id in self.auto_close.started() {
            match self.registry.status(id) {
                // Gone from the registry already: there is nothing to close, only to forget.
                None => self.auto_close.forget(id),
                Some(_) => self.close_if_run_ended(id).await,
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
                    DomainEvent::ProcessStatusChanged { id, to, .. } if !to.is_active() => {
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
