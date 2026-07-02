use soloist_core::{FeedbackEntry, FeedbackRepo};
use tempfile::tempdir;

use crate::SqliteStore;

#[test]
fn append_then_list_round_trips_oldest_first() {
    let store = SqliteStore::open_in_memory().expect("in-memory store");

    let first = store.append("first note", 1_000).expect("append first");
    let second = store.append("second note", 2_000).expect("append second");

    assert_ne!(first.id, second.id);
    assert_eq!(
        store.list().expect("list"),
        vec![
            FeedbackEntry {
                id: first.id,
                message: "first note".into(),
                submitted_unix_millis: 1_000,
            },
            FeedbackEntry {
                id: second.id,
                message: "second note".into(),
                submitted_unix_millis: 2_000,
            },
        ]
    );
}

#[test]
fn feedback_survives_a_store_reopen() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("soloist.db");

    let entry = {
        let store = SqliteStore::open(&path).expect("open store");
        store.append("keep me", 42).expect("append")
    };

    let reopened = SqliteStore::open(&path).expect("reopen store");
    assert_eq!(reopened.list().expect("list"), vec![entry]);
}
