//! Coordination primitives (context C6): scratchpads, todos, timers, leases, and key-value.
//!
//! Durable, project-scoped state agents share to orchestrate each other — persisted via the
//! [`Store`](crate::ports::Store) ports and kept separate from ephemeral process state, so it
//! outlives any one process or chat. Five aggregates live here: **leases** — project-scoped,
//! process-owned signal locks with an explicit TTL, auto-released on expiry or owner close —
//! **timers** — process-owned timers that, when they fire (at a deadline, or when the agents they
//! watch go idle), deliver a body to their owner as a fresh turn, the token-free orchestration
//! primitive — **scratchpads** — durable, project-scoped shared documents with a disciplined, typed
//! body and revision-guarded writes — **todos** — durable, project-scoped work items with a
//! disciplined document, blockers that gate completion, comments, and a process-owned lock — and
//! **kv** — a simple project-scoped JSON key-value store for small structured state, without
//! revision guarding or process ownership. Leases and timers are process-owned, so launch
//! reconciliation clears them (a per-run process id is recycled, so neither can be matched safely
//! to a later run's processes); scratchpads, todos, and kv entries are durable shared content that
//! **survives** an app restart — only a todo's process-owned *lock* is cleared on launch, never the
//! todo itself.

mod kv;
mod kv_repo;
mod lease;
mod link;
mod releaser;
mod repo;
mod scheduler;
mod scratchpad;
mod scratchpad_repo;
mod timer;
mod timer_repo;
mod todo;
mod todo_releaser;
mod todo_repo;

pub use kv::Kv;
pub use kv_repo::{KvEntry, KvRepo, NoopKvRepo};
pub use lease::{AcquireOutcome, LeaseView, Leases};
pub use link::{is_link, Link, LinkContent, LinkError, LinkTarget};
pub use releaser::LeaseReleaser;
pub use repo::{LockRepo, NoopLockRepo, StoredLease};
pub use scheduler::TimerScheduler;
pub use scratchpad::{
    RenameError, ScratchpadDoc, ScratchpadSummary, ScratchpadView, Scratchpads, WriteError,
};
pub use scratchpad_repo::{
    NoopScratchpadRepo, RenameResult, ScratchpadRepo, StoredScratchpad, TransferResult, WriteResult,
};
pub(crate) use timer::watched_is_idle;
pub use timer::{FireCond, IdleMode, SetWhenIdleOutcome, TimerStatus, TimerView, Timers};
pub use timer_repo::{NewTimer, NoopTimerRepo, StoredTimer, TimerRepo};
pub use todo::{
    Comment, CommentAuthor, CommentOutcome, TodoDoc, TodoError, TodoStatus, TodoSummary, TodoView,
    Todos,
};
pub use todo_releaser::TodoLockReleaser;
pub use todo_repo::{CommentEdit, NoopTodoRepo, StoredTodo, TodoRepo, TodoWriteResult};
