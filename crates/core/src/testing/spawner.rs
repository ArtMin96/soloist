//! A [`ProcessSpawner`] fake whose children are entirely in-memory: no OS process and
//! no real PTY. Its behaviour is chosen per constructor so a test can drive a specific
//! actor path — the grace window, panic isolation, a clean or signalled exit, or an
//! output stream into the terminal buffers.

use std::sync::Mutex;

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};

use crate::ports::{
    ExitFuture, ExitStatus, ProcessControl, ProcessSpawner, PtyIo, PtySize, SpawnError, SpawnSpec,
    Spawned,
};
use crate::sync::lock;

/// Signal numbers a simulated kill records on a fake child's exit status.
const SIGKILL: i32 = 9;
const SIGTERM: i32 = 15;

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
}

/// A closed PTY output channel: the receiver yields nothing and reports EOF at once.
/// Most fake children produce no output; the streaming behaviour overrides this.
fn no_output() -> mpsc::Receiver<Vec<u8>> {
    let (_tx, rx) = mpsc::channel(1);
    rx
}

#[async_trait]
impl ProcessSpawner for FakeSpawner {
    async fn spawn(&self, _spec: &SpawnSpec) -> Result<Spawned, SpawnError> {
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
                    pid: Some(0),
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
