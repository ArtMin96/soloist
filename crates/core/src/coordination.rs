//! Coordination primitives (context C6): scratchpads, todos, timers, leases, and key-value.
//!
//! Durable, project-scoped state agents share to orchestrate each other — persisted via the
//! [`Store`](crate::ports::Store) ports and kept separate from ephemeral process state, so it
//! outlives any one process or chat. Three aggregates live here so far: **leases** — project-scoped,
//! process-owned signal locks with an explicit TTL, auto-released on expiry or owner close —
//! **timers** — process-owned timers that, when they fire (at a deadline, or when the agents they
//! watch go idle), deliver a body to their owner as a fresh turn, the token-free orchestration
//! primitive — and **scratchpads** — durable, project-scoped shared documents with a disciplined,
//! typed body and revision-guarded writes. Leases and timers are process-owned, so launch
//! reconciliation clears them (a per-run process id is recycled, so neither can be matched safely to
//! a later run's processes); scratchpads are durable shared content that **survives** a restart
//! (matrix G11).

mod lease;
mod releaser;
mod repo;
mod scheduler;
mod scratchpad;
mod scratchpad_repo;
mod timer;
mod timer_repo;

pub use lease::{AcquireOutcome, LeaseView, Leases};
pub use releaser::LeaseReleaser;
pub use repo::{LockRepo, NoopLockRepo, StoredLease};
pub use scheduler::TimerScheduler;
pub use scratchpad::{
    RenameError, ScratchpadDoc, ScratchpadSummary, ScratchpadView, Scratchpads, WriteError,
};
pub use scratchpad_repo::{
    NoopScratchpadRepo, RenameResult, ScratchpadRepo, StoredScratchpad, WriteResult,
};
pub(crate) use timer::watched_is_idle;
pub use timer::{FireCond, IdleMode, SetWhenIdleOutcome, TimerStatus, TimerView, Timers};
pub use timer_repo::{NewTimer, NoopTimerRepo, StoredTimer, TimerRepo};
