//! Process registry, the supervised process actor, and panic isolation.
//!
//! Each managed process is one supervised `tokio` task (the *actor*) that solely
//! owns its child handle and control, interacting with the rest of the core only by
//! publishing [`DomainEvent`]s and updating its own registry entry. There is no
//! shared mutable domain state behind a lock: the registry's `Mutex` guards only the
//! lookup map, and each entry has exactly one writer — the actor that owns that id.
//!
//! The actor is wrapped in a panic-isolation boundary so a panic inside one process
//! marks just that unit [`ProcStatus::Crashed`] and never takes down the supervisor.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::events::{DomainEvent, EventBus};
use crate::ids::ProcessId;
use crate::ports::{Clock, ProcessSpawner, SpawnSpec, Spawned};
use crate::process::{ProcStatus, ProcessView};
use crate::sync::lock;

/// Grace window between SIGTERM and SIGKILL on a graceful stop. The *timing* is a
/// core policy (so it is testable against the mock clock); the *signalling* is the
/// adapter's job. Real lifecycle tuning arrives with the supervisor phase.
const STOP_GRACE: Duration = Duration::from_secs(5);

struct ProcEntry {
    view: ProcessView,
    cancel: CancellationToken,
}

/// The in-memory process registry: a cheap projection adapters read via `snapshot`,
/// plus the cancellation handle used to stop each process. Cloneable; all clones
/// share one map.
#[derive(Clone, Default)]
pub struct Registry {
    inner: Arc<Mutex<HashMap<ProcessId, ProcEntry>>>,
}

impl Registry {
    /// Records a freshly created process and its cancellation handle.
    pub(crate) fn insert(&self, view: ProcessView, cancel: CancellationToken) {
        let mut guard = lock(&self.inner);
        guard.insert(view.id, ProcEntry { view, cancel });
    }

    /// Updates the cached status for `id` (no-op if it has left the registry).
    pub(crate) fn set_status(&self, id: ProcessId, status: ProcStatus) {
        let mut guard = lock(&self.inner);
        if let Some(entry) = guard.get_mut(&id) {
            entry.view.status = status;
        }
    }

    /// The last cached status for `id`, if still present.
    pub(crate) fn last_status(&self, id: ProcessId) -> Option<ProcStatus> {
        let guard = lock(&self.inner);
        guard.get(&id).map(|entry| entry.view.status)
    }

    /// Signals the actor for `id` to stop. Returns whether a process was found.
    pub(crate) fn cancel(&self, id: ProcessId) -> bool {
        let guard = lock(&self.inner);
        match guard.get(&id) {
            Some(entry) => {
                entry.cancel.cancel();
                true
            }
            None => false,
        }
    }

    /// A cloned snapshot of every process view — the read model adapters render.
    pub fn snapshot(&self) -> Vec<ProcessView> {
        let guard = lock(&self.inner);
        guard.values().map(|entry| entry.view.clone()).collect()
    }
}

/// Applies a state transition through the FSM, updating the registry and publishing
/// the delta. An illegal transition is refused (current state retained) rather than
/// forced — the FSM is the contract.
fn transition(
    registry: &Registry,
    bus: &EventBus,
    id: ProcessId,
    status: &mut ProcStatus,
    to: ProcStatus,
) {
    let from = *status;
    if let Ok(new) = from.transition(to) {
        *status = new;
        registry.set_status(id, new);
        bus.publish(DomainEvent::ProcessStatusChanged { id, from, to: new });
    }
}

/// Spawns the supervised actor for one process inside a panic-isolation boundary.
///
/// The actor's body runs in a child task; if it panics, this wrapper catches the
/// `JoinError`, marks the process [`ProcStatus::Crashed`], and returns — the rest of
/// the app is unaffected. Must be called from within a `tokio` runtime.
pub(crate) fn spawn_supervised(
    id: ProcessId,
    spec: SpawnSpec,
    spawner: Arc<dyn ProcessSpawner>,
    clock: Arc<dyn Clock>,
    bus: EventBus,
    registry: Registry,
    cancel: CancellationToken,
) {
    tokio::spawn(async move {
        let body = {
            let (spawner, clock, bus, registry, cancel) = (
                spawner.clone(),
                clock.clone(),
                bus.clone(),
                registry.clone(),
                cancel.clone(),
            );
            tokio::spawn(run_actor(id, spec, spawner, clock, bus, registry, cancel))
        };

        if let Err(join_err) = body.await {
            if join_err.is_panic() {
                let from = registry.last_status(id).unwrap_or(ProcStatus::Running);
                registry.set_status(id, ProcStatus::Crashed);
                bus.publish(DomainEvent::ProcessStatusChanged {
                    id,
                    from,
                    to: ProcStatus::Crashed,
                });
            }
        }
    });
}

/// The actor body: spawn the child, mark it running, then race "child exited"
/// against "stop requested". On stop, drive the graceful SIGTERM → grace → SIGKILL
/// sequence and reap. The entry is left in a terminal state so `snapshot` reflects
/// the outcome.
async fn run_actor(
    id: ProcessId,
    spec: SpawnSpec,
    spawner: Arc<dyn ProcessSpawner>,
    clock: Arc<dyn Clock>,
    bus: EventBus,
    registry: Registry,
    cancel: CancellationToken,
) {
    // The facade has already inserted this process as `Starting`.
    let mut status = ProcStatus::Starting;

    let spawned = match spawner.spawn(&spec).await {
        Ok(spawned) => spawned,
        Err(_err) => {
            transition(&registry, &bus, id, &mut status, ProcStatus::Crashed);
            return;
        }
    };

    let Spawned {
        pid: _,
        mut exit,
        mut control,
    } = spawned;

    transition(&registry, &bus, id, &mut status, ProcStatus::Running);

    tokio::select! {
        final_status = &mut exit => {
            // The child exited on its own before any stop was requested.
            let to = if final_status.success() {
                ProcStatus::Stopped
            } else {
                ProcStatus::Crashed
            };
            transition(&registry, &bus, id, &mut status, to);
        }
        _ = cancel.cancelled() => {
            transition(&registry, &bus, id, &mut status, ProcStatus::Stopping);
            let _ = control.terminate().await;
            tokio::select! {
                _ = &mut exit => {
                    // Exited within the grace window; already reaped.
                }
                _ = clock.sleep(STOP_GRACE) => {
                    let _ = control.kill().await;
                    let _ = exit.await; // reap the killed child
                }
            }
            transition(&registry, &bus, id, &mut status, ProcStatus::Stopped);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::DomainEvent;
    use crate::process::{ProcessKind, ProcessView};
    use crate::testing::{FakeSpawner, MockClock};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::broadcast::error::RecvError;

    fn seed(registry: &Registry, id: ProcessId, cancel: &CancellationToken) {
        registry.insert(
            ProcessView {
                id,
                kind: ProcessKind::Command,
                label: "demo".into(),
                status: ProcStatus::Starting,
            },
            cancel.clone(),
        );
    }

    async fn next_status(rx: &mut tokio::sync::broadcast::Receiver<DomainEvent>) -> ProcStatus {
        loop {
            match rx.recv().await {
                Ok(DomainEvent::ProcessStatusChanged { to, .. }) => return to,
                Ok(_) => continue,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("event bus closed"),
            }
        }
    }

    #[tokio::test]
    async fn start_then_stop_drives_full_lifecycle_via_mock_clock() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let registry = Registry::default();
        let clock = MockClock::new();
        let spawner = FakeSpawner::exits_on_kill();
        let cancel = CancellationToken::new();
        let id = ProcessId::next();
        seed(&registry, id, &cancel);

        spawn_supervised(
            id,
            SpawnSpec {
                program: "sleep".into(),
                args: vec!["60".into()],
            },
            Arc::new(spawner),
            Arc::new(clock.clone()),
            bus.clone(),
            registry.clone(),
            cancel.clone(),
        );

        assert_eq!(next_status(&mut rx).await, ProcStatus::Running);
        assert_eq!(registry.last_status(id), Some(ProcStatus::Running));

        // Request stop: the actor sends SIGTERM and waits out the grace window. The
        // fake child only exits on SIGKILL, so advancing the mock clock past the
        // grace window (no real time elapsed) drives the kill-and-reap path.
        cancel.cancel();
        assert_eq!(next_status(&mut rx).await, ProcStatus::Stopping);

        // Let the actor reach `clock.sleep`, then step the clock past the grace.
        tokio::task::yield_now().await;
        clock.advance(STOP_GRACE + Duration::from_secs(1));

        assert_eq!(next_status(&mut rx).await, ProcStatus::Stopped);
        assert_eq!(registry.last_status(id), Some(ProcStatus::Stopped));
    }

    #[tokio::test]
    async fn panicking_child_marks_crashed_without_killing_the_app() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let registry = Registry::default();
        let clock = MockClock::new();
        let cancel = CancellationToken::new();
        let id = ProcessId::next();
        seed(&registry, id, &cancel);

        spawn_supervised(
            id,
            SpawnSpec {
                program: "boom".into(),
                args: vec![],
            },
            Arc::new(FakeSpawner::panics_after_running()),
            Arc::new(clock),
            bus.clone(),
            registry.clone(),
            cancel,
        );

        // Running, then the panic is isolated and surfaced as Crashed.
        assert_eq!(next_status(&mut rx).await, ProcStatus::Running);
        assert_eq!(next_status(&mut rx).await, ProcStatus::Crashed);
        assert_eq!(registry.last_status(id), Some(ProcStatus::Crashed));

        // The supervisor is still alive: a second process runs to completion.
        let id2 = ProcessId::next();
        let cancel2 = CancellationToken::new();
        seed(&registry, id2, &cancel2);
        spawn_supervised(
            id2,
            SpawnSpec {
                program: "sleep".into(),
                args: vec![],
            },
            Arc::new(FakeSpawner::exits_on_kill()),
            Arc::new(MockClock::new()),
            bus.clone(),
            registry.clone(),
            cancel2.clone(),
        );
        assert_eq!(next_status(&mut rx).await, ProcStatus::Running);
        assert_eq!(registry.last_status(id2), Some(ProcStatus::Running));
    }
}
