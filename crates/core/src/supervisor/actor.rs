//! The supervised process actor and its panic-isolation boundary.
//!
//! Each managed process is one supervised `tokio` task that solely owns its child
//! handle and control. It interacts with the rest of the core only by publishing
//! [`DomainEvent`]s and updating its own registry entry, never by locking shared
//! domain state. The actor loop spans the *managed process*, not a single child: a
//! restart kills the current child and spawns a fresh one within the same task.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::events::{DomainEvent, EventBus};
use crate::ids::ProcessId;
use crate::ports::{
    Clock, ExitFuture, ExitStatus, LockReleaser, ProcessControl, ProcessSpawner, SpawnSpec, Spawned,
};
use crate::process::ProcStatus;

use super::apply_transition;
use super::registry::Registry;

/// Grace window between SIGTERM and SIGKILL on a graceful stop. The *timing* is a core
/// policy (driven by the [`Clock`] port so it is testable without real time elapsing);
/// the *signalling* is the adapter's job.
const STOP_GRACE: Duration = Duration::from_secs(5);

/// A message to a running actor.
pub(crate) enum ActorMsg {
    /// Stop the process: graceful SIGTERM → grace → SIGKILL, then the actor ends.
    Stop,
    /// Restart the process: stop the current child, then spawn a fresh one.
    Restart,
}

/// What ended the current child's run.
enum Outcome {
    Exited(ExitStatus),
    Stop,
    Restart,
}

/// The ports an actor needs, bundled to keep its signature readable.
pub(crate) struct ActorPorts {
    pub(crate) spawner: Arc<dyn ProcessSpawner>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) locks: Arc<dyn LockReleaser>,
    pub(crate) bus: EventBus,
    pub(crate) registry: Registry,
}

impl ActorPorts {
    fn clone_for_inner(&self) -> Self {
        Self {
            spawner: self.spawner.clone(),
            clock: self.clock.clone(),
            locks: self.locks.clone(),
            bus: self.bus.clone(),
            registry: self.registry.clone(),
        }
    }
}

/// Spawns the supervised actor inside a panic-isolation boundary and returns the outer
/// task handle. A panic inside the actor marks just that process
/// [`ProcStatus::Crashed`], releases its locks, and is otherwise contained — the
/// supervisor and every other process stay alive.
pub(crate) fn spawn(
    id: ProcessId,
    launch: SpawnSpec,
    ports: ActorPorts,
    mailbox: mpsc::Receiver<ActorMsg>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let inner = tokio::spawn(run(id, launch, ports.clone_for_inner(), mailbox));
        if let Err(join_err) = inner.await {
            if join_err.is_panic() {
                // A panic is an out-of-band fault: force `Crashed` directly rather
                // than through the FSM, since the unit must end up crashed even from a
                // state the normal transition rules would reject.
                let from = ports.registry.status(id).unwrap_or(ProcStatus::Running);
                ports.registry.set_status(id, ProcStatus::Crashed, None);
                ports.bus.publish(DomainEvent::ProcessStatusChanged {
                    id,
                    from,
                    to: ProcStatus::Crashed,
                    exit_code: None,
                });
                ports.locks.release_all(id);
            }
        }
    })
}

/// The actor body: spawn a child, mark it running, then race "child exited" against a
/// mailbox message. On stop, drive the graceful sequence and end; on restart, drive it
/// and loop to respawn; on a self-exit, classify it and end.
async fn run(
    id: ProcessId,
    launch: SpawnSpec,
    ports: ActorPorts,
    mut mailbox: mpsc::Receiver<ActorMsg>,
) {
    let ActorPorts {
        spawner,
        clock,
        locks,
        bus,
        registry,
    } = ports;
    // The supervisor has already moved this process into `Starting`.
    let mut status = ProcStatus::Starting;

    loop {
        let spawned = match spawner.spawn(&launch).await {
            Ok(spawned) => spawned,
            Err(_err) => {
                advance(&registry, &bus, id, &mut status, ProcStatus::Crashed, None);
                locks.release_all(id);
                return;
            }
        };
        let Spawned {
            pid: _,
            mut exit,
            control,
        } = spawned;
        advance(&registry, &bus, id, &mut status, ProcStatus::Running, None);

        let outcome = tokio::select! {
            finished = &mut exit => Outcome::Exited(finished),
            message = mailbox.recv() => match message {
                Some(ActorMsg::Restart) => Outcome::Restart,
                // A dropped mailbox (sender gone, e.g. shutdown) means stop.
                Some(ActorMsg::Stop) | None => Outcome::Stop,
            },
        };

        match outcome {
            Outcome::Exited(exit_status) => {
                let (to, code) = classify_exit(exit_status);
                advance(&registry, &bus, id, &mut status, to, code);
                locks.release_all(id);
                return;
            }
            Outcome::Stop => {
                advance(&registry, &bus, id, &mut status, ProcStatus::Stopping, None);
                graceful_stop(control, exit, clock.as_ref()).await;
                advance(&registry, &bus, id, &mut status, ProcStatus::Stopped, None);
                locks.release_all(id);
                return;
            }
            Outcome::Restart => {
                advance(
                    &registry,
                    &bus,
                    id,
                    &mut status,
                    ProcStatus::Restarting,
                    None,
                );
                graceful_stop(control, exit, clock.as_ref()).await;
                advance(&registry, &bus, id, &mut status, ProcStatus::Starting, None);
                // Loop to respawn a fresh child under the same actor.
            }
        }
    }
}

/// Applies one FSM transition, threading the actor's local status mirror (the source
/// of `from`) and publishing the delta.
fn advance(
    registry: &Registry,
    bus: &EventBus,
    id: ProcessId,
    status: &mut ProcStatus,
    to: ProcStatus,
    exit_code: Option<i32>,
) {
    *status = apply_transition(registry, bus, id, *status, to, exit_code);
}

/// Graceful stop: SIGTERM to the group, wait out the grace window, escalate to SIGKILL
/// only on timeout, and reap in every case. Consumes the control + exit handles.
async fn graceful_stop(
    mut control: Box<dyn ProcessControl>,
    mut exit: ExitFuture,
    clock: &dyn Clock,
) {
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
}

/// Classifies a self-exit: a clean `exit(0)` is a `Stopped` process; anything else (a
/// non-zero code, or termination by a signal we did not send) is a crash. The exit
/// code is carried through; a signal kill has no code.
fn classify_exit(status: ExitStatus) -> (ProcStatus, Option<i32>) {
    if status.success() {
        (ProcStatus::Stopped, status.code)
    } else {
        (ProcStatus::Crashed, status.code)
    }
}
