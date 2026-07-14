use std::sync::Arc;

use serde_json::json;

use super::Kv;
use crate::coordination::KvEntry;
use crate::ids::ProjectId;
use crate::testing::FakeKvRepo;

const P: ProjectId = ProjectId::from_raw(1);
const Q: ProjectId = ProjectId::from_raw(2);

fn kv() -> Kv {
    Kv::new(Arc::new(FakeKvRepo::new()))
}

#[test]
fn get_absent_key_returns_none() {
    assert_eq!(kv().get(P, "missing").unwrap(), None);
}

#[test]
fn set_and_get_round_trip() {
    let kv = kv();
    kv.set(P, "x", &json!(42)).unwrap();
    assert_eq!(kv.get(P, "x").unwrap(), Some(json!(42)));
}

#[test]
fn set_replaces_existing_value() {
    let kv = kv();
    kv.set(P, "x", &json!("first")).unwrap();
    kv.set(P, "x", &json!("second")).unwrap();
    assert_eq!(kv.get(P, "x").unwrap(), Some(json!("second")));
}

#[test]
fn delete_returns_true_when_present() {
    let kv = kv();
    kv.set(P, "x", &json!(1)).unwrap();
    assert!(kv.delete(P, "x").unwrap());
    assert_eq!(kv.get(P, "x").unwrap(), None);
}

#[test]
fn delete_returns_false_when_absent() {
    assert!(!kv().delete(P, "missing").unwrap());
}

#[test]
fn list_returns_entries_ordered_by_key() {
    let kv = kv();
    kv.set(P, "b", &json!(2)).unwrap();
    kv.set(P, "a", &json!(1)).unwrap();
    kv.set(P, "c", &json!(3)).unwrap();
    let entries = kv.list(P).unwrap();
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
    let kv = kv();
    kv.set(P, "x", &json!("p")).unwrap();
    assert_eq!(kv.get(Q, "x").unwrap(), None);
    assert!(kv.list(Q).unwrap().is_empty());
}
