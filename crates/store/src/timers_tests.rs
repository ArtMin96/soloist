use std::path::Path;
use std::sync::{Arc, Barrier};

use soloist_core::{FireCond, NewTimer, ProcessId, ProjectId, ProjectRepo, TimerRepo, TimerStatus};
use tempfile::tempdir;

use super::*;

fn open() -> (tempfile::TempDir, SqliteStore) {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    (dir, store)
}

fn project(store: &SqliteStore, root: &str) -> ProjectId {
    store
        .upsert(Path::new(root), None, None)
        .expect("project for timer fk")
        .id
}

fn at_timer(project: ProjectId, owner: u64, body: &str, deadline: u64) -> NewTimer {
    NewTimer {
        project,
        owner: ProcessId::from_raw(owner),
        body: body.to_owned(),
        fire: FireCond::At,
        deadline_unix_millis: deadline,
    }
}

#[test]
fn create_then_armed_round_trips_the_timer_including_its_fire_condition() {
    let (_dir, store) = open();
    let p = project(&store, "/p/app");
    let watched = vec![ProcessId::from_raw(5), ProcessId::from_raw(6)];
    let id = store
        .create(&NewTimer {
            project: p,
            owner: ProcessId::from_raw(7),
            body: "all done".into(),
            fire: FireCond::WhenIdleAll {
                watched: watched.clone(),
            },
            deadline_unix_millis: 90_000,
        })
        .expect("create");

    let armed = store.armed().expect("armed");
    assert_eq!(armed.len(), 1);
    let timer = &armed[0];
    assert_eq!(timer.id, id);
    assert_eq!(timer.owner, ProcessId::from_raw(7));
    assert_eq!(timer.body, "all done");
    assert_eq!(timer.deadline_unix_millis, 90_000);
    assert_eq!(timer.status, TimerStatus::Armed);
    assert_eq!(
        timer.fire,
        FireCond::WhenIdleAll { watched },
        "the fire condition survives the JSON column round-trip"
    );
}

#[test]
fn create_assigns_distinct_ids() {
    let (_dir, store) = open();
    let p = project(&store, "/p/app");
    let a = store.create(&at_timer(p, 1, "a", 10_000)).expect("a");
    let b = store.create(&at_timer(p, 1, "b", 10_000)).expect("b");
    assert_ne!(a, b, "every timer gets its own id");
}

#[test]
fn take_if_armed_claims_a_timer_once_and_removes_it() {
    let (_dir, store) = open();
    let p = project(&store, "/p/app");
    let id = store
        .create(&at_timer(p, 1, "ping", 10_000))
        .expect("create");

    let claimed = store.take_if_armed(id).expect("first claim");
    assert_eq!(claimed.map(|timer| timer.body), Some("ping".to_owned()));
    // A second claim finds nothing — the timer fired exactly once.
    assert!(store.take_if_armed(id).expect("second claim").is_none());
    assert!(store.armed().expect("armed").is_empty());
}

#[test]
fn take_if_armed_never_claims_a_paused_timer() {
    let (_dir, store) = open();
    let p = project(&store, "/p/app");
    let id = store
        .create(&at_timer(p, 1, "ping", 10_000))
        .expect("create");
    assert!(store
        .pause(id, ProcessId::from_raw(1), 1_000)
        .expect("pause"));

    assert!(
        store.take_if_armed(id).expect("claim").is_none(),
        "a paused timer is never fired"
    );
}

#[test]
fn cancel_removes_only_for_the_owner() {
    let (_dir, store) = open();
    let p = project(&store, "/p/app");
    let id = store
        .create(&at_timer(p, 1, "ping", 10_000))
        .expect("create");

    assert!(!store
        .cancel(id, ProcessId::from_raw(2))
        .expect("foreign cancel"));
    assert!(store
        .cancel(id, ProcessId::from_raw(1))
        .expect("owner cancel"));
    assert!(store.armed().expect("armed").is_empty());
}

#[test]
fn pause_freezes_the_remaining_time_and_resume_re_arms_from_it() {
    let (_dir, store) = open();
    let p = project(&store, "/p/app");
    let owner = ProcessId::from_raw(1);
    let id = store
        .create(&at_timer(p, 1, "ping", 100_000))
        .expect("create");

    // Pause at t=30_000 with deadline 100_000: 70_000 remains. It leaves the armed set.
    assert!(store.pause(id, owner, 30_000).expect("pause"));
    assert!(store.armed().expect("armed").is_empty());
    assert_eq!(
        TimerRepo::list(&store, owner).expect("list")[0].status,
        TimerStatus::Paused
    );

    // Resume much later: the deadline is now plus the frozen remainder, not the original.
    assert!(store.resume(id, owner, 1_000_000).expect("resume"));
    let armed = store.armed().expect("armed");
    assert_eq!(armed.len(), 1);
    assert_eq!(armed[0].deadline_unix_millis, 1_070_000);
}

#[test]
fn pause_and_resume_act_only_for_the_owner() {
    let (_dir, store) = open();
    let p = project(&store, "/p/app");
    let owner = ProcessId::from_raw(1);
    let intruder = ProcessId::from_raw(2);
    let id = store
        .create(&at_timer(p, 1, "ping", 100_000))
        .expect("create");

    assert!(!store.pause(id, intruder, 1_000).expect("foreign pause"));
    assert!(store.pause(id, owner, 1_000).expect("owner pause"));
    assert!(!store.resume(id, intruder, 2_000).expect("foreign resume"));
    assert!(store.resume(id, owner, 2_000).expect("owner resume"));
}

#[test]
fn list_returns_only_the_owners_timers() {
    let (_dir, store) = open();
    let p = project(&store, "/p/app");
    store.create(&at_timer(p, 1, "mine", 10_000)).expect("a");
    store.create(&at_timer(p, 2, "theirs", 10_000)).expect("b");

    let mine = TimerRepo::list(&store, ProcessId::from_raw(1)).expect("list");
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].body, "mine");
}

#[test]
fn release_owner_drops_every_timer_of_one_owner() {
    let (_dir, store) = open();
    let p = project(&store, "/p/app");
    store.create(&at_timer(p, 1, "a", 10_000)).expect("a");
    store.create(&at_timer(p, 1, "b", 10_000)).expect("b");
    store.create(&at_timer(p, 2, "c", 10_000)).expect("c");

    assert_eq!(
        store
            .release_owner(ProcessId::from_raw(1))
            .expect("release"),
        2
    );
    assert_eq!(
        TimerRepo::list(&store, ProcessId::from_raw(2))
            .expect("list")
            .len(),
        1
    );
}

#[test]
fn clear_drops_every_timer() {
    let (_dir, store) = open();
    let p = project(&store, "/p/app");
    store.create(&at_timer(p, 1, "a", 10_000)).expect("a");
    store.create(&at_timer(p, 2, "b", 10_000)).expect("b");

    assert_eq!(store.clear().expect("clear"), 2);
    assert!(store.armed().expect("armed").is_empty());
}

#[test]
fn removing_a_project_cascades_its_timers() {
    let (_dir, store) = open();
    let p = project(&store, "/p/cascade");
    store
        .create(&at_timer(p, 1, "ping", 10_000))
        .expect("create");

    ProjectRepo::remove(&store, p).expect("remove project");
    assert!(
        store.armed().expect("armed").is_empty(),
        "timer rows must cascade-delete with their project"
    );
}

#[test]
fn a_timer_persists_across_a_reopen() {
    // The durable fact survives a restart (production clears it on launch via reconcile).
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("soloist.db");
    let (p, id) = {
        let store = SqliteStore::open(&db).expect("open");
        let p = project(&store, "/p/app");
        let id = store
            .create(&at_timer(p, 7, "ping", 90_000))
            .expect("create");
        (p, id)
    };
    let _ = p;

    let reopened = SqliteStore::open(&db).expect("reopen");
    let armed = reopened.armed().expect("armed");
    assert_eq!(armed.len(), 1);
    assert_eq!(armed[0].id, id);
}

#[test]
fn concurrent_claims_of_one_armed_timer_grant_exactly_one_winner() {
    // The race the atomic claim fixes: many scheduler passes (or a pass and a retry) try to fire the
    // same timer at once. Exactly one must claim it; every other must find it already gone.
    let (_dir, store) = open();
    let store = Arc::new(store);
    let p = project(&store, "/p/race");
    let id = store
        .create(&at_timer(p, 1, "ping", 10_000))
        .expect("create");
    const CLAIMANTS: usize = 16;

    let barrier = Arc::new(Barrier::new(CLAIMANTS));
    let claims: Vec<bool> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..CLAIMANTS)
            .map(|_| {
                let store = store.clone();
                let barrier = barrier.clone();
                scope.spawn(move || {
                    barrier.wait();
                    store.take_if_armed(id).expect("claim").is_some()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("thread"))
            .collect()
    });

    let winners = claims.iter().filter(|&&claimed| claimed).count();
    assert_eq!(winners, 1, "exactly one pass may claim a timer to fire it");
}

#[test]
fn list_in_project_returns_this_project_s_timers_armed_and_paused_ordered_by_id() {
    let (_dir, store) = open();
    let a = project(&store, "/p/a");
    let b = project(&store, "/p/b");
    // Two timers in project a (one of them paused) and one in project b.
    let armed = store.create(&at_timer(a, 1, "armed", 10_000)).expect("a1");
    let paused = store.create(&at_timer(a, 2, "paused", 10_000)).expect("a2");
    store.create(&at_timer(b, 3, "other", 10_000)).expect("b1");
    assert!(store
        .pause(paused, ProcessId::from_raw(2), 1_000)
        .expect("pause"));

    // Project a's timers only — both the armed and the paused one — ordered by id.
    let listed = store.list_in_project(a).expect("list_in_project");
    let ids: Vec<_> = listed.iter().map(|timer| timer.id).collect();
    assert_eq!(ids, vec![armed, paused], "both a's timers, ordered by id");
    assert_eq!(listed[0].status, TimerStatus::Armed);
    assert_eq!(listed[1].status, TimerStatus::Paused);
}
