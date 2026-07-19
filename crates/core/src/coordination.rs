//! Coordination primitives (context C6): scratchpads, todos, timers, leases, and key-value.
//!
//! Durable, project-scoped state agents share to orchestrate each other — persisted via this
//! context's repository ports and kept separate from ephemeral process state, so it
//! outlives any one process or chat. Five aggregates live here: **leases** — project-scoped,
//! process-owned signal locks with an explicit TTL, auto-released on expiry or owner close —
//! **timers** — process-owned timers that, when they fire (at a deadline, or when the agents they
//! watch go idle), deliver a body to their owner as a fresh turn, the token-free orchestration
//! primitive — **scratchpads** — durable, project-scoped shared free-form Markdown documents with
//! revision-guarded writes — **todos** — durable, project-scoped work items with a title, a
//! free-form Markdown body, blockers that gate completion, comments, and a process-owned lock — and
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
mod scratchpad_link;
mod scratchpad_repo;
mod template;
mod template_repo;
mod timer;
mod timer_repo;
mod todo;
mod todo_blocker;
mod todo_comment;
mod todo_doc;
mod todo_releaser;
mod todo_repo;

pub use kv::{Kv, MAX_KV_VALUE_BYTES};
pub use kv_repo::{KvEntry, KvRepo, NoopKvRepo};
pub use lease::{AcquireOutcome, LeaseView, Leases};
pub use link::{is_link, Link, LinkContent, LinkError, LinkTarget};
pub use releaser::LeaseReleaser;
pub use repo::{LockRepo, NoopLockRepo, StoredLease};
pub use scheduler::TimerScheduler;
pub use scratchpad::{
    RenameError, ScratchpadRef, ScratchpadSummary, ScratchpadTransfer, ScratchpadView, Scratchpads,
    WriteError, MAX_SCRATCHPAD_CONTENT_BYTES,
};
pub use scratchpad_link::ScratchpadLink;
pub use scratchpad_repo::{
    NoopScratchpadRepo, RenameResult, ScratchpadRepo, StoredScratchpad, TransferResult,
    TransferredScratchpad, WriteResult,
};
pub use template::{
    placeholders, ExportedTemplate, TemplateSummary, TemplateView, TemplateWriteError, Templates,
    MAX_TEMPLATE_BODY, MAX_TEMPLATE_DESCRIPTION, MAX_TEMPLATE_NAME,
};
pub use template_repo::{NoopTemplateRepo, StoredTemplate, TemplateRepo, TemplateWriteResult};
pub(crate) use timer::watched_is_idle;
pub use timer::{
    FireCond, IdleMode, SetWhenIdleOutcome, TimerStatus, TimerView, Timers, MAX_TIMER_BODY_BYTES,
};
pub use timer_repo::{NewTimer, NoopTimerRepo, StoredTimer, TimerRepo};
pub use todo::{TodoError, TodoSummary, TodoView, Todos};
pub use todo_comment::{Comment, CommentAuthor, CommentOutcome};
pub use todo_doc::{TodoDoc, TodoStatus, MAX_TODO_DOC_BYTES};
pub use todo_releaser::TodoLockReleaser;
pub use todo_repo::{CommentEdit, NoopTodoRepo, StoredTodo, TodoRepo, TodoWriteResult};
