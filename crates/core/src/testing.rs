//! In-memory port fakes used by the core's headless tests: a manually-advanced
//! [`MockClock`] and a [`FakeSpawner`] whose children never touch the OS. These let
//! every actor transition, the grace window, and panic isolation be exercised
//! deterministically with no real time elapsed and no real processes spawned.

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::oneshot;

use crate::hash::Hash;
use crate::ids::ProjectId;
use crate::ports::{
    Clock, ExitFuture, ExitStatus, ProcessControl, ProcessSpawner, ProjectRecord, ProjectRepo,
    SpawnError, SpawnSpec, Spawned, StoreError, TrustRepo,
};
use crate::sync::lock;

/// Signal number a simulated SIGKILL records on a fake child's exit status.
const SIGKILL: i32 = 9;

/// The exit status of a fake child that was force-killed.
fn killed() -> ExitStatus {
    ExitStatus {
        code: None,
        signal: Some(SIGKILL),
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

enum Behavior {
    /// Ignores SIGTERM; exits only when killed — forces the grace path.
    ExitsOnKill,
    /// Panics the moment it is polled after reaching `Running`.
    PanicsAfterRunning,
}

/// A [`ProcessSpawner`] that returns fully in-memory children. Its behaviour is
/// chosen per constructor so tests can drive specific actor paths.
pub struct FakeSpawner {
    behavior: Behavior,
}

impl FakeSpawner {
    pub fn exits_on_kill() -> Self {
        Self {
            behavior: Behavior::ExitsOnKill,
        }
    }

    pub fn panics_after_running() -> Self {
        Self {
            behavior: Behavior::PanicsAfterRunning,
        }
    }
}

#[async_trait]
impl ProcessSpawner for FakeSpawner {
    async fn spawn(&self, _spec: &SpawnSpec) -> Result<Spawned, SpawnError> {
        match self.behavior {
            Behavior::ExitsOnKill => {
                let (exit_tx, exit_rx) = oneshot::channel::<ExitStatus>();
                let control = Box::new(KillExitsControl {
                    exit_tx: Mutex::new(Some(exit_tx)),
                });
                let exit: ExitFuture = Box::pin(async move { exit_rx.await.unwrap_or(killed()) });
                Ok(Spawned {
                    pid: Some(424242),
                    exit,
                    control,
                })
            }
            Behavior::PanicsAfterRunning => {
                let exit: ExitFuture = Box::pin(async { panic!("fake child panicked") });
                Ok(Spawned {
                    pid: Some(0),
                    exit,
                    control: Box::new(NoopControl),
                })
            }
        }
    }
}

/// Control whose `kill` resolves the paired exit future (the child "dies" on SIGKILL
/// only); `terminate` is a no-op, modelling a child that ignores SIGTERM.
struct KillExitsControl {
    exit_tx: Mutex<Option<oneshot::Sender<ExitStatus>>>,
}

#[async_trait]
impl ProcessControl for KillExitsControl {
    async fn terminate(&mut self) -> Result<(), SpawnError> {
        Ok(())
    }

    async fn kill(&mut self) -> Result<(), SpawnError> {
        if let Some(tx) = lock(&self.exit_tx).take() {
            let _ = tx.send(killed());
        }
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

// ─────────────────────────────── FakeTrustRepo ──────────────────────────────

/// An in-memory [`TrustRepo`] keyed by `(project, variant hex)`, for headless
/// trust and sync tests.
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

/// An in-memory [`ProjectRepo`] assigning sequential ids, for headless registry
/// tests. Mirrors the SQLite store's semantics (canonical-root upsert, cascade-free
/// remove) closely enough to exercise the [`crate::projects::Projects`] logic.
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
