//! Coordination primitives (context C6): scratchpads, todos, timers, leases, and key-value.
//!
//! Durable, project-scoped state agents share to orchestrate each other — persisted via the
//! [`Store`](crate::ports::Store) ports and kept separate from ephemeral process state, so it
//! outlives any one process or chat. Leases are the first aggregate here: project-scoped,
//! process-owned signal locks with an explicit TTL, auto-released on expiry or owner close.

mod lease;
mod releaser;
mod repo;

pub use lease::{AcquireOutcome, LeaseView, Leases};
pub use releaser::LeaseReleaser;
pub use repo::{LockRepo, NoopLockRepo, StoredLease};
