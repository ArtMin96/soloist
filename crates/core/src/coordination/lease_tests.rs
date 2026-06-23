use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::ids::{ProcessId, ProjectId};
use crate::testing::{FakeLockRepo, MockClock};

/// Builds a lease aggregate over an in-memory repo and a mock clock, returning both so a test can
/// advance time and inspect the store.
fn leases() -> (Leases, MockClock, Arc<FakeLockRepo>) {
    let repo = Arc::new(FakeLockRepo::new());
    let clock = MockClock::new();
    (
        Leases::new(repo.clone(), Arc::new(clock.clone())),
        clock,
        repo,
    )
}

fn project() -> ProjectId {
    ProjectId::from_raw(1)
}

/// A finite TTL, the common case.
fn ttl(secs: u64) -> Option<Duration> {
    Some(Duration::from_secs(secs))
}

#[test]
fn acquiring_a_free_key_grants_it() {
    let (leases, _clock, _repo) = leases();
    let owner = ProcessId::from_raw(7);

    let outcome = leases
        .acquire(project(), "deploy", owner, ttl(30))
        .expect("acquire");

    match outcome {
        AcquireOutcome::Acquired(view) => {
            assert_eq!(view.key, "deploy");
            assert_eq!(view.owner, owner);
        }
        AcquireOutcome::Held(_) => panic!("a free key must be granted"),
    }
}

#[test]
fn acquiring_a_held_key_reports_the_holder_without_taking_it() {
    let (leases, _clock, _repo) = leases();
    let holder = ProcessId::from_raw(1);
    let contender = ProcessId::from_raw(2);
    leases
        .acquire(project(), "deploy", holder, ttl(30))
        .expect("first acquire");

    let outcome = leases
        .acquire(project(), "deploy", contender, ttl(30))
        .expect("contended acquire");

    match outcome {
        AcquireOutcome::Held(view) => assert_eq!(view.owner, holder, "the holder is reported"),
        AcquireOutcome::Acquired(_) => panic!("a held key must not be taken from its owner"),
    }
    // The contender did not become the owner.
    assert_eq!(
        leases
            .status(project(), "deploy")
            .expect("status")
            .map(|v| v.owner),
        Some(holder)
    );
}

#[test]
fn a_lease_expires_after_its_ttl_and_is_then_free() {
    let (leases, clock, _repo) = leases();
    let holder = ProcessId::from_raw(1);
    leases
        .acquire(project(), "deploy", holder, ttl(10))
        .expect("acquire");

    // Still held just before the TTL.
    clock.advance(Duration::from_secs(9));
    assert!(leases
        .status(project(), "deploy")
        .expect("status")
        .is_some());

    // Past the TTL the key reads free, and a new owner can take it.
    clock.advance(Duration::from_secs(2));
    assert!(leases
        .status(project(), "deploy")
        .expect("status")
        .is_none());
    let other = ProcessId::from_raw(2);
    let outcome = leases
        .acquire(project(), "deploy", other, ttl(10))
        .expect("re-acquire after expiry");
    assert!(matches!(outcome, AcquireOutcome::Acquired(view) if view.owner == other));
}

#[test]
fn an_owner_renews_its_own_lease_by_re_acquiring() {
    let (leases, clock, _repo) = leases();
    let owner = ProcessId::from_raw(1);
    leases
        .acquire(project(), "deploy", owner, ttl(10))
        .expect("acquire");

    // Re-acquire near expiry: the owner keeps the key with a fresh deadline.
    clock.advance(Duration::from_secs(9));
    let outcome = leases
        .acquire(project(), "deploy", owner, ttl(10))
        .expect("renew");
    assert!(matches!(outcome, AcquireOutcome::Acquired(_)));

    // After the original deadline would have passed, the renewed lease is still held.
    clock.advance(Duration::from_secs(5));
    assert_eq!(
        leases
            .status(project(), "deploy")
            .expect("status")
            .map(|v| v.owner),
        Some(owner)
    );
}

#[test]
fn release_frees_a_key_only_for_its_owner() {
    let (leases, _clock, _repo) = leases();
    let owner = ProcessId::from_raw(1);
    let other = ProcessId::from_raw(2);
    leases
        .acquire(project(), "deploy", owner, ttl(30))
        .expect("acquire");

    // A non-owner cannot release it.
    assert!(!leases
        .release(project(), "deploy", other)
        .expect("foreign release"));
    assert!(leases
        .status(project(), "deploy")
        .expect("status")
        .is_some());

    // The owner releases it, and it becomes free.
    assert!(leases
        .release(project(), "deploy", owner)
        .expect("owner release"));
    assert!(leases
        .status(project(), "deploy")
        .expect("status")
        .is_none());
}

#[test]
fn releasing_an_owner_drops_every_lease_it_holds() {
    let (leases, _clock, repo) = leases();
    let owner = ProcessId::from_raw(1);
    leases
        .acquire(project(), "deploy", owner, ttl(30))
        .expect("first");
    leases
        .acquire(project(), "migrate", owner, ttl(30))
        .expect("second");

    // The supervisor's hook routes here on process close.
    let dropped = repo.release_owner(owner).expect("release owner");
    assert_eq!(dropped, 2);
    assert!(leases
        .status(project(), "deploy")
        .expect("status")
        .is_none());
    assert!(leases
        .status(project(), "migrate")
        .expect("status")
        .is_none());
}

#[test]
fn launch_reconcile_clears_every_lease() {
    let (leases, _clock, _repo) = leases();
    leases
        .acquire(project(), "from-last-run", ProcessId::from_raw(1), ttl(30))
        .expect("first");
    leases
        .acquire(project(), "also-last-run", ProcessId::from_raw(2), ttl(30))
        .expect("second");

    // On launch every persisted lease is stale (its owner's run ended; per-run ids are recycled),
    // so reconcile clears the table before any process acquires anew.
    let cleared = leases.reconcile().expect("reconcile");

    assert_eq!(cleared, 2);
    assert!(leases
        .status(project(), "from-last-run")
        .expect("status")
        .is_none());
    assert!(leases
        .status(project(), "also-last-run")
        .expect("status")
        .is_none());
}

#[test]
fn an_omitted_ttl_uses_the_default() {
    let (leases, clock, _repo) = leases();
    let owner = ProcessId::from_raw(1);
    leases
        .acquire(project(), "deploy", owner, None)
        .expect("acquire with the default ttl");

    // The default is minutes, not seconds: still held well after a 10-second-style lease would
    // have lapsed, and gone well before the one-hour ceiling.
    clock.advance(Duration::from_secs(60));
    assert!(
        leases
            .status(project(), "deploy")
            .expect("status")
            .is_some(),
        "the default ttl outlasts a minute"
    );
    clock.advance(Duration::from_secs(60 * 60));
    assert!(
        leases
            .status(project(), "deploy")
            .expect("status")
            .is_none(),
        "the default ttl is well under the ceiling"
    );
}

#[test]
fn a_sub_second_ttl_is_raised_to_the_floor() {
    let (leases, clock, _repo) = leases();
    let owner = ProcessId::from_raw(1);
    // A zero TTL would otherwise grant a lease that is already expired; the floor keeps it briefly
    // live so an acquire is meaningful.
    let outcome = leases
        .acquire(project(), "deploy", owner, Some(Duration::ZERO))
        .expect("acquire");
    assert!(matches!(outcome, AcquireOutcome::Acquired(_)));
    assert!(
        leases
            .status(project(), "deploy")
            .expect("status")
            .is_some(),
        "a floored lease is live the instant it is acquired"
    );

    // It still expires shortly after — the floor is a second, not forever.
    clock.advance(Duration::from_secs(2));
    assert!(leases
        .status(project(), "deploy")
        .expect("status")
        .is_none());
}

#[test]
fn a_ttl_above_the_ceiling_is_clamped() {
    let (leases, clock, _repo) = leases();
    let owner = ProcessId::from_raw(1);
    // Request far beyond the ceiling; the lease must not outlive it.
    leases
        .acquire(project(), "deploy", owner, ttl(60 * 60 * 24))
        .expect("acquire");

    clock.advance(Duration::from_secs(60 * 60) + Duration::from_secs(1));
    assert!(
        leases
            .status(project(), "deploy")
            .expect("status")
            .is_none(),
        "a clamped lease expires at the ceiling, not the requested TTL"
    );
}

#[test]
fn leases_are_scoped_per_project() {
    let (leases, _clock, _repo) = leases();
    let owner = ProcessId::from_raw(1);
    let a = ProjectId::from_raw(1);
    let b = ProjectId::from_raw(2);
    leases
        .acquire(a, "deploy", owner, ttl(30))
        .expect("acquire in a");

    // The same key is free in a different project.
    assert!(leases.status(b, "deploy").expect("status").is_none());
    let outcome = leases
        .acquire(b, "deploy", ProcessId::from_raw(2), ttl(30))
        .expect("acquire in b");
    assert!(matches!(outcome, AcquireOutcome::Acquired(_)));
}
