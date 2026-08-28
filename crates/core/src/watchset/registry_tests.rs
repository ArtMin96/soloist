//! Behavioural tests for the held-path map and the budget it spends against, kept out of the
//! implementation file. A real [`FakeFileWatcher`] session stands in for the OS, so what was
//! actually registered through it is the observable outcome rather than the map's own bookkeeping.

use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::filewatch::{FileChange, FileWatcher};
use crate::testing::FakeFileWatcher;

use super::*;

/// A capacity small enough that a test can fill the whole budget by hand.
const CAPACITY: usize = 8;

/// The path a test registers as its `index`th watch.
fn path(index: usize) -> PathBuf {
    PathBuf::from(format!("/project/d{index}"))
}

/// A live session over `watcher`, holding the change receiver open for as long as the session
/// itself — nothing here sends a change, but a closed receiver is not a state the real backend
/// ever hands its caller.
fn session(watcher: &FakeFileWatcher) -> (Arc<dyn WatchSession>, mpsc::Receiver<FileChange>) {
    let (changes, receiver) = mpsc::channel(1);
    let session = watcher
        .open(changes, Arc::new(AtomicU64::new(0)))
        .expect("the fake opens a session");
    (session, receiver)
}

#[test]
fn a_registration_past_the_whole_budget_is_refused_without_asking_the_os() {
    let watcher = FakeFileWatcher::new().with_capacity(CAPACITY);
    let (session, _changes) = session(&watcher);
    let mut registrations = Registrations::new(Some(CAPACITY));
    let project = ProjectId::next();
    // One open project's share is the whole budget, so filling it fills the app's total.
    let total = registrations.share(1);

    for index in 0..total {
        registrations
            .register(&path(index), project, false, session.as_ref())
            .expect("a registration within the budget");
    }

    assert_eq!(
        registrations.register(&path(total), project, false, session.as_ref()),
        Err(WatchError::ShareExhausted),
    );
    assert_eq!(
        watcher.registered().len(),
        total,
        "the OS must never be asked for a watch the budget cannot pay for",
    );
}

// The two ways a watch is refused for want of budget are different conditions with different
// remedies, so the one the OS reports and the one Soloist imposes on itself must not arrive as the
// same error: a share Soloist spent says nothing about how much of the system's limit is left, and
// only the system's own limit is raised by raising the system's setting.
#[test]
fn an_os_refusal_is_reported_apart_from_a_spent_share() {
    let watcher = FakeFileWatcher::new().with_capacity(CAPACITY);
    let (session, _changes) = session(&watcher);
    let mut registrations = Registrations::new(Some(CAPACITY));
    let project = ProjectId::next();
    watcher.refuse(path(0));

    assert_eq!(
        registrations.register(&path(0), project, false, session.as_ref()),
        Err(WatchError::BudgetExhausted),
        "a refusal the OS gave is the system's limit, with budget to spare",
    );
}

#[test]
fn releasing_a_path_returns_its_budget_to_the_next_registration() {
    let watcher = FakeFileWatcher::new().with_capacity(CAPACITY);
    let (session, _changes) = session(&watcher);
    let mut registrations = Registrations::new(Some(CAPACITY));
    let project = ProjectId::next();
    let total = registrations.share(1);
    for index in 0..total {
        registrations
            .register(&path(index), project, false, session.as_ref())
            .expect("a registration within the budget");
    }

    registrations.release(&path(0), project, session.as_ref());
    registrations
        .register(&path(total), project, false, session.as_ref())
        .expect("the unit the release returned");

    assert!(watcher.registered().contains(&path(total)));
    assert!(!watcher.registered().contains(&path(0)));
}
