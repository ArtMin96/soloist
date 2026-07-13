//! A [`ProcessSpawner`] fake whose children are entirely in-memory: no OS process and
//! no real PTY. Its behaviour is chosen per constructor so a test can drive a specific
//! actor path — the grace window, panic isolation, a clean or signalled exit, or an
//! output stream into the terminal buffers.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot, Notify};

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
type CommandLog = Arc<Mutex<Vec<String>>>;

/// Signal numbers a simulated kill records on a fake child's exit status.
const SIGKILL: i32 = 9;
const SIGTERM: i32 = 15;

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
    /// Stays alive until killed and records every byte written to its PTY into a shared
    /// buffer — so a test can prove what reached a process's input (e.g. a timer delivering
    /// its body as a fresh turn).
    RecordsInput(Arc<Mutex<Vec<u8>>>),
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

    /// A long-lived child that records every byte written to its PTY, returning the spawner and
    /// the shared buffer the test reads. Used to prove what reached a process's input — e.g. that
    /// a fired timer delivered its body, followed by a carriage return, as a fresh turn.
    pub fn records_input() -> (Self, Arc<Mutex<Vec<u8>>>) {
        let recorder = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                behavior: Behavior::RecordsInput(recorder.clone()),
            },
            recorder,
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
            Behavior::RecordsInput(recorder) => {
                let (exit_tx, exit_rx) = oneshot::channel::<ExitStatus>();
                let control = Box::new(OneshotControl {
                    exit_tx: Mutex::new(Some(exit_tx)),
                    dies_on: DiesOn::Kill,
                });
                let exit: ExitFuture =
                    Box::pin(async move { exit_rx.await.unwrap_or_else(|_| killed_by(SIGKILL)) });
                Ok(Spawned {
                    pid: Some(424244),
                    output: no_output(),
                    exit,
                    control,
                    io: Box::new(RecordingPtyIo {
                        recorder: recorder.clone(),
                    }),
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

/// A [`PtyIo`] that appends every written byte to a shared buffer (discarding resizes), so a
/// test can read back exactly what was sent to a process's input.
struct RecordingPtyIo {
    recorder: Arc<Mutex<Vec<u8>>>,
}

#[async_trait]
impl PtyIo for RecordingPtyIo {
    async fn write(&self, data: &[u8]) -> Result<(), SpawnError> {
        lock(&self.recorder).extend_from_slice(data);
        Ok(())
    }

    async fn resize(&self, _size: PtySize) -> Result<(), SpawnError> {
        Ok(())
    }
}
