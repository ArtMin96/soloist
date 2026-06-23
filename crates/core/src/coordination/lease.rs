//! The lease aggregate (context C6): project-scoped, process-owned lease locks with an explicit
//! TTL.
//!
//! "Signals, not ownership" — acquiring is non-blocking: an attempt on a key another process
//! already holds reports the holder rather than waiting. A lease auto-releases three ways:
//! an explicit [`release`](Leases::release), TTL expiry (applied lazily on the next read), or the
//! owning process closing (the supervisor's [`LockReleaser`](crate::ports::LockReleaser) hook,
//! adapted by [`LeaseReleaser`](super::LeaseReleaser)). Re-acquiring a key you already hold renews
//! it. Expiry is compared against the persistable wall clock, so a deadline survives a restart —
//! though a lease whose owner did not survive is dropped at launch.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::repo::{LockRepo, StoredLease};
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{Clock, StoreError};

/// The ceiling on a requested TTL — a bound (per the longevity rules) so a caller cannot pin a
/// key indefinitely; holding longer means renewing (re-acquiring). A request above this is
/// clamped, never rejected.
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

/// The lease aggregate over the durable [`LockRepo`] and the [`Clock`]. The repo persists; the
/// clock supplies the persistable now the TTL policy compares against. Cheap to clone-share via
/// the `Arc`s it holds.
pub struct Leases {
    repo: Arc<dyn LockRepo>,
    clock: Arc<dyn Clock>,
}

impl Leases {
    /// Builds the aggregate over its durable store and clock.
    pub fn new(repo: Arc<dyn LockRepo>, clock: Arc<dyn Clock>) -> Self {
        Self { repo, clock }
    }

    /// Acquires `(project, key)` for `owner` with `ttl` (clamped to [`MAX_LEASE_TTL`]). If the key
    /// is held by a live lease owned by someone else, returns [`AcquireOutcome::Held`] with the
    /// holder and changes nothing. A live lease the caller already owns is renewed with a fresh
    /// expiry; an expired lease is treated as free.
    pub fn acquire(
        &self,
        project: ProjectId,
        key: &str,
        owner: ProcessId,
        ttl: Duration,
    ) -> Result<AcquireOutcome, StoreError> {
        let now = self.clock.now_unix_millis();
        if let Some(existing) = self.live_lease(project, key, now)? {
            if existing.owner != owner {
                return Ok(AcquireOutcome::Held(LeaseView::of(existing)));
            }
        }
        let ttl_millis = ttl.min(MAX_LEASE_TTL).as_millis() as u64;
        let lease = StoredLease {
            project,
            key: key.to_owned(),
            owner,
            acquired_unix_millis: now,
            expires_unix_millis: now.saturating_add(ttl_millis),
        };
        self.repo.put(&lease)?;
        Ok(AcquireOutcome::Acquired(LeaseView::of(lease)))
    }

    /// The current holder of `(project, key)`, or `None` if free or expired. Prunes a lease found
    /// to have expired so a stale row never lingers.
    pub fn status(&self, project: ProjectId, key: &str) -> Result<Option<LeaseView>, StoreError> {
        let now = self.clock.now_unix_millis();
        Ok(self.live_lease(project, key, now)?.map(LeaseView::of))
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
        match self.repo.get(project, key)? {
            Some(lease) if lease.owner == owner => self.repo.remove(project, key),
            _ => Ok(false),
        }
    }

    /// Clears every lease — launch reconciliation (see [`LockRepo::clear`]). A lease does not
    /// outlive the run that created it: its owning process is gone and per-run process ids are
    /// recycled, so the durable table is cleared on launch before any process acquires anew.
    /// Returns how many were cleared.
    pub fn reconcile(&self) -> Result<usize, StoreError> {
        self.repo.clear()
    }

    /// The stored lease for `(project, key)` if it is still live at `now`, pruning it if it has
    /// expired. The single place the TTL policy is applied.
    fn live_lease(
        &self,
        project: ProjectId,
        key: &str,
        now: u64,
    ) -> Result<Option<StoredLease>, StoreError> {
        match self.repo.get(project, key)? {
            Some(lease) if lease.expires_unix_millis > now => Ok(Some(lease)),
            Some(_) => {
                self.repo.remove(project, key)?;
                Ok(None)
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
#[path = "lease_tests.rs"]
mod tests;
