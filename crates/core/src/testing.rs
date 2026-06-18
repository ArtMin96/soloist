//! In-memory port fakes used by the core's headless tests: a manually-advanced
//! [`MockClock`], a [`FakeSpawner`] whose children never touch the OS, a
//! [`RecordingLockReleaser`], and [`FakeTrustRepo`]/[`FakeProjectRepo`] standing in
//! for the durable store. These let every actor transition, the grace window, panic
//! isolation, the trust gate, and the sync logic be exercised deterministically — no
//! real time elapsed, no real processes spawned, no SQLite.

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};

use crate::hash::Hash;
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{
    Clock, ExitFuture, ExitStatus, LockReleaser, OrphanControl, OrphanRecord, ProcessControl,
    ProcessSpawner, ProjectRecord, ProjectRepo, PtyIo, PtySize, RuntimeState, RuntimeStateError,
    SpawnError, SpawnSpec, Spawned, StoreError, TrustRepo,
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

// ──────────────────────────────── MockClock ────────────────────────────────

struct Sleeper {
    deadline: Instant,
    waker: oneshot::Sender<()>,
}

struct MockState {
    now: Instant,
    sleepers: Vec<Sleeper>,
}

/// A [`Clock`] whose time only moves when the test calls [`MockClock::advance`].
/// `sleep` registers a waiter that completes once time passes its deadline.
#[derive(Clone)]
pub struct MockClock {
    state: Arc<Mutex<MockState>>,
}

impl MockClock {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockState {
                now: Instant::now(),
                sleepers: Vec::new(),
            })),
        }
    }

    /// Advances time by `by`, completing every sleeper whose deadline has passed.
    pub fn advance(&self, by: Duration) {
        let mut state = lock(&self.state);
        state.now += by;
        let now = state.now;
        let mut pending = Vec::new();
        for sleeper in state.sleepers.drain(..) {
            if sleeper.deadline <= now {
                let _ = sleeper.waker.send(());
            } else {
                pending.push(sleeper);
            }
        }
        state.sleepers = pending;
    }
}

#[async_trait]
impl Clock for MockClock {
    fn now(&self) -> Instant {
        lock(&self.state).now
    }

    async fn sleep(&self, dur: Duration) {
        let rx = {
            let mut state = lock(&self.state);
            let deadline = state.now + dur;
            if deadline <= state.now {
                return;
            }
            let (tx, rx) = oneshot::channel();
            state.sleepers.push(Sleeper {
                deadline,
                waker: tx,
            });
            rx
        };
        let _ = rx.await;
    }
}

// ─────────────────────────────── FakeSpawner ───────────────────────────────

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
    /// Emits the given output chunks, then exits cleanly — drives the actor's PTY
    /// output drain into the terminal buffers without a real process.
    StreamsThenExits(Vec<Vec<u8>>),
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
            behavior: Behavior::StreamsThenExits(chunks),
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
            Behavior::StreamsThenExits(chunks) => {
                let (tx, output) = mpsc::channel(chunks.len().max(1));
                for chunk in chunks {
                    let _ = tx.try_send(chunk.clone());
                }
                drop(tx);
                let exit: ExitFuture = Box::pin(async {
                    ExitStatus {
                        code: Some(0),
                        signal: None,
                    }
                });
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

// ────────────────────────────── RecordingLockReleaser ───────────────────────

/// A [`LockReleaser`] that records which processes it was asked to release locks for,
/// so a test can assert the supervisor frees a process's locks when it closes.
#[derive(Clone, Default)]
pub struct RecordingLockReleaser {
    released: Arc<Mutex<Vec<ProcessId>>>,
}

impl RecordingLockReleaser {
    pub fn new() -> Self {
        Self::default()
    }

    /// The processes whose locks have been released, in order.
    pub fn released(&self) -> Vec<ProcessId> {
        lock(&self.released).clone()
    }
}

impl LockReleaser for RecordingLockReleaser {
    fn release_all(&self, process: ProcessId) {
        lock(&self.released).push(process);
    }
}

// ────────────────────────────── FakeRuntimeState ────────────────────────────

/// An in-memory [`RuntimeState`] standing in for the runtime-state file: records are
/// upserted by pgid, so a test can seed leftovers and assert what reconciliation
/// recorded, forgot, or pruned.
#[derive(Clone, Default)]
pub struct FakeRuntimeState {
    records: Arc<Mutex<Vec<OrphanRecord>>>,
}

impl FakeRuntimeState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-populates a leftover record, as if written by a previous run.
    pub fn seed(&self, record: OrphanRecord) {
        lock(&self.records).push(record);
    }

    /// The currently recorded process groups.
    pub fn records(&self) -> Vec<OrphanRecord> {
        lock(&self.records).clone()
    }
}

impl RuntimeState for FakeRuntimeState {
    fn record(&self, record: &OrphanRecord) -> Result<(), RuntimeStateError> {
        let mut records = lock(&self.records);
        records.retain(|r| r.pgid != record.pgid);
        records.push(record.clone());
        Ok(())
    }

    fn forget(&self, pgid: i32) -> Result<(), RuntimeStateError> {
        lock(&self.records).retain(|r| r.pgid != pgid);
        Ok(())
    }

    fn load(&self) -> Result<Vec<OrphanRecord>, RuntimeStateError> {
        Ok(lock(&self.records).clone())
    }
}

// ───────────────────────────── FakeOrphanControl ─────────────────────────────

/// An in-memory [`OrphanControl`]: a test marks pgids alive, and signalling reaps the
/// group (removing it from the live set) so an adopted process's liveness poll sees it
/// die. Records the signals sent for assertions.
#[derive(Clone, Default)]
pub struct FakeOrphanControl {
    alive: Arc<Mutex<HashSet<i32>>>,
    signalled: Arc<Mutex<Vec<(i32, bool)>>>,
}

impl FakeOrphanControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks a process group alive, as if left running by a previous run.
    pub fn set_alive(&self, pgid: i32) {
        lock(&self.alive).insert(pgid);
    }

    /// The signals sent, as `(pgid, force)` where `force` is SIGKILL vs SIGTERM.
    pub fn signalled(&self) -> Vec<(i32, bool)> {
        lock(&self.signalled).clone()
    }
}

impl OrphanControl for FakeOrphanControl {
    fn is_alive(&self, pgid: i32) -> bool {
        lock(&self.alive).contains(&pgid)
    }

    fn signal(&self, pgid: i32, force: bool) -> Result<(), SpawnError> {
        lock(&self.signalled).push((pgid, force));
        lock(&self.alive).remove(&pgid);
        Ok(())
    }
}

// ─────────────────────────────── FakeTrustRepo ──────────────────────────────

/// An in-memory [`TrustRepo`] keyed by `(project, variant hex)`, for headless trust
/// and sync tests.
#[derive(Default)]
pub struct FakeTrustRepo {
    trusted: Mutex<HashSet<(u64, String)>>,
}

impl FakeTrustRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TrustRepo for FakeTrustRepo {
    fn is_trusted(&self, project: ProjectId, variant: &Hash) -> Result<bool, StoreError> {
        Ok(lock(&self.trusted).contains(&(project.get(), variant.to_hex())))
    }

    fn set_trusted(&self, project: ProjectId, variant: &Hash) -> Result<(), StoreError> {
        lock(&self.trusted).insert((project.get(), variant.to_hex()));
        Ok(())
    }

    fn revoke(&self, project: ProjectId, variant: &Hash) -> Result<(), StoreError> {
        lock(&self.trusted).remove(&(project.get(), variant.to_hex()));
        Ok(())
    }
}

// ────────────────────────────── FakeProjectRepo ─────────────────────────────

struct FakeProjects {
    next_id: u64,
    rows: Vec<ProjectRecord>,
}

/// An in-memory [`ProjectRepo`] assigning sequential ids, for headless registry tests.
/// Mirrors the SQLite store's semantics (canonical-root upsert, cascade-free remove)
/// closely enough to exercise the [`crate::projects::Projects`] logic.
pub struct FakeProjectRepo {
    inner: Mutex<FakeProjects>,
}

impl FakeProjectRepo {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(FakeProjects {
                next_id: 1,
                rows: Vec::new(),
            }),
        }
    }
}

impl Default for FakeProjectRepo {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectRepo for FakeProjectRepo {
    fn upsert(
        &self,
        root: &Path,
        name: Option<&str>,
        icon: Option<&Path>,
    ) -> Result<ProjectRecord, StoreError> {
        let mut inner = lock(&self.inner);
        if let Some(existing) = inner.rows.iter_mut().find(|r| r.root.as_path() == root) {
            existing.name = name.map(str::to_owned);
            existing.icon = icon.map(Path::to_path_buf);
            return Ok(existing.clone());
        }
        let record = ProjectRecord {
            id: ProjectId::from_raw(inner.next_id),
            root: root.to_path_buf(),
            name: name.map(str::to_owned),
            icon: icon.map(Path::to_path_buf),
        };
        inner.next_id += 1;
        inner.rows.push(record.clone());
        Ok(record)
    }

    fn list(&self) -> Result<Vec<ProjectRecord>, StoreError> {
        Ok(lock(&self.inner).rows.iter().rev().cloned().collect())
    }

    fn get(&self, id: ProjectId) -> Result<Option<ProjectRecord>, StoreError> {
        Ok(lock(&self.inner).rows.iter().find(|r| r.id == id).cloned())
    }

    fn remove(&self, id: ProjectId) -> Result<(), StoreError> {
        lock(&self.inner).rows.retain(|r| r.id != id);
        Ok(())
    }
}
