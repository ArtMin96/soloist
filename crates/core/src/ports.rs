//! Hexagonal ports: the traits the pure core defines and adapters implement.
//!
//! The core depends only on these abstractions, never on a concrete OS, UI,
//! transport, or storage technology. Each port states its contract in doc comments;
//! adapters (the `pty`, `store`, and `app` crates, plus in-test fakes) provide the
//! implementations. Mockable ports plus a controllable [`Clock`] are what make the
//! whole supervisor headless-testable with no real time elapsed.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::{Duration, Instant};

use async_trait::async_trait;

use crate::hash::Hash;
use crate::ids::{ProcessId, ProjectId};

// ───────────────────────────── ProcessSpawner ──────────────────────────────

/// What to launch: a shell command line, the directory it runs in, and per-process
/// environment overrides. The command is executed through the user's login shell
/// (`$SHELL -lc <command>`) so aliases and version-manager PATHs resolve; `env`
/// overrides are layered onto the inherited app environment (process `env` wins —
/// the documented precedence). The PTY size is added in the terminal-I/O phase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpawnSpec {
    /// The shell command line, run via the login shell.
    pub command: String,
    /// The absolute working directory the command runs in.
    pub working_dir: PathBuf,
    /// Per-process environment overrides, layered onto the inherited app env. A sorted
    /// map so application order is deterministic.
    pub env: BTreeMap<String, String>,
}

/// How a child finished: an exit code, or the signal that terminated it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExitStatus {
    pub code: Option<i32>,
    pub signal: Option<i32>,
}

impl ExitStatus {
    /// True only for a clean `exit(0)` with no terminating signal.
    pub fn success(&self) -> bool {
        self.code == Some(0) && self.signal.is_none()
    }
}

/// Errors a spawner adapter surfaces. Typed so callers handle a missing binary or a
/// failed signal as ordinary values rather than panics.
#[derive(Debug, thiserror::Error)]
pub enum SpawnError {
    #[error("failed to spawn process: {0}")]
    Spawn(String),
    #[error("failed to signal process: {0}")]
    Signal(String),
}

/// A future that resolves once the child has exited and been reaped.
pub type ExitFuture = Pin<Box<dyn Future<Output = ExitStatus> + Send>>;

/// The result of a spawn: the child's pid, a future that resolves when it exits
/// (and is reaped), and a control handle to signal it. The exit future and the
/// control handle are separate values so the owning actor can race "child exited"
/// against "stop requested" without aliasing one handle.
pub struct Spawned {
    pub pid: Option<u32>,
    pub exit: ExitFuture,
    pub control: Box<dyn ProcessControl>,
}

/// Signals a running child. Adapters target the child's whole **process group**, not
/// a bare pid, so a process that forks children is fully torn down (no orphans).
#[async_trait]
pub trait ProcessControl: Send + Sync {
    /// Requests a graceful stop (SIGTERM to the process group).
    async fn terminate(&mut self) -> Result<(), SpawnError>;
    /// Forces termination (SIGKILL to the process group).
    async fn kill(&mut self) -> Result<(), SpawnError>;
}

/// Spawns OS processes. The real adapter spawns into a fresh process group via a
/// PTY (later phases) or `tokio::process` (the skeleton); the test adapter returns a
/// fully in-memory fake child.
#[async_trait]
pub trait ProcessSpawner: Send + Sync {
    /// Spawns `spec` into a fresh process group.
    async fn spawn(&self, spec: &SpawnSpec) -> Result<Spawned, SpawnError>;
}

// ──────────────────────────────────── Clock ────────────────────────────────

/// The passage of time, behind a port so timing logic (grace windows, debounce,
/// backoff, rate limits) is driven by a deterministic mock in tests.
#[async_trait]
pub trait Clock: Send + Sync {
    /// The current instant per this clock.
    fn now(&self) -> Instant;
    /// Completes after `dur` has elapsed per this clock. A mock clock advances only
    /// when its test explicitly steps it, so no wall-clock time passes.
    async fn sleep(&self, dur: Duration);
}

/// The real clock, backed by `tokio::time`. Lives in the core because the core
/// already depends on `tokio`; it carries no business state.
#[derive(Clone, Copy, Default)]
pub struct TokioClock;

#[async_trait]
impl Clock for TokioClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    async fn sleep(&self, dur: Duration) {
        tokio::time::sleep(dur).await;
    }
}

// ──────────────────────────────────── Store ────────────────────────────────

/// Errors a durable-store adapter surfaces.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("store backend error: {0}")]
    Backend(String),
}

/// Durable key/value metadata — the walking-skeleton seed of the repository surface
/// (trust, projects, todos, scratchpads, …) that later phases grow on top of the
/// SQLite adapter. Kept synchronous: backing reads/writes are tiny and local.
pub trait Store: Send + Sync {
    /// Reads a metadata value by key, `None` if absent.
    fn meta_get(&self, key: &str) -> Result<Option<String>, StoreError>;
    /// Inserts or replaces a metadata value.
    fn meta_set(&self, key: &str, value: &str) -> Result<(), StoreError>;
}

/// A persisted project: a workspace root plus optional display metadata. `id` is
/// the durable [`ProjectId`] (stable across runs), assigned by the store from the
/// project's canonical root path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRecord {
    pub id: ProjectId,
    pub root: PathBuf,
    pub name: Option<String>,
    pub icon: Option<PathBuf>,
}

/// Durable registry of projects (the workspace roots Soloist manages). The
/// canonical `root` path is the natural key; `id` is stable across runs.
pub trait ProjectRepo: Send + Sync {
    /// Inserts the project at `root`, or updates its metadata if already present,
    /// returning its durable record.
    fn upsert(
        &self,
        root: &Path,
        name: Option<&str>,
        icon: Option<&Path>,
    ) -> Result<ProjectRecord, StoreError>;
    /// All known projects, most-recently-added first.
    fn list(&self) -> Result<Vec<ProjectRecord>, StoreError>;
    /// One project by id, `None` if absent.
    fn get(&self, id: ProjectId) -> Result<Option<ProjectRecord>, StoreError>;
    /// Removes a project (cascading to its trust records).
    fn remove(&self, id: ProjectId) -> Result<(), StoreError>;
}

/// Durable trust store, keyed by `(project, command-variant hash)`. The presence of
/// a row means that exact command variant is trusted to run within that project.
/// All methods are idempotent.
pub trait TrustRepo: Send + Sync {
    /// Whether `variant` is trusted within `project`.
    fn is_trusted(&self, project: ProjectId, variant: &Hash) -> Result<bool, StoreError>;
    /// Marks `variant` trusted within `project`.
    fn set_trusted(&self, project: ProjectId, variant: &Hash) -> Result<(), StoreError>;
    /// Revokes trust for `variant` within `project`.
    fn revoke(&self, project: ProjectId, variant: &Hash) -> Result<(), StoreError>;
}

// ──────────────────────────────── LockReleaser ─────────────────────────────

/// Notified when a managed process closes so any cross-process coordination state it
/// holds — todo locks, leases — can be released. The coordination context (C6)
/// provides the real implementation in a later phase; until then [`NoopLockReleaser`]
/// satisfies the port. The supervisor calls this whenever a process reaches a
/// terminal state (stopped or crashed), matching Solo's "locks auto-release when the
/// owning process closes".
pub trait LockReleaser: Send + Sync {
    /// Releases every lock or lease owned by `process`. Must not block or panic.
    fn release_all(&self, process: ProcessId);
}

/// A [`LockReleaser`] that does nothing — the default until coordination (C6) lands.
#[derive(Clone, Copy, Default)]
pub struct NoopLockReleaser;

impl LockReleaser for NoopLockReleaser {
    fn release_all(&self, _process: ProcessId) {}
}

// ───────────── Ports realized in later phases (contracts only) ──────────────

/// Watches the filesystem and emits debounced create/modify events for configured
/// globs, relative to the project root, with sensible default ignores. Methods are
/// added when the file-watch feature lands.
pub trait FileWatcher: Send + Sync {}

/// Emits best-effort desktop notifications. Must never block or panic the core; a
/// missing notification backend degrades silently. Methods are added when the
/// notification feature lands.
pub trait Notifier: Send + Sync {}

/// Produces an idle summary for an agent from a rendered-text snapshot. Optional by
/// design: when absent, idle detection degrades to the heuristic-only signal.
/// Methods are added when the agent-summary feature lands.
pub trait Summarizer: Send + Sync {}
