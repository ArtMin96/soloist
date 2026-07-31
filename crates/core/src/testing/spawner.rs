//! A [`ProcessSpawner`] fake whose children are entirely in-memory: no OS process and
//! no real PTY. Its behaviour is chosen per constructor so a test can drive a specific
//! actor path — the grace window, panic isolation, a clean or signalled exit, or an
//! output stream into the terminal buffers.

use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot, Notify};

use crate::ids::{ProcessId, PROCESS_ID_ENV};
use crate::ports::{
    ExitFuture, ExitStatus, ProcessControl, ProcessSpawner, PtyIo, PtySize, SpawnError, SpawnSpec,
    Spawned,
};
use crate::sync::lock;

/// A shared buffer of the environment of each spawn, recorded by
/// [`FakeSpawner::records_spec_env`] so a test can read back what reached a process.
type SpecEnvLog = Arc<Mutex<Vec<BTreeMap<String, String>>>>;

/// A shared buffer of the command line of each spawn, recorded by
/// [`FakeSpawner::records_command`] so a test can read back which command line launched a
/// process — e.g. the fresh launch versus the resume command line.
pub type CommandLog = Arc<Mutex<Vec<String>>>;

/// The shared record a [`FakeSpawner::records_input`] child fills: every byte written to a
/// child's PTY, kept under the process that received it. Attribution comes from the
/// [`PROCESS_ID_ENV`] the supervisor injects into each launch, so a test can prove not merely
/// that a write happened but *which* process it reached — the difference between a report
/// delivered to its lead and one the reporter sent to itself.
#[derive(Clone, Default)]
pub struct InputLog {
    per_process: Arc<Mutex<BTreeMap<ProcessId, Vec<u8>>>>,
}

impl InputLog {
    /// Every byte written to `process`'s PTY, in order. Empty for a process that received none.
    pub fn to(&self, process: ProcessId) -> Vec<u8> {
        lock(&self.per_process)
            .get(&process)
            .cloned()
            .unwrap_or_default()
    }

    /// What every process received, keyed by process — for a test that must find where a write
    /// landed rather than assert about one it already names.
    pub fn by_process(&self) -> BTreeMap<ProcessId, Vec<u8>> {
        lock(&self.per_process).clone()
    }
}

/// The shared record a [`FakeSpawner::records_resizes`] child fills: the winsize each spawn
/// created its PTY at (`spawns`, in launch order) and every resize applied to a live PTY
/// (`resizes`, in order). A test reads these to prove a resize reaches the child and that a
/// respawn re-creates the PTY at the last requested size instead of the 80×24 default.
#[derive(Clone, Default)]
pub struct ResizeLog {
    spawns: Arc<Mutex<Vec<PtySize>>>,
    resizes: Arc<Mutex<Vec<PtySize>>>,
    applied: Arc<Notify>,
}

impl ResizeLog {
    /// The winsize each spawn created its PTY at, in launch order.
    pub fn spawns(&self) -> Vec<PtySize> {
        lock(&self.spawns).clone()
    }

    /// Every resize applied to a live PTY, in order.
    pub fn resizes(&self) -> Vec<PtySize> {
        lock(&self.resizes).clone()
    }

    /// Resolves once a resize has been applied to the live PTY, so a test can wait for the input
    /// pump to have processed a resize — and thus recorded the last size — without polling.
    pub async fn resize_applied(&self) {
        self.applied.notified().await;
    }
}

/// Ends a [`FakeSpawner::exits_when_told`] child's run on cue, so a test can drive a process
/// finishing **by itself** — which a caller's stop is not, and which is the only ending an
/// auto-close acts on. Keyed by process, so ending one run and not another keeps the order of the
/// events a test then observes deterministic.
#[derive(Clone, Default)]
pub struct ExitTrigger {
    fired: Arc<Mutex<HashSet<ProcessId>>>,
    changed: Arc<Notify>,
}

impl ExitTrigger {
    /// Ends `process`'s current run: its child exits cleanly on its own, as a finished worker
    /// does. Spent by the child that takes it, so a process started again afterwards runs on
    /// until it is fired again — a restart is a new run, not a replay of the last ending.
    pub fn fire(&self, process: ProcessId) {
        lock(&self.fired).insert(process);
        self.changed.notify_waiters();
    }

    /// Resolves once `process` has been fired, taking that ending so no later run inherits it.
    async fn awaited(&self, process: ProcessId) {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            // Registered before the check, so a fire landing between the two still wakes this.
            changed.as_mut().enable();
            if lock(&self.fired).remove(&process) {
                return;
            }
            changed.await;
        }
    }
}

/// Signal numbers a simulated kill records on a fake child's exit status.
const SIGKILL: i32 = 9;
const SIGTERM: i32 = 15;

/// The pid an [`FakeSpawner::exits_when_told`] child reports, offset by the process it belongs to
/// so no two of them share a group — a shared group makes the home-process lookup ambiguous, and
/// these children are launched several at a time.
const CUED_EXIT_PID_BASE: u32 = 430_000;

/// The pid — and therefore process group — of [`FakeSpawner::panics_after_running`]'s child.
/// The panic-isolation test asserts this exact group is SIGKILLed when the actor reaps the child
/// the panicked task left behind, so the two sites share one binding.
pub(crate) const PANIC_FAKE_PGID: i32 = 9191;

/// The exit status of a fake child terminated by `signal`.
fn killed_by(signal: i32) -> ExitStatus {
    ExitStatus {
        code: None,
        signal: Some(signal),
    }
}

/// Which signal makes a long-lived fake child finally exit.
#[derive(Clone, Copy)]
enum DiesOn {
    Terminate,
    Kill,
}

enum Behavior {
    /// Runs until signalled; obeys SIGTERM or only SIGKILL per [`DiesOn`].
    LongLived(DiesOn),
    /// Panics the moment its exit future is polled after reaching `Running`.
    PanicsAfterRunning,
    /// Exits on its own immediately with a fixed status.
    ExitsImmediately(ExitStatus),
    /// Emits the given output chunks, then exits with `exit` — drives the actor's PTY
    /// output drain into the terminal buffers without a real process. A clean `exit`
    /// stops the process; a non-zero one crashes it (so its output is the "last crash
    /// output" a relaunch retains).
    Streams {
        chunks: Vec<Vec<u8>>,
        exit: ExitStatus,
    },
    /// Emits the given output chunks, then stays alive until killed — a process that
    /// produced output and remains running, for exercising the idle classifier (output is
    /// in the buffers while the process is still `Running`).
    StreamsThenStaysAlive { chunks: Vec<Vec<u8>> },
    /// Stays alive until killed and records every byte written to its PTY under the process it
    /// belongs to — so a test can prove what reached a *given* process's input (e.g. a timer
    /// delivering its body as a fresh turn, or a report reaching its lead and not its author).
    RecordsInput(InputLog),
    /// Stays alive until killed and records the environment of each spawn into a shared
    /// buffer — so a test can prove what env reached a process (e.g. the captured shell
    /// environment merged with the per-process overrides).
    RecordsSpecEnv(SpecEnvLog),
    /// Stays alive until killed and records the command line of each spawn into a shared
    /// buffer — so a test can prove which command line launched a process (e.g. a resume
    /// replays the resume command while a fresh start uses the original).
    RecordsCommand(CommandLog),
    /// Stays alive (exiting on SIGTERM) but blocks forever on every stdin write — a child
    /// that has stopped reading its input, so a test can prove a stuck write never wedges
    /// the owning actor. The [`Notify`] fires as the write begins to block, so the test can
    /// wait for that deterministically before it checks the actor is still responsive.
    BlocksOnInput(Arc<Notify>),
    /// Fails to spawn with a fixed message — a missing binary or bad working dir — so a test
    /// can prove the actor surfaces the reason in the terminal and crashes.
    FailsToSpawn(String),
    /// Stays alive (exiting on SIGTERM) and records, per spawn, the winsize its PTY was created
    /// at and every resize applied to it — so a test can prove a resize reaches the child and
    /// that a respawn re-creates the PTY at the last requested size.
    RecordsResizes(ResizeLog),
    /// Stays alive until the test ends its run through the shared [`ExitTrigger`], then exits
    /// cleanly **on its own** — the self-ended run a stop is not. Obeys SIGTERM too, so one
    /// spawner drives both endings.
    ExitsWhenTold(ExitTrigger),
}

/// A [`ProcessSpawner`] that returns fully in-memory children. Its behaviour is chosen
/// per constructor so tests can drive specific actor paths.
pub struct FakeSpawner {
    behavior: Behavior,
}

impl FakeSpawner {
    /// A child that ignores SIGTERM and exits only on SIGKILL — forces the grace path.
    pub fn exits_on_kill() -> Self {
        Self {
            behavior: Behavior::LongLived(DiesOn::Kill),
        }
    }

    /// A child that exits promptly on SIGTERM — the fast graceful-stop path.
    pub fn exits_on_terminate() -> Self {
        Self {
            behavior: Behavior::LongLived(DiesOn::Terminate),
        }
    }

    /// A child that panics once running — drives the panic-isolation boundary.
    pub fn panics_after_running() -> Self {
        Self {
            behavior: Behavior::PanicsAfterRunning,
        }
    }

    /// A child that exits on its own with the given code (no terminating signal).
    pub fn exits_with_code(code: i32) -> Self {
        Self {
            behavior: Behavior::ExitsImmediately(ExitStatus {
                code: Some(code),
                signal: None,
            }),
        }
    }

    /// A child that is terminated on its own by an external `signal`.
    pub fn killed_by_signal(signal: i32) -> Self {
        Self {
            behavior: Behavior::ExitsImmediately(killed_by(signal)),
        }
    }

    /// A child that emits `chunks` on its PTY, then exits cleanly. Used to prove the
    /// actor drains output into the per-process terminal buffers.
    pub fn streams_then_exits(chunks: Vec<Vec<u8>>) -> Self {
        Self {
            behavior: Behavior::Streams {
                chunks,
                exit: ExitStatus {
                    code: Some(0),
                    signal: None,
                },
            },
        }
    }

    /// A child that emits `chunks` on its PTY, then crashes with `code`. Used to prove a
    /// relaunch retains the previous run's output (the "last crash output") and marks a
    /// restart boundary before the new run's.
    pub fn streams_then_crashes(chunks: Vec<Vec<u8>>, code: i32) -> Self {
        Self {
            behavior: Behavior::Streams {
                chunks,
                exit: ExitStatus {
                    code: Some(code),
                    signal: None,
                },
            },
        }
    }

    /// A child that emits `chunks` on its PTY, then stays running until killed — output is
    /// captured in the terminal buffers while the process remains `Running`, for exercising
    /// the agent idle classifier.
    pub fn streams_then_stays_alive(chunks: Vec<Vec<u8>>) -> Self {
        Self {
            behavior: Behavior::StreamsThenStaysAlive { chunks },
        }
    }

    /// A long-lived child that records every byte written to its PTY under the process that
    /// received it, returning the spawner and the shared [`InputLog`] the test reads. Used to
    /// prove what reached a given process's input — e.g. that a fired timer delivered its body,
    /// followed by a carriage return, as a fresh turn to the process that set it.
    pub fn records_input() -> (Self, InputLog) {
        let log = InputLog::default();
        (
            Self {
                behavior: Behavior::RecordsInput(log.clone()),
            },
            log,
        )
    }

    /// A long-lived child that records the environment of each spawn, returning the spawner
    /// and the shared buffer the test reads. Used to prove which variables reached a
    /// process — e.g. that the captured shell environment was merged with the process's own
    /// `env` and the injected process id.
    pub fn records_spec_env() -> (Self, SpecEnvLog) {
        let recorder = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                behavior: Behavior::RecordsSpecEnv(recorder.clone()),
            },
            recorder,
        )
    }

    /// A long-lived child that records the command line of each spawn, returning the spawner
    /// and the shared buffer the test reads (one entry per launch, in order). Used to prove a
    /// resume replays the resume command while a fresh start uses the original.
    pub fn records_command() -> (Self, CommandLog) {
        let recorder = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                behavior: Behavior::RecordsCommand(recorder.clone()),
            },
            recorder,
        )
    }

    /// A child that stays alive (exiting on SIGTERM) but blocks forever on every stdin
    /// write, modelling a process that has stopped reading its input. Returns the spawner
    /// and a [`Notify`] that fires when a write begins to block, so a test can wait for the
    /// wedge deterministically before proving the owning actor is still responsive.
    pub fn blocks_on_input() -> (Self, Arc<Notify>) {
        let entered = Arc::new(Notify::new());
        (
            Self {
                behavior: Behavior::BlocksOnInput(entered.clone()),
            },
            entered,
        )
    }

    /// A spawner whose spawn always fails with `message` — a missing binary or bad working
    /// dir — so a test can prove the actor writes the reason into the terminal and crashes.
    pub fn fails_to_spawn(message: &str) -> Self {
        Self {
            behavior: Behavior::FailsToSpawn(message.to_string()),
        }
    }

    /// A long-lived child (exiting on SIGTERM) that records the winsize each spawn created its
    /// PTY at and every resize applied to it, returning the spawner and the shared [`ResizeLog`]
    /// the test reads. Used to prove a resize reaches the child and that a respawn re-creates the
    /// PTY at the last requested size rather than the 80×24 default.
    pub fn records_resizes() -> (Self, ResizeLog) {
        let log = ResizeLog::default();
        (
            Self {
                behavior: Behavior::RecordsResizes(log.clone()),
            },
            log,
        )
    }

    /// Children that run until the test ends them through the returned [`ExitTrigger`], then exit
    /// cleanly on their own. The one fake that separates a run a process finished itself from one
    /// a caller stopped, which the auto-close policy treats as different endings.
    pub fn exits_when_told() -> (Self, ExitTrigger) {
        let trigger = ExitTrigger::default();
        (
            Self {
                behavior: Behavior::ExitsWhenTold(trigger.clone()),
            },
            trigger,
        )
    }
}

/// A closed PTY output channel: the receiver yields nothing and reports EOF at once.
/// Most fake children produce no output; the streaming behaviour overrides this.
fn no_output() -> mpsc::Receiver<Vec<u8>> {
    let (_tx, rx) = mpsc::channel(1);
    rx
}

#[async_trait]
impl ProcessSpawner for FakeSpawner {
    async fn spawn(&self, spec: &SpawnSpec) -> Result<Spawned, SpawnError> {
        match &self.behavior {
            Behavior::LongLived(dies_on) => {
                let (exit_tx, exit_rx) = oneshot::channel::<ExitStatus>();
                let control = Box::new(OneshotControl {
                    exit_tx: Mutex::new(Some(exit_tx)),
                    dies_on: *dies_on,
                });
                let exit: ExitFuture =
                    Box::pin(async move { exit_rx.await.unwrap_or_else(|_| killed_by(SIGKILL)) });
                Ok(Spawned {
                    pid: Some(424242),
                    output: no_output(),
                    exit,
                    control,
                    io: Box::new(NoopPtyIo),
                })
            }
            Behavior::PanicsAfterRunning => {
                // The fake panics by design to drive the actor's panic-isolation boundary.
                #[allow(clippy::panic)]
                let exit: ExitFuture = Box::pin(async { panic!("fake child panicked") });
                Ok(Spawned {
                    // A realistic live pgid, so a test can prove the panic path reaps the child
                    // the panicked inner task left behind.
                    pid: Some(PANIC_FAKE_PGID as u32),
                    output: no_output(),
                    exit,
                    control: Box::new(NoopControl),
                    io: Box::new(NoopPtyIo),
                })
            }
            Behavior::ExitsImmediately(status) => {
                let status = *status;
                let exit: ExitFuture = Box::pin(async move { status });
                Ok(Spawned {
                    pid: Some(1),
                    output: no_output(),
                    exit,
                    control: Box::new(NoopControl),
                    io: Box::new(NoopPtyIo),
                })
            }
            Behavior::Streams { chunks, exit } => {
                let (tx, output) = mpsc::channel(chunks.len().max(1));
                for chunk in chunks {
                    let _ = tx.try_send(chunk.clone());
                }
                drop(tx);
                let status = *exit;
                let exit: ExitFuture = Box::pin(async move { status });
                Ok(Spawned {
                    pid: Some(7),
                    output,
                    exit,
                    control: Box::new(NoopControl),
                    io: Box::new(NoopPtyIo),
                })
            }
            Behavior::StreamsThenStaysAlive { chunks } => {
                let (tx, output) = mpsc::channel(chunks.len().max(1));
                for chunk in chunks {
                    let _ = tx.try_send(chunk.clone());
                }
                // Close the output stream (EOF) but leave the child running: it exits only
                // when killed, like a long-lived process that has gone quiet.
                drop(tx);
                let (exit_tx, exit_rx) = oneshot::channel::<ExitStatus>();
                let control = Box::new(OneshotControl {
                    exit_tx: Mutex::new(Some(exit_tx)),
                    dies_on: DiesOn::Kill,
                });
                let exit: ExitFuture =
                    Box::pin(async move { exit_rx.await.unwrap_or_else(|_| killed_by(SIGKILL)) });
                Ok(Spawned {
                    pid: Some(424243),
                    output,
                    exit,
                    control,
                    io: Box::new(NoopPtyIo),
                })
            }
            Behavior::RecordsInput(log) => {
                let (exit_tx, exit_rx) = oneshot::channel::<ExitStatus>();
                let control = Box::new(OneshotControl {
                    exit_tx: Mutex::new(Some(exit_tx)),
                    dies_on: DiesOn::Kill,
                });
                let exit: ExitFuture =
                    Box::pin(async move { exit_rx.await.unwrap_or_else(|_| killed_by(SIGKILL)) });
                // A launch that carries no process id cannot be attributed, so its child discards
                // its input rather than pooling it under someone else's: a test then reads nothing
                // and reddens, instead of reading a write that was never meant for the process it
                // asked about.
                let io: Box<dyn PtyIo> = match spawned_process(spec) {
                    Some(process) => Box::new(RecordingPtyIo {
                        process,
                        log: log.clone(),
                    }),
                    None => Box::new(NoopPtyIo),
                };
                Ok(Spawned {
                    pid: Some(424244),
                    output: no_output(),
                    exit,
                    control,
                    io,
                })
            }
            Behavior::RecordsSpecEnv(recorder) => {
                lock(recorder).push(spec.env.clone());
                let (exit_tx, exit_rx) = oneshot::channel::<ExitStatus>();
                let control = Box::new(OneshotControl {
                    exit_tx: Mutex::new(Some(exit_tx)),
                    dies_on: DiesOn::Kill,
                });
                let exit: ExitFuture =
                    Box::pin(async move { exit_rx.await.unwrap_or_else(|_| killed_by(SIGKILL)) });
                Ok(Spawned {
                    pid: Some(424245),
                    output: no_output(),
                    exit,
                    control,
                    io: Box::new(NoopPtyIo),
                })
            }
            Behavior::RecordsCommand(recorder) => {
                lock(recorder).push(spec.command.clone());
                let (exit_tx, exit_rx) = oneshot::channel::<ExitStatus>();
                // Exits promptly on SIGTERM so a test can cycle stop → resume → stop without
                // stepping the grace window each time.
                let control = Box::new(OneshotControl {
                    exit_tx: Mutex::new(Some(exit_tx)),
                    dies_on: DiesOn::Terminate,
                });
                let exit: ExitFuture =
                    Box::pin(async move { exit_rx.await.unwrap_or_else(|_| killed_by(SIGKILL)) });
                Ok(Spawned {
                    pid: Some(424246),
                    output: no_output(),
                    exit,
                    control,
                    io: Box::new(NoopPtyIo),
                })
            }
            Behavior::RecordsResizes(log) => {
                lock(&log.spawns).push(spec.size);
                let (exit_tx, exit_rx) = oneshot::channel::<ExitStatus>();
                let control = Box::new(OneshotControl {
                    exit_tx: Mutex::new(Some(exit_tx)),
                    // Exits promptly on SIGTERM so a test can restart it without stepping the
                    // grace window each time.
                    dies_on: DiesOn::Terminate,
                });
                let exit: ExitFuture =
                    Box::pin(async move { exit_rx.await.unwrap_or_else(|_| killed_by(SIGKILL)) });
                Ok(Spawned {
                    pid: Some(424248),
                    output: no_output(),
                    exit,
                    control,
                    io: Box::new(ResizeRecordingPtyIo {
                        resizes: log.resizes.clone(),
                        applied: log.applied.clone(),
                    }),
                })
            }
            Behavior::ExitsWhenTold(trigger) => {
                let (exit_tx, exit_rx) = oneshot::channel::<ExitStatus>();
                let control = Box::new(OneshotControl {
                    exit_tx: Mutex::new(Some(exit_tx)),
                    dies_on: DiesOn::Terminate,
                });
                let trigger = trigger.clone();
                // A launch that carries no process id cannot be told apart from another's, so it
                // is never ended this way — a test that meant to end it reddens instead of ending
                // somebody else's run.
                let process = spawned_process(spec);
                let exit: ExitFuture = Box::pin(async move {
                    let told = async {
                        match process {
                            Some(process) => trigger.awaited(process).await,
                            None => std::future::pending().await,
                        }
                        ExitStatus {
                            code: Some(0),
                            signal: None,
                        }
                    };
                    tokio::select! {
                        signalled = exit_rx => signalled.unwrap_or_else(|_| killed_by(SIGKILL)),
                        exited = told => exited,
                    }
                });
                Ok(Spawned {
                    pid: Some(
                        CUED_EXIT_PID_BASE + process.map(|id| id.get() as u32).unwrap_or_default(),
                    ),
                    output: no_output(),
                    exit,
                    control,
                    io: Box::new(NoopPtyIo),
                })
            }
            Behavior::FailsToSpawn(message) => Err(SpawnError::Spawn(message.clone())),
            Behavior::BlocksOnInput(entered) => {
                let (exit_tx, exit_rx) = oneshot::channel::<ExitStatus>();
                let control = Box::new(OneshotControl {
                    exit_tx: Mutex::new(Some(exit_tx)),
                    dies_on: DiesOn::Terminate,
                });
                let exit: ExitFuture =
                    Box::pin(async move { exit_rx.await.unwrap_or_else(|_| killed_by(SIGKILL)) });
                Ok(Spawned {
                    pid: Some(424247),
                    output: no_output(),
                    exit,
                    control,
                    io: Box::new(BlockingPtyIo {
                        entered: entered.clone(),
                    }),
                })
            }
        }
    }
}

/// Control whose configured signal resolves the paired exit future. Holds only the
/// exit sender, so it never aliases the child handle the exit future owns.
struct OneshotControl {
    exit_tx: Mutex<Option<oneshot::Sender<ExitStatus>>>,
    dies_on: DiesOn,
}

impl OneshotControl {
    fn resolve(&self, status: ExitStatus) {
        if let Some(tx) = lock(&self.exit_tx).take() {
            let _ = tx.send(status);
        }
    }
}

#[async_trait]
impl ProcessControl for OneshotControl {
    async fn terminate(&mut self) -> Result<(), SpawnError> {
        if matches!(self.dies_on, DiesOn::Terminate) {
            self.resolve(killed_by(SIGTERM));
        }
        Ok(())
    }

    async fn kill(&mut self) -> Result<(), SpawnError> {
        self.resolve(killed_by(SIGKILL));
        Ok(())
    }
}

struct NoopControl;

#[async_trait]
impl ProcessControl for NoopControl {
    async fn terminate(&mut self) -> Result<(), SpawnError> {
        Ok(())
    }

    async fn kill(&mut self) -> Result<(), SpawnError> {
        Ok(())
    }
}

/// A [`PtyIo`] that accepts and discards every write and resize — fake children have
/// no real terminal to drive.
struct NoopPtyIo;

#[async_trait]
impl PtyIo for NoopPtyIo {
    async fn write(&self, _data: &[u8]) -> Result<(), SpawnError> {
        Ok(())
    }

    async fn resize(&self, _size: PtySize) -> Result<(), SpawnError> {
        Ok(())
    }
}

/// A [`PtyIo`] whose every write blocks forever, modelling a child that has stopped
/// reading its stdin so a PTY master write stalls in the kernel. Resizes still return —
/// only writes wedge. Fires `entered` as the write begins to block so a test can
/// synchronise on the wedge. The owning actor must stay responsive regardless.
struct BlockingPtyIo {
    entered: Arc<Notify>,
}

#[async_trait]
impl PtyIo for BlockingPtyIo {
    async fn write(&self, _data: &[u8]) -> Result<(), SpawnError> {
        self.entered.notify_one();
        // Never resolves: a child that has stopped draining its stdin stalls the master write
        // in the kernel forever. The owning actor's input pump must absorb this without wedging.
        std::future::pending().await
    }

    async fn resize(&self, _size: PtySize) -> Result<(), SpawnError> {
        Ok(())
    }
}

/// A [`PtyIo`] that records every resize applied to it (discarding writes) and fires `applied`
/// as each resize lands, so a test can prove a resize reaches the child and can wait for the
/// input pump to have processed one without polling.
struct ResizeRecordingPtyIo {
    resizes: Arc<Mutex<Vec<PtySize>>>,
    applied: Arc<Notify>,
}

#[async_trait]
impl PtyIo for ResizeRecordingPtyIo {
    async fn write(&self, _data: &[u8]) -> Result<(), SpawnError> {
        Ok(())
    }

    async fn resize(&self, size: PtySize) -> Result<(), SpawnError> {
        lock(&self.resizes).push(size);
        self.applied.notify_one();
        Ok(())
    }
}

/// The process a launch belongs to, read from the id the supervisor injects into every
/// managed process's environment. `None` for a launch that carries none.
fn spawned_process(spec: &SpawnSpec) -> Option<ProcessId> {
    let raw = spec.env.get(PROCESS_ID_ENV)?.parse().ok()?;
    Some(ProcessId::from_raw(raw))
}

/// A [`PtyIo`] that appends every written byte to the shared log under its own process
/// (discarding resizes), so a test can read back exactly what was sent to that process's input.
struct RecordingPtyIo {
    process: ProcessId,
    log: InputLog,
}

#[async_trait]
impl PtyIo for RecordingPtyIo {
    async fn write(&self, data: &[u8]) -> Result<(), SpawnError> {
        lock(&self.log.per_process)
            .entry(self.process)
            .or_default()
            .extend_from_slice(data);
        Ok(())
    }

    async fn resize(&self, _size: PtySize) -> Result<(), SpawnError> {
        Ok(())
    }
}
