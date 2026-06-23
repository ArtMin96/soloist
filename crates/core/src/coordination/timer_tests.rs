use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;

use super::*;
use crate::ids::{ProcessId, ProjectId};
use crate::ports::Clock;
use crate::testing::{FakeTimerRepo, MockClock};

const PROJECT: ProjectId = ProjectId::from_raw(1);
const OWNER: ProcessId = ProcessId::from_raw(7);

/// The aggregate's policy bounds, mirrored here so the assertions read in real units.
const HOUR_MS: u64 = 60 * 60 * 1000;
const DAY_MS: u64 = 24 * HOUR_MS;

/// Builds a timer aggregate over an in-memory repo and a mock clock, returning all three so a test
/// can advance time and inspect the store.
fn timers() -> (Timers, MockClock, Arc<FakeTimerRepo>) {
    let repo = Arc::new(FakeTimerRepo::new());
    let clock = MockClock::new();
    (
        Timers::new(
            repo.clone(),
            Arc::new(clock.clone()),
            Arc::new(Notify::new()),
        ),
        clock,
        repo,
    )
}

#[test]
fn set_arms_an_at_timer_for_now_plus_the_delay() {
    let (timers, clock, _repo) = timers();
    let now = clock.now_unix_millis();

    let view = timers
        .set(PROJECT, OWNER, "ping".into(), Some(Duration::from_secs(30)))
        .expect("set");

    assert_eq!(view.fire, FireCond::At);
    assert_eq!(view.status, TimerStatus::Armed);
    assert_eq!(view.body, "ping");
    assert_eq!(view.deadline_unix_millis, now + 30_000);
}

#[test]
fn a_timer_with_no_delay_is_due_at_once() {
    let (timers, clock, _repo) = timers();
    let now = clock.now_unix_millis();

    let view = timers.set(PROJECT, OWNER, "now".into(), None).expect("set");

    assert_eq!(view.deadline_unix_millis, now);
}

#[test]
fn a_delay_above_the_ceiling_is_clamped() {
    let (timers, clock, _repo) = timers();
    let now = clock.now_unix_millis();

    let view = timers
        .set(
            PROJECT,
            OWNER,
            "later".into(),
            Some(Duration::from_secs(7 * 24 * 60 * 60)),
        )
        .expect("set");

    assert_eq!(
        view.deadline_unix_millis,
        now + DAY_MS,
        "a week-long delay is clamped to the 24h ceiling"
    );
}

#[test]
fn set_when_idle_records_the_watched_set_and_backstop() {
    let (timers, clock, _repo) = timers();
    let now = clock.now_unix_millis();
    let watched = vec![ProcessId::from_raw(2), ProcessId::from_raw(3)];

    let view = timers
        .set_when_idle(
            PROJECT,
            OWNER,
            "go".into(),
            watched.clone(),
            IdleMode::All,
            Some(Duration::from_secs(120)),
        )
        .expect("set");

    assert_eq!(view.fire, FireCond::WhenIdleAll { watched });
    assert_eq!(view.deadline_unix_millis, now + 120_000);
}

#[test]
fn the_idle_backstop_defaults_and_clamps() {
    let (timers, clock, _repo) = timers();
    let now = clock.now_unix_millis();

    // Omitted → the default one-hour backstop.
    let default = timers
        .set_when_idle(
            PROJECT,
            OWNER,
            "x".into(),
            vec![ProcessId::from_raw(2)],
            IdleMode::Any,
            None,
        )
        .expect("set");
    assert_eq!(default.deadline_unix_millis, now + HOUR_MS);

    // Above the ceiling → clamped to 24h.
    let clamped = timers
        .set_when_idle(
            PROJECT,
            OWNER,
            "y".into(),
            vec![ProcessId::from_raw(2)],
            IdleMode::Any,
            Some(Duration::from_secs(48 * 60 * 60)),
        )
        .expect("set");
    assert_eq!(clamped.deadline_unix_millis, now + DAY_MS);
}

#[test]
fn list_returns_only_the_owners_timers() {
    let (timers, _clock, _repo) = timers();
    let other = ProcessId::from_raw(9);
    timers
        .set(PROJECT, OWNER, "mine".into(), Some(Duration::from_secs(10)))
        .expect("set");
    timers
        .set(
            PROJECT,
            other,
            "theirs".into(),
            Some(Duration::from_secs(10)),
        )
        .expect("set");

    let mine = timers.list(OWNER).expect("list");

    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].body, "mine");
}

#[test]
fn cancel_removes_only_for_the_owner() {
    let (timers, _clock, _repo) = timers();
    let view = timers
        .set(PROJECT, OWNER, "ping".into(), Some(Duration::from_secs(30)))
        .expect("set");

    // A non-owner cannot cancel it.
    assert!(!timers
        .cancel(view.id, ProcessId::from_raw(99))
        .expect("foreign cancel"));
    assert_eq!(timers.list(OWNER).expect("list").len(), 1);

    // The owner cancels it.
    assert!(timers.cancel(view.id, OWNER).expect("owner cancel"));
    assert!(timers.list(OWNER).expect("list").is_empty());
}

#[test]
fn pause_freezes_the_remaining_time_and_resume_re_arms_from_it() {
    let (timers, clock, repo) = timers();
    let view = timers
        .set(
            PROJECT,
            OWNER,
            "ping".into(),
            Some(Duration::from_secs(100)),
        )
        .expect("set");

    // Pause 30s in: 70s remain. A paused timer is excluded from the armed set the scheduler reads.
    clock.advance(Duration::from_secs(30));
    assert!(timers.pause(view.id, OWNER).expect("pause"));
    assert!(repo.armed().expect("armed").is_empty());
    assert_eq!(
        timers.list(OWNER).expect("list")[0].status,
        TimerStatus::Paused
    );

    // Resume much later: the deadline is now plus the 70s that remained, not the original deadline.
    clock.advance(Duration::from_secs(1000));
    let resumed_at = clock.now_unix_millis();
    assert!(timers.resume(view.id, OWNER).expect("resume"));
    let armed = repo.armed().expect("armed");
    assert_eq!(armed.len(), 1);
    assert_eq!(armed[0].deadline_unix_millis, resumed_at + 70_000);
}

#[test]
fn pause_and_resume_act_only_for_the_owner() {
    let (timers, _clock, _repo) = timers();
    let view = timers
        .set(
            PROJECT,
            OWNER,
            "ping".into(),
            Some(Duration::from_secs(100)),
        )
        .expect("set");
    let intruder = ProcessId::from_raw(99);

    assert!(!timers.pause(view.id, intruder).expect("foreign pause"));
    assert!(timers.pause(view.id, OWNER).expect("owner pause"));
    assert!(!timers.resume(view.id, intruder).expect("foreign resume"));
    assert!(timers.resume(view.id, OWNER).expect("owner resume"));
}

#[test]
fn reconcile_clears_every_timer() {
    let (timers, _clock, _repo) = timers();
    timers
        .set(PROJECT, OWNER, "a".into(), Some(Duration::from_secs(10)))
        .expect("set");
    timers
        .set(PROJECT, OWNER, "b".into(), Some(Duration::from_secs(10)))
        .expect("set");

    let cleared = timers.reconcile().expect("reconcile");

    assert_eq!(cleared, 2);
    assert!(timers.list(OWNER).expect("list").is_empty());
}
