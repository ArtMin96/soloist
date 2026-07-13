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
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::events::{DomainEvent, EventBus};
use crate::identity::PROCESS_ID_ENV;
use crate::ids::ProcessId;
use crate::ports::{
    Clock, ExitFuture, ExitStatus, LockReleaser, OrphanControl, OrphanRecord, ProcessControl,
    ProcessSpawner, PtyIo, PtySize, RuntimeState, SpawnSpec, Spawned,
};
use crate::process::ProcStatus;
use crate::shellenv::ShellEnv;
use crate::sync::lock;
use crate::terminal::{ActorTerminal, PtyInput, Recorder, TerminalSignal};

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
    /// Signals a leftover process group by pgid — used only on the panic path, to reap a child
    /// the panicked inner task left behind.
    pub(crate) orphan_control: Arc<dyn OrphanControl>,
    pub(crate) bus: EventBus,
    pub(crate) registry: Registry,
    pub(crate) shell_env: Arc<ShellEnv>,
}

impl ActorPorts {
    fn clone_for_inner(&self) -> Self {
        Self {
            spawner: self.spawner.clone(),
            clock: self.clock.clone(),
            locks: self.locks.clone(),
            runtime: self.runtime.clone(),
            orphan_control: self.orphan_control.clone(),
            bus: self.bus.clone(),
            registry: self.registry.clone(),
            shell_env: self.shell_env.clone(),
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
    terminal: ActorTerminal,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let inner = tokio::spawn(run(
            id,
            launch,
            identity,
            ports.clone_for_inner(),
            mailbox,
            initial,
            terminal,
        ));
        if let Err(join_err) = inner.await {
            if join_err.is_panic() {
                // A panic is an out-of-band fault. The inner task may have spawned a child and
                // panicked before reaping it, so reap its recorded group and clear the pgid
                // first: otherwise a crash auto-restart would spawn a second child beside the
                // still-running first (port conflicts, double workers), and the orphan record
                // would linger. Best-effort — a failed signal or forget must not mask the crash.
                if let Some(pgid) = ports.registry.pgid_of(id) {
                    let _ = ports.orphan_control.signal(pgid, true);
                    ports.registry.set_pgid(id, None);
                    forget_orphan(&ports.runtime, Some(pgid)).await;
                }
                // Force `Crashed` directly rather than through the FSM, since the unit must end
                // up crashed even from a state the normal transition rules would reject.
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
    mut launch: SpawnSpec,
    identity: OrphanIdentity,
    ports: ActorPorts,
    mut mailbox: mpsc::Receiver<ActorMsg>,
    mut initial: Option<Spawned>,
    terminal: ActorTerminal,
) {
    // Every managed process carries its own id in the environment, so an agent it runs
    // can bind its MCP session to the process it lives in. Set once here so every spawn
    // (including each restart) of this process inherits it.
    launch
        .env
        .insert(PROCESS_ID_ENV.to_string(), id.get().to_string());
    let ActorPorts {
        spawner,
        clock,
        locks,
        runtime,
        // Only the panic path (in `spawn`) reaps via the orphan control; the running actor
        // reaps through its own `control` handle.
        orphan_control: _,
        bus,
        registry,
        shell_env,
    } = ports;
    // The terminal channel is opened synchronously in the launch path (before this actor is
    // spawned), so a viewer that attaches in the scheduling window finds a live channel rather
    // than "process has not started"; the actor receives the actor-facing half here.
    let ActorTerminal { input, recorder } = terminal;
    // Input is applied off the select loop by a dedicated pump: a blocking write to a
    // child that has stopped reading its stdin then stalls only the pump (typed input
    // backpressures on the bounded channel), never this actor — so stop, restart, exit,
    // and output draining stay responsive. The pump is torn down with the actor.
    let current_io: CurrentIo = Arc::new(Mutex::new(None));
    // The last winsize a viewer requested, remembered so every (re)spawn re-creates the PTY at
    // that size instead of the 80×24 default — otherwise a relaunched TUI stays mis-sized until
    // the next resize. Shared with the pump, which records each resize even when no child is live
    // (the pre-/post-`Running` window), so a resize in that window is not lost.
    let last_size = Arc::new(Mutex::new(launch.size));
    let _pump = PumpGuard(spawn_input_pump(
        input,
        current_io.clone(),
        last_size.clone(),
    ));
    // The supervisor has already moved this process into `Starting`.
    let mut status = ProcStatus::Starting;

    loop {
        // If this process already has output from a previous run — an in-place restart
        // looping here, or a fresh actor reusing the buffers after a crash auto-restart —
        // mark the boundary so the kept output is divided from the new run's. A no-op on
        // the first run, when there is nothing to separate.
        recorder.mark_restart();
        // The first iteration of an adopted process uses the pre-built handle over its
        // existing process group; every spawn (and every restart) creates a fresh child.
        let spawned = match initial.take() {
            Some(spawned) => spawned,
            None => {
                // Resolve the launch environment at spawn time: the captured login-shell
                // environment (version-manager PATHs included) under this process's own
                // overrides. Done per spawn so a restart picks up a refreshed capture, and
                // kept off `launch` so the canonical overrides are re-resolved each time.
                let spec = SpawnSpec {
                    env: shell_env.resolve(&launch.env).await,
                    command: launch.command.clone(),
                    working_dir: launch.working_dir.clone(),
                    // Re-create the PTY at the last size a viewer requested, so a relaunch keeps
                    // the pane's dimensions rather than resetting to the 80×24 default.
                    size: *lock(&last_size),
                };
                match spawner.spawn(&spec).await {
                    Ok(spawned) => spawned,
                    Err(err) => {
                        // Surface *why* the spawn failed (missing binary, bad working dir, …) in
                        // the process's own terminal, so the crash is diagnosable instead of an
                        // empty pane. Then crash, as before.
                        recorder
                            .record(format!("soloist: failed to start: {err}\r\n").into_bytes());
                        advance(&registry, &bus, id, &mut status, ProcStatus::Crashed, None);
                        locks.release_all(id);
                        return;
                    }
                }
            }
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
        // after the announcement still leaves a reconcilable runtime-state record. The
        // same pgid is recorded in the registry so monitoring can sample the live group.
        record_orphan(&runtime, &identity, &launch, pgid).await;
        registry.set_pgid(id, pgid);
        // Hand the live child's I/O to the input pump *before* announcing `Running`, so a resize
        // that arrives as the child comes up is applied to it rather than dropped. Cleared below
        // when the child stops.
        *lock(&current_io) = Some(Arc::from(io));
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
            }
        };
        // The current child is exiting or about to be stopped — drop it as a sampling
        // target and stop routing input to it. A restart re-records the fresh group's
        // pgid and I/O on the next iteration.
        registry.set_pgid(id, None);
        *lock(&current_io) = None;

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

/// The shared slot holding the currently-live child's I/O handle. The actor swaps in the
/// new handle on each (re)spawn and clears it when the child stops; the input pump reads
/// it per message. A plain mutex the pump never holds across an `.await`.
type CurrentIo = Arc<Mutex<Option<Arc<dyn PtyIo>>>>;

/// Aborts the input pump when the actor's `run` ends, so a pump blocked on a stuck write
/// to a child that stopped reading its stdin is torn down with the actor, not leaked.
struct PumpGuard(JoinHandle<()>);

impl Drop for PumpGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Spawns the per-actor input pump: it owns the process's input receiver and applies each
/// message to whichever child is currently live, off the actor's select loop. A blocking
/// PTY write therefore stalls only the pump — never stop, restart, exit, or output
/// draining. A resize updates `last_size` first, so the size is remembered for the next
/// (re)spawn even when it has no live child to apply to right now; the write or resize is
/// then applied to the current child if there is one, and otherwise dropped.
fn spawn_input_pump(
    mut input: mpsc::Receiver<PtyInput>,
    current_io: CurrentIo,
    last_size: Arc<Mutex<PtySize>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(message) = input.recv().await {
            if let PtyInput::Resize(size) = &message {
                *lock(&last_size) = *size;
            }
            let io = lock(&current_io).clone();
            if let Some(io) = io {
                apply_input(io.as_ref(), message).await;
            }
        }
    })
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
