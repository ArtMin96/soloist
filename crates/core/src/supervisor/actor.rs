//! The supervised process actor and its panic-isolation boundary.
//!
//! Each managed process is one supervised `tokio` task that solely owns its child
//! handle, control, and PTY I/O. It interacts with the rest of the core only by
//! publishing [`DomainEvent`]s, writing into its shared terminal buffers, and updating
//! its own registry entry — never by locking shared domain state. The actor loop spans
//! the *managed process*, not a single child: a restart kills the current child and
//! spawns a fresh one within the same task, while the process's terminal buffers carry
//! across so its output history survives the restart.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::events::{DomainEvent, EventBus};
use crate::ids::ProcessId;
use crate::ports::{
    Clock, ExitFuture, ExitStatus, LockReleaser, OrphanRecord, ProcessControl, ProcessSpawner,
    PtyIo, RuntimeState, SpawnSpec, Spawned,
};
use crate::process::ProcStatus;
use crate::terminal::{ActorTerminal, PtyInput, Recorder, TerminalSignal, Terminals};

use super::apply_transition;
use super::registry::Registry;

/// Grace window between SIGTERM and SIGKILL on a graceful stop. The *timing* is a core
/// policy (driven by the [`Clock`] port so it is testable without real time elapsing);
/// the *signalling* is the adapter's job.
const STOP_GRACE: Duration = Duration::from_secs(5);

/// How long the actor waits for the read loop's final, in-flight bytes after the child
/// has exited before giving up. Bounded so a forked grandchild that keeps the PTY slave
/// open (no EOF) cannot wedge the actor on shutdown.
const DRAIN_GRACE: Duration = Duration::from_millis(100);

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

/// The orphan-adoption identity of a process: the fields a runtime-state record is
/// keyed on so a leftover from a crash can be matched back to this command on relaunch.
pub(crate) struct OrphanIdentity {
    pub(crate) project_root: PathBuf,
    pub(crate) name: String,
}

/// The ports an actor needs, bundled to keep its signature readable.
pub(crate) struct ActorPorts {
    pub(crate) spawner: Arc<dyn ProcessSpawner>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) locks: Arc<dyn LockReleaser>,
    pub(crate) runtime: Arc<dyn RuntimeState>,
    pub(crate) bus: EventBus,
    pub(crate) registry: Registry,
    pub(crate) terminals: Terminals,
}

impl ActorPorts {
    fn clone_for_inner(&self) -> Self {
        Self {
            spawner: self.spawner.clone(),
            clock: self.clock.clone(),
            locks: self.locks.clone(),
            runtime: self.runtime.clone(),
            bus: self.bus.clone(),
            registry: self.registry.clone(),
            terminals: self.terminals.clone(),
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
    identity: OrphanIdentity,
    ports: ActorPorts,
    mailbox: mpsc::Receiver<ActorMsg>,
    initial: Option<Spawned>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let inner = tokio::spawn(run(
            id,
            launch,
            identity,
            ports.clone_for_inner(),
            mailbox,
            initial,
        ));
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

/// The actor body: open the process's terminal channel, then spawn a child, mark it
/// running, and race "child exited" against a mailbox message while streaming the
/// child's output into the terminal buffers and routing input to its PTY. On stop,
/// drive the graceful sequence and end; on restart, drive it and loop to respawn; on a
/// self-exit, classify it and end.
async fn run(
    id: ProcessId,
    launch: SpawnSpec,
    identity: OrphanIdentity,
    ports: ActorPorts,
    mut mailbox: mpsc::Receiver<ActorMsg>,
    mut initial: Option<Spawned>,
) {
    let ActorPorts {
        spawner,
        clock,
        locks,
        runtime,
        bus,
        registry,
        terminals,
    } = ports;
    let ActorTerminal {
        mut input,
        recorder,
    } = terminals.open(id);
    // The supervisor has already moved this process into `Starting`.
    let mut status = ProcStatus::Starting;

    loop {
        // The first iteration of an adopted process uses the pre-built handle over its
        // existing process group; every spawn (and every restart) creates a fresh child.
        let spawned = match initial.take() {
            Some(spawned) => spawned,
            None => match spawner.spawn(&launch).await {
                Ok(spawned) => spawned,
                Err(_err) => {
                    advance(&registry, &bus, id, &mut status, ProcStatus::Crashed, None);
                    locks.release_all(id);
                    return;
                }
            },
        };
        let Spawned {
            pid,
            mut output,
            mut exit,
            control,
            io,
        } = spawned;
        let pgid = pid.map(|raw| raw as i32);
        // Record the running group before announcing Running, so a crash immediately
        // after the announcement still leaves a reconcilable runtime-state record.
        record_orphan(&runtime, &identity, &launch, pgid).await;
        advance(&registry, &bus, id, &mut status, ProcStatus::Running, None);

        // Once the child closes its output the branch is disabled (its `recv` would
        // otherwise return `None` forever and busy-spin the select).
        let mut output_open = true;
        let outcome = loop {
            tokio::select! {
                finished = &mut exit => break Outcome::Exited(finished),
                message = mailbox.recv() => break match message {
                    Some(ActorMsg::Restart) => Outcome::Restart,
                    // A dropped mailbox (sender gone, e.g. shutdown) means stop.
                    Some(ActorMsg::Stop) | None => Outcome::Stop,
                },
                chunk = output.recv(), if output_open => match chunk {
                    Some(chunk) => publish_output(&recorder, id, &bus, chunk),
                    None => output_open = false,
                },
                message = input.recv() => {
                    if let Some(message) = message {
                        apply_input(io.as_ref(), message).await;
                    }
                }
            }
        };

        match outcome {
            Outcome::Exited(exit_status) => {
                drain_output(&mut output, &recorder, id, &bus, clock.as_ref()).await;
                forget_orphan(&runtime, pgid).await;
                let (to, code) = classify_exit(exit_status);
                advance(&registry, &bus, id, &mut status, to, code);
                locks.release_all(id);
                return;
            }
            Outcome::Stop => {
                advance(&registry, &bus, id, &mut status, ProcStatus::Stopping, None);
                graceful_stop(control, exit, clock.as_ref()).await;
                drain_output(&mut output, &recorder, id, &bus, clock.as_ref()).await;
                forget_orphan(&runtime, pgid).await;
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
                drain_output(&mut output, &recorder, id, &bus, clock.as_ref()).await;
                forget_orphan(&runtime, pgid).await;
                advance(&registry, &bus, id, &mut status, ProcStatus::Starting, None);
                // Loop to respawn a fresh child under the same actor (and buffers).
            }
        }
    }
}

/// Records the running process group in the runtime-state file so a leftover from a
/// crash or force-quit can be reconciled on the next launch. Best-effort: a failed
/// write must not take down the actor.
async fn record_orphan(
    runtime: &Arc<dyn RuntimeState>,
    identity: &OrphanIdentity,
    launch: &SpawnSpec,
    pgid: Option<i32>,
) {
    if let Some(pgid) = pgid {
        let runtime = runtime.clone();
        let record = OrphanRecord {
            project_root: identity.project_root.clone(),
            name: identity.name.clone(),
            command: launch.command.clone(),
            pgid,
        };
        // The runtime-state write touches the filesystem; run it off the async runtime
        // so a slow disk never stalls the supervisor's worker thread.
        let _ = tokio::task::spawn_blocking(move || runtime.record(&record)).await;
    }
}

/// Drops the runtime-state record for a reaped process group, off the async runtime.
async fn forget_orphan(runtime: &Arc<dyn RuntimeState>, pgid: Option<i32>) {
    if let Some(pgid) = pgid {
        let runtime = runtime.clone();
        let _ = tokio::task::spawn_blocking(move || runtime.forget(pgid)).await;
    }
}

/// Records a chunk of PTY output into the terminal buffers and publishes any title or
/// bell signals it carried.
fn publish_output(recorder: &Recorder, id: ProcessId, bus: &EventBus, chunk: Vec<u8>) {
    for signal in recorder.record(chunk) {
        let event = match signal {
            TerminalSignal::Title(title) => DomainEvent::TerminalTitleChanged { id, title },
            TerminalSignal::Bell => DomainEvent::TerminalBell { id },
        };
        bus.publish(event);
    }
}

/// Drains the read loop's remaining output as the actor winds down so a process's final
/// bytes (e.g. a crash message) are not lost. Prefers buffered chunks and waits for the
/// in-flight tail until the channel closes (EOF — every byte captured), but is bounded
/// by [`DRAIN_GRACE`] so a forked grandchild holding the PTY slave open cannot wedge it.
async fn drain_output(
    output: &mut mpsc::Receiver<Vec<u8>>,
    recorder: &Recorder,
    id: ProcessId,
    bus: &EventBus,
    clock: &dyn Clock,
) {
    let deadline = clock.sleep(DRAIN_GRACE);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            biased;
            chunk = output.recv() => match chunk {
                Some(chunk) => publish_output(recorder, id, bus, chunk),
                None => break,
            },
            _ = &mut deadline => break,
        }
    }
}

/// Applies one input message to the child's PTY. Best-effort: a write to an
/// already-exiting child fails harmlessly and is dropped.
async fn apply_input(io: &dyn PtyIo, message: PtyInput) {
    let _ = match message {
        PtyInput::Write(data) => io.write(&data).await,
        PtyInput::Resize(size) => io.resize(size).await,
    };
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
