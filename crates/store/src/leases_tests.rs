use std::path::Path;
use std::sync::{Arc, Barrier};

use soloist_core::{LockRepo, ProcessId, ProjectId, ProjectRepo, StoredLease};
use tempfile::tempdir;

use super::*;

fn project(store: &SqliteStore, root: &str) -> ProjectId {
    store
        .upsert(Path::new(root), None, None)
        .expect("project for lease fk")
        .id
}

fn lease(project: ProjectId, key: &str, owner: u64, acquired: u64, expires: u64) -> StoredLease {
    StoredLease {
        project,
        key: key.to_owned(),
        owner: ProcessId::from_raw(owner),
        acquired_unix_millis: acquired,
        expires_unix_millis: expires,
    }
}

#[test]
fn acquiring_a_free_key_grants_it_and_persists_across_reopen() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("soloist.db");
    let (project_id, stored) = {
        let store = SqliteStore::open(&db).expect("open");
        let project_id = project(&store, "/p/app");
        let stored = lease(project_id, "deploy", 7, 1_000, 99_000);
        assert_eq!(
            store.acquire(&stored, 1_000).expect("acquire"),
            None,
            "a free key is granted"
        );
        (project_id, stored)
    };

    // The durable lease fact survives a restart (production clears it on launch via reconcile).
    let reopened = SqliteStore::open(&db).expect("reopen");
    assert_eq!(
        reopened.live(project_id, "deploy", 50_000).expect("live"),
        Some(stored),
        "the lease is readable after reopen, before its TTL"
    );
}

#[test]
fn the_owner_renews_and_an_expired_key_is_re_grantable() {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let p = project(&store, "/p/app");

    // The owner re-acquiring renews (granted, new deadline).
    assert_eq!(
        store
            .acquire(&lease(p, "deploy", 1, 1_000, 50_000), 1_000)
            .expect("acquire"),
        None
    );
    assert_eq!(
        store
            .acquire(&lease(p, "deploy", 1, 2_000, 60_000), 2_000)
            .expect("renew"),
        None
    );
    assert_eq!(
        store
            .live(p, "deploy", 2_000)
            .expect("live")
            .map(|l| l.expires_unix_millis),
        Some(60_000),
        "the renewed deadline replaces the old one"
    );

    // Once expired, a different owner may take it.
    assert_eq!(
        store
            .acquire(&lease(p, "deploy", 2, 70_000, 130_000), 70_000)
            .expect("re-grant"),
        None,
        "an expired key is free for a new owner"
    );
    assert_eq!(
        store
            .live(p, "deploy", 70_000)
            .expect("live")
            .map(|l| l.owner),
        Some(ProcessId::from_raw(2))
    );
}

#[test]
fn acquiring_a_held_key_reports_the_holder_without_taking_it() {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let p = project(&store, "/p/app");
    store
        .acquire(&lease(p, "deploy", 1, 1_000, 50_000), 1_000)
        .expect("first");

    let blocked = store
        .acquire(&lease(p, "deploy", 2, 2_000, 60_000), 2_000)
        .expect("contended acquire");
    assert_eq!(
        blocked.map(|l| l.owner),
        Some(ProcessId::from_raw(1)),
        "the live holder is reported to the contender"
    );
    assert_eq!(
        store
            .live(p, "deploy", 2_000)
            .expect("live")
            .map(|l| l.owner),
        Some(ProcessId::from_raw(1)),
        "the holder is unchanged"
    );
}

#[test]
fn live_prunes_an_expired_row_and_reads_a_live_one() {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let p = project(&store, "/p/app");
    store
        .acquire(&lease(p, "deploy", 1, 1_000, 10_000), 1_000)
        .expect("acquire");

    // Before the deadline it reads live.
    assert!(store.live(p, "deploy", 9_000).expect("live").is_some());
    // Past the deadline it reads free and the stale row is pruned.
    assert!(store.live(p, "deploy", 11_000).expect("live").is_none());
    assert!(
        store.live(p, "deploy", 1).expect("live").is_none(),
        "the expired row was removed, not just filtered"
    );
}

#[test]
fn release_frees_a_key_only_for_its_owner() {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let p = project(&store, "/p/app");
    store
        .acquire(&lease(p, "deploy", 1, 1_000, 50_000), 1_000)
        .expect("acquire");

    assert!(
        !store
            .release(p, "deploy", ProcessId::from_raw(2))
            .expect("foreign release"),
        "a non-owner cannot release it"
    );
    assert!(store.live(p, "deploy", 1_000).expect("live").is_some());

    assert!(store
        .release(p, "deploy", ProcessId::from_raw(1))
        .expect("owner release"));
    assert!(store.live(p, "deploy", 1_000).expect("live").is_none());
}

#[test]
fn release_owner_drops_every_lease_of_one_owner() {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let p = project(&store, "/p/app");
    store
        .acquire(&lease(p, "deploy", 1, 1_000, 50_000), 1_000)
        .expect("a");
    store
        .acquire(&lease(p, "migrate", 1, 1_000, 50_000), 1_000)
        .expect("b");
    store
        .acquire(&lease(p, "other", 2, 1_000, 50_000), 1_000)
        .expect("c");

    assert_eq!(
        store
            .release_owner(ProcessId::from_raw(1))
            .expect("release"),
        2
    );
    assert!(store.live(p, "deploy", 1_000).expect("live").is_none());
    assert!(
        store.live(p, "other", 1_000).expect("live").is_some(),
        "another owner's lease is untouched"
    );
}

#[test]
fn clear_drops_every_lease() {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let p = project(&store, "/p/app");
    store
        .acquire(&lease(p, "deploy", 1, 1_000, 50_000), 1_000)
        .expect("a");
    store
        .acquire(&lease(p, "migrate", 2, 1_000, 50_000), 1_000)
        .expect("b");

    assert_eq!(store.clear().expect("clear"), 2);
    assert!(store.live(p, "deploy", 1_000).expect("live").is_none());
}

#[test]
fn removing_a_project_cascades_its_leases() {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let p = project(&store, "/p/cascade");
    store
        .acquire(&lease(p, "deploy", 1, 1_000, 50_000), 1_000)
        .expect("acquire");

    ProjectRepo::remove(&store, p).expect("remove project");
    assert!(
        store.live(p, "deploy", 1_000).expect("live").is_none(),
        "lease rows must cascade-delete with their project"
    );
}

#[test]
fn leases_are_isolated_per_project() {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let a = project(&store, "/p/a");
    let b = project(&store, "/p/b");
    store
        .acquire(&lease(a, "deploy", 1, 1_000, 50_000), 1_000)
        .expect("put a");

    assert!(store.live(a, "deploy", 1_000).expect("live a").is_some());
    assert!(
        store.live(b, "deploy", 1_000).expect("live b").is_none(),
        "the same key is free in another project"
    );
}

#[test]
fn live_in_project_returns_only_this_project_s_unexpired_leases_ordered_by_key() {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let a = project(&store, "/p/a");
    let b = project(&store, "/p/b");
    // Two live and one already-expired lease in project a, plus one in project b.
    store
        .acquire(&lease(a, "migrate", 1, 1_000, 50_000), 1_000)
        .expect("a migrate");
    store
        .acquire(&lease(a, "deploy", 2, 1_000, 50_000), 1_000)
        .expect("a deploy");
    store
        .acquire(&lease(a, "stale", 3, 1_000, 2_000), 1_000)
        .expect("a stale");
    store
        .acquire(&lease(b, "deploy", 4, 1_000, 50_000), 1_000)
        .expect("b deploy");

    // At a `now` past the stale lease's expiry: only project a's two live leases, ordered by key,
    // and never project b's.
    let live = store.live_in_project(a, 10_000).expect("live_in_project");
    let keys: Vec<&str> = live.iter().map(|lease| lease.key.as_str()).collect();
    assert_eq!(
        keys,
        vec!["deploy", "migrate"],
        "live leases, ordered by key"
    );
    assert_eq!(live[0].owner, ProcessId::from_raw(2));
}

#[test]
fn concurrent_acquires_of_one_key_grant_exactly_one_winner() {
    // The race the atomic acquire fixes: many processes acquire the same free key at once. Exactly
    // one must win; every other must be told the (single, stable) holder — never two grants.
    let dir = tempdir().expect("temp dir");
    let store = Arc::new(SqliteStore::open(&dir.path().join("soloist.db")).expect("open"));
    let p = project(&store, "/p/race");
    const NOW: u64 = 1_000;
    const FAR: u64 = 10_000_000;
    const CONTENDERS: u64 = 16;

    let barrier = Arc::new(Barrier::new(CONTENDERS as usize));
    let winners: Vec<u64> = std::thread::scope(|scope| {
        let handles: Vec<_> = (1..=CONTENDERS)
            .map(|owner| {
                let store = store.clone();
                let barrier = barrier.clone();
                scope.spawn(move || {
                    barrier.wait();
                    store
                        .acquire(&lease(p, "deploy", owner, NOW, FAR), NOW)
                        .expect("acquire")
                        // Granted → this owner won; blocked → who actually holds it.
                        .map_or(owner, |holder| holder.owner.get())
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("thread"))
            .collect()
    });

    // Every thread reports the same owner: the one and only winner.
    let held = store
        .live(p, "deploy", NOW)
        .expect("live")
        .expect("a winner holds it");
    assert!(
        winners.iter().all(|&w| w == held.owner.get()),
        "every contender must agree on the single holder {held:?}, got {winners:?}"
    );
    // Exactly one thread reported its own owner id — the grant. Every other reported the winner,
    // a different id, so only the winner satisfies this.
    let self_grants = (1..=CONTENDERS)
        .filter(|&owner| winners[(owner - 1) as usize] == owner)
        .count();
    assert_eq!(self_grants, 1, "exactly one acquire was granted");
}
