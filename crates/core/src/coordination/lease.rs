//! The lease aggregate (context C6): project-scoped, process-owned lease locks with an explicit
//! TTL.
//!
//! "Signals, not ownership" — acquiring is non-blocking: an attempt on a key another process
//! already holds reports the holder rather than waiting. A lease auto-releases three ways:
//! an explicit [`release`](Leases::release), TTL expiry (applied lazily on the next read), or the
//! owning process closing (the supervisor's [`LockReleaser`](crate::ports::LockReleaser) hook,
//! adapted by [`LeaseReleaser`](super::LeaseReleaser)). Re-acquiring a key you already hold renews
//! it. The aggregate owns the TTL *policy* (a default when the caller names none, and the [`MIN`
//! and `MAX`](MIN_LEASE_TTL) bounds); the durable [`LockRepo`] performs each state-dependent step
//! atomically, so two callers racing to acquire the same free key cannot both be granted it.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::repo::{LockRepo, StoredLease};
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{Clock, StoreError};

/// The lease lifetime used when a caller names none — long enough for a typical coordinated step,
/// short enough that a holder which crashed without releasing frees the key soon after. Lives
/// here, in the core, so every frontend (MCP today, HTTP/CLI later) shares one default rather
/// than each adapter inventing its own.
const DEFAULT_LEASE_TTL: Duration = Duration::from_secs(5 * 60);

/// The floor on a TTL — so an acquired lease is live for at least a meaningful instant rather
/// than expiring the moment it is granted. A request below this (including zero) is raised to it.
const MIN_LEASE_TTL: Duration = Duration::from_secs(1);

/// The ceiling on a TTL — a bound (per the longevity rules) so a caller cannot pin a key
/// indefinitely; holding longer means renewing (re-acquiring). A request above this is clamped.
const MAX_LEASE_TTL: Duration = Duration::from_secs(60 * 60);

/// A live lease as a caller sees it: the key, who holds it, and the absolute expiry. The owner
/// is reported so a contending caller knows who to coordinate with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseView {
    pub key: String,
    pub owner: ProcessId,
    pub expires_unix_millis: u64,
}

impl LeaseView {
    fn of(lease: StoredLease) -> Self {
        Self {
            key: lease.key,
            owner: lease.owner,
            expires_unix_millis: lease.expires_unix_millis,
        }
    }
}

/// The outcome of an acquire attempt: granted to the caller (a fresh lease or a renewal of one it
/// already held), or already held by another live lease — whose holder is reported so the caller
/// can decide without the call blocking.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum AcquireOutcome {
    Acquired(LeaseView),
    Held(LeaseView),
}

/// The lease aggregate over the durable [`LockRepo`] and the [`Clock`]. The repo persists (and
/// makes each state-dependent step atomic); the clock supplies the persistable now the TTL policy
/// compares against. Cheap to clone-share via the `Arc`s it holds.
pub struct Leases {
    repo: Arc<dyn LockRepo>,
    clock: Arc<dyn Clock>,
}

impl Leases {
    /// Builds the aggregate over its durable store and clock.
    pub fn new(repo: Arc<dyn LockRepo>, clock: Arc<dyn Clock>) -> Self {
        Self { repo, clock }
    }

    /// Acquires `(project, key)` for `owner` with `ttl` (the [default](DEFAULT_LEASE_TTL) when
    /// `None`, bounded to [`MIN`](MIN_LEASE_TTL)..=[`MAX`](MAX_LEASE_TTL)). The acquire is atomic
    /// in the store: if the key is held by a live lease owned by someone else, returns
    /// [`AcquireOutcome::Held`] with the holder and changes nothing; a live lease the caller
    /// already owns is renewed with a fresh expiry; an expired lease is treated as free.
    pub fn acquire(
        &self,
        project: ProjectId,
        key: &str,
        owner: ProcessId,
        ttl: Option<Duration>,
    ) -> Result<AcquireOutcome, StoreError> {
        let now = self.clock.now_unix_millis();
        let ttl_millis = ttl
            .unwrap_or(DEFAULT_LEASE_TTL)
            .clamp(MIN_LEASE_TTL, MAX_LEASE_TTL)
            .as_millis() as u64;
        let candidate = StoredLease {
            project,
            key: key.to_owned(),
            owner,
            acquired_unix_millis: now,
            expires_unix_millis: now.saturating_add(ttl_millis),
        };
        match self.repo.acquire(&candidate, now)? {
            None => Ok(AcquireOutcome::Acquired(LeaseView::of(candidate))),
            Some(holder) => Ok(AcquireOutcome::Held(LeaseView::of(holder))),
        }
    }

    /// The current holder of `(project, key)`, or `None` if free or expired. Prunes a lease found
    /// to have expired so a stale row never lingers.
    pub fn status(&self, project: ProjectId, key: &str) -> Result<Option<LeaseView>, StoreError> {
        let now = self.clock.now_unix_millis();
        Ok(self.repo.live(project, key, now)?.map(LeaseView::of))
    }

    /// Releases `(project, key)` if it is held by `owner`, returning whether a lease the caller
    /// owned was released. Releasing a key held by another process does nothing (returns `false`)
    /// — a caller cannot steal or drop another's lease; owner-close handles the rest.
    pub fn release(
        &self,
        project: ProjectId,
        key: &str,
        owner: ProcessId,
    ) -> Result<bool, StoreError> {
        self.repo.release(project, key, owner)
    }

    /// Clears every lease — launch reconciliation (see [`LockRepo::clear`]). A lease does not
    /// outlive the run that created it: its owning process is gone and per-run process ids are
    /// recycled, so the durable table is cleared on launch before any process acquires anew.
    /// Returns how many were cleared.
    pub fn reconcile(&self) -> Result<usize, StoreError> {
        self.repo.clear()
    }
}

#[cfg(test)]
#[path = "lease_tests.rs"]
mod tests;
