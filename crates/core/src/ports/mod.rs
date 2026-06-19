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
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::hash::Hash;
use crate::ids::{ProcessId, ProjectId};

mod bundle;

pub use bundle::{CorePorts, CorePortsBuilder};

// ───────────────────────────── ProcessSpawner ──────────────────────────────

/// The character dimensions of a pseudo-terminal. The child reads these from its
/// controlling terminal (and on a change receives `SIGWINCH`), so a full-screen TUI
/// lays out to the right size. Pixel dimensions are not modelled — Soloist drives
/// only the cell grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PtySize {
    pub cols: u16,
    pub rows: u16,
}

impl Default for PtySize {
    /// A conventional 80×24 terminal — the size a process starts at before a viewer
    /// attaches and sends the real dimensions.
    fn default() -> Self {
        Self { cols: 80, rows: 24 }
    }
}

/// What to launch: a shell command line, the directory it runs in, per-process
/// environment overrides, and the initial PTY size. The command is executed through
/// the user's login shell (`$SHELL -lc <command>`) so aliases and version-manager
/// PATHs resolve; `env` overrides are layered onto the inherited app environment
/// (process `env` wins — the documented precedence).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpawnSpec {
    /// The shell command line, run via the login shell.
    pub command: String,
    /// The absolute working directory the command runs in.
    pub working_dir: PathBuf,
    /// Per-process environment overrides, layered onto the inherited app env. A sorted
    /// map so application order is deterministic.
    pub env: BTreeMap<String, String>,
    /// The initial terminal dimensions the child is spawned with.
    pub size: PtySize,
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

/// The result of a spawn: the child's pid, its PTY output byte stream, a future that
/// resolves when it exits (and is reaped), a control handle to signal it, and an I/O
/// handle to write input and resize it. The exit future, control handle, and I/O
/// handle are separate values so the owning actor can race "child exited" against
/// "stop requested" while still writing input, without aliasing one handle.
///
/// `output` is a **bounded** receiver of raw PTY bytes: when the actor cannot keep
/// up, the adapter's read loop blocks on send, the OS PTY buffer fills, and the child
/// blocks on write — backpressure all the way down, never an unbounded buffer.
pub struct Spawned {
    pub pid: Option<u32>,
    pub output: mpsc::Receiver<Vec<u8>>,
    pub exit: ExitFuture,
    pub control: Box<dyn ProcessControl>,
    pub io: Box<dyn PtyIo>,
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

/// Writes input to a running child's PTY and resizes it. Held by the owning actor —
/// the single writer to the child's stdin. Both operations are best-effort: once the
/// child has exited the adapter returns an error the caller logs and drops rather than
/// propagating as a process failure.
#[async_trait]
pub trait PtyIo: Send + Sync {
    /// Forwards bytes (typed text or raw control sequences) to the PTY master.
    async fn write(&self, data: &[u8]) -> Result<(), SpawnError>;
    /// Resizes the PTY so the child sees the new dimensions (and a `SIGWINCH`).
    async fn resize(&self, size: PtySize) -> Result<(), SpawnError>;
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

// ───────────────────────────── Orphan adoption ──────────────────────────────

/// A record of a managed process that was running when Soloist last persisted state,
/// written to the runtime-state file so a leftover from a crash or force-quit can be
/// reconciled on the next launch. The identity `{project_root, name, command}` is what
/// an adoption match is keyed on; `pgid` is the process group to adopt or reap.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrphanRecord {
    pub project_root: PathBuf,
    pub name: String,
    pub command: String,
    pub pgid: i32,
}

/// Errors a runtime-state adapter surfaces.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeStateError {
    #[error("runtime-state error: {0}")]
    Backend(String),
}

/// The small, file-backed record of currently-running process groups, kept outside
/// SQLite (it is ephemeral runtime state rebuilt each run, not durable domain state).
/// Updated as processes start and stop; read once on launch to reconcile orphans.
/// Implementations must tolerate a missing or partially written file (treat as empty).
pub trait RuntimeState: Send + Sync {
    /// Records (upserts by pgid) a running process group.
    fn record(&self, record: &OrphanRecord) -> Result<(), RuntimeStateError>;
    /// Removes the record for `pgid` once its process has been reaped.
    fn forget(&self, pgid: i32) -> Result<(), RuntimeStateError>;
    /// Every recorded process group, for reconciliation on launch.
    fn load(&self) -> Result<Vec<OrphanRecord>, RuntimeStateError>;
}

/// Operates on a bare process group by id — used to adopt a leftover process whose
/// original child handle is gone: check whether it is still alive, and signal it to
/// stop. Targets the whole group (as the spawner does) so a forking orphan is reaped.
pub trait OrphanControl: Send + Sync {
    /// Whether the process group `pgid` still has a live member.
    fn is_alive(&self, pgid: i32) -> bool;
    /// Signals the group: a graceful SIGTERM, or SIGKILL when `force`.
    fn signal(&self, pgid: i32, force: bool) -> Result<(), SpawnError>;
}

/// A [`RuntimeState`] that records nothing and reconciles to empty — the default when
/// orphan adoption is not wired (tests that do not exercise it, headless tools).
#[derive(Clone, Copy, Default)]
pub struct NoopRuntimeState;

impl RuntimeState for NoopRuntimeState {
    fn record(&self, _record: &OrphanRecord) -> Result<(), RuntimeStateError> {
        Ok(())
    }
    fn forget(&self, _pgid: i32) -> Result<(), RuntimeStateError> {
        Ok(())
    }
    fn load(&self) -> Result<Vec<OrphanRecord>, RuntimeStateError> {
        Ok(Vec::new())
    }
}

/// An [`OrphanControl`] that reports nothing alive — the default when orphan adoption
/// is not wired. With it, reconciliation prunes every record and adopts nothing.
#[derive(Clone, Copy, Default)]
pub struct NoopOrphanControl;

impl OrphanControl for NoopOrphanControl {
    fn is_alive(&self, _pgid: i32) -> bool {
        false
    }
    fn signal(&self, _pgid: i32, _force: bool) -> Result<(), SpawnError> {
        Ok(())
    }
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
