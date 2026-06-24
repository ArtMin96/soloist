use serde_json::{json, Value};
use soloist_core::{KvEntry, KvRepo, ProjectId};

use crate::SqliteStore;

const P: ProjectId = ProjectId::from_raw(1);
const Q: ProjectId = ProjectId::from_raw(2);

fn store_with_project() -> SqliteStore {
    let store = SqliteStore::open_in_memory().expect("in-memory store");
    store
        .lock()
        .execute(
            "INSERT INTO projects (id, root, name) VALUES (?1, ?2, ?3)",
            (P.get() as i64, "/p", "P"),
        )
        .expect("seed project P");
    store
        .lock()
        .execute(
            "INSERT INTO projects (id, root, name) VALUES (?1, ?2, ?3)",
            (Q.get() as i64, "/q", "Q"),
        )
        .expect("seed project Q");
    store
}

#[test]
fn get_absent_key_returns_none() {
    let store = store_with_project();
    assert_eq!(store.get(P, "missing").unwrap(), None);
}

#[test]
fn set_and_get_round_trip_scalar() {
    let store = store_with_project();
    store.set(P, "n", &json!(42)).unwrap();
    assert_eq!(store.get(P, "n").unwrap(), Some(json!(42)));
}

#[test]
fn set_and_get_round_trip_object() {
    let store = store_with_project();
    let val: Value = json!({ "a": 1, "b": [true, null] });
    store.set(P, "obj", &val).unwrap();
    assert_eq!(store.get(P, "obj").unwrap(), Some(val));
}

#[test]
fn set_replaces_existing_value() {
    let store = store_with_project();
    store.set(P, "x", &json!("first")).unwrap();
    store.set(P, "x", &json!("second")).unwrap();
    assert_eq!(store.get(P, "x").unwrap(), Some(json!("second")));
}

#[test]
fn delete_returns_true_when_present() {
    let store = store_with_project();
    store.set(P, "x", &json!(1)).unwrap();
    assert!(store.delete(P, "x").unwrap());
    assert_eq!(store.get(P, "x").unwrap(), None);
}

#[test]
fn delete_returns_false_when_absent() {
    let store = store_with_project();
    assert!(!store.delete(P, "missing").unwrap());
}

#[test]
fn list_returns_entries_ordered_by_key() {
    let store = store_with_project();
    store.set(P, "b", &json!(2)).unwrap();
    store.set(P, "a", &json!(1)).unwrap();
    store.set(P, "c", &json!(3)).unwrap();
    let entries = store.list(P).unwrap();
    assert_eq!(
        entries,
        vec![
            KvEntry {
                key: "a".into(),
                value: json!(1)
            },
            KvEntry {
                key: "b".into(),
                value: json!(2)
            },
            KvEntry {
                key: "c".into(),
                value: json!(3)
            },
        ]
    );
}

#[test]
fn kv_is_project_scoped() {
    let store = store_with_project();
    store.set(P, "x", &json!("p")).unwrap();
    assert_eq!(store.get(Q, "x").unwrap(), None);
    assert!(store.list(Q).unwrap().is_empty());
}

#[test]
fn list_empty_project_returns_empty() {
    let store = store_with_project();
    assert!(store.list(P).unwrap().is_empty());
}
