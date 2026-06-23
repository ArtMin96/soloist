use std::path::Path;

use soloist_core::{ProcessId, ProjectId, ProjectRepo, StoredLease};
use tempfile::tempdir;

use super::*;

fn project(store: &SqliteStore, root: &str) -> ProjectId {
    store
        .upsert(Path::new(root), None, None)
        .expect("project for lease fk")
        .id
}

fn lease(project: ProjectId, key: &str, owner: u64, expires: u64) -> StoredLease {
    StoredLease {
        project,
        key: key.to_owned(),
        owner: ProcessId::from_raw(owner),
        acquired_unix_millis: 1_000,
        expires_unix_millis: expires,
    }
}

#[test]
fn a_lease_round_trips_and_persists_across_reopen() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("soloist.db");
    let (project_id, stored) = {
        let store = SqliteStore::open(&db).expect("open");
        let project_id = project(&store, "/p/app");
        let stored = lease(project_id, "deploy", 7, 99_000);
        store.put(&stored).expect("put");
        (project_id, stored)
    };

    let reopened = SqliteStore::open(&db).expect("reopen");
    assert_eq!(
        LockRepo::get(&reopened, project_id, "deploy").expect("get"),
        Some(stored),
        "the durable lease fact survives a restart (the aggregate reconciles staleness on launch)"
    );
}

#[test]
fn put_replaces_an_existing_key() {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let p = project(&store, "/p/app");
    store.put(&lease(p, "deploy", 1, 50_000)).expect("first");
    store.put(&lease(p, "deploy", 2, 60_000)).expect("renew");

    let got = LockRepo::get(&store, p, "deploy")
        .expect("get")
        .expect("present");
    assert_eq!(got.owner, ProcessId::from_raw(2), "the latest write wins");
    assert_eq!(got.expires_unix_millis, 60_000);
}

#[test]
fn get_returns_a_stored_lease_regardless_of_expiry() {
    // The store is expiry-agnostic: it returns whatever is stored, and the core aggregate applies
    // the TTL policy. So an "expired" deadline is still returned here.
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let p = project(&store, "/p/app");
    store.put(&lease(p, "deploy", 1, 1)).expect("put");

    assert!(LockRepo::get(&store, p, "deploy").expect("get").is_some());
}

#[test]
fn remove_reports_whether_a_lease_was_present() {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let p = project(&store, "/p/app");
    store.put(&lease(p, "deploy", 1, 50_000)).expect("put");

    assert!(LockRepo::remove(&store, p, "deploy").expect("remove present"));
    assert!(!LockRepo::remove(&store, p, "deploy").expect("remove absent"));
}

#[test]
fn release_owner_drops_every_lease_of_one_owner() {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let p = project(&store, "/p/app");
    store.put(&lease(p, "deploy", 1, 50_000)).expect("a");
    store.put(&lease(p, "migrate", 1, 50_000)).expect("b");
    store.put(&lease(p, "other", 2, 50_000)).expect("c");

    assert_eq!(
        store
            .release_owner(ProcessId::from_raw(1))
            .expect("release"),
        2
    );
    assert!(LockRepo::get(&store, p, "deploy").expect("get").is_none());
    assert!(
        LockRepo::get(&store, p, "other").expect("get").is_some(),
        "another owner's lease is untouched"
    );
}

#[test]
fn clear_drops_every_lease() {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let p = project(&store, "/p/app");
    store.put(&lease(p, "deploy", 1, 50_000)).expect("a");
    store.put(&lease(p, "migrate", 2, 50_000)).expect("b");

    assert_eq!(store.clear().expect("clear"), 2);
    assert!(LockRepo::get(&store, p, "deploy").expect("get").is_none());
}

#[test]
fn removing_a_project_cascades_its_leases() {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let p = project(&store, "/p/cascade");
    store.put(&lease(p, "deploy", 1, 50_000)).expect("put");

    ProjectRepo::remove(&store, p).expect("remove project");
    assert!(
        LockRepo::get(&store, p, "deploy").expect("get").is_none(),
        "lease rows must cascade-delete with their project"
    );
}

#[test]
fn leases_are_isolated_per_project() {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let a = project(&store, "/p/a");
    let b = project(&store, "/p/b");
    store.put(&lease(a, "deploy", 1, 50_000)).expect("put a");

    assert!(LockRepo::get(&store, a, "deploy").expect("get a").is_some());
    assert!(
        LockRepo::get(&store, b, "deploy").expect("get b").is_none(),
        "the same key is free in another project"
    );
}
