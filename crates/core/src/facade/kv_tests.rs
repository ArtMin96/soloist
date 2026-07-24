use crate::facade::Facade;
use crate::PeerCredentials;
use std::path::Path;
use std::sync::Arc;

use serde_json::json;

use super::*;
use crate::composition::CorePorts;
use crate::coordination::MAX_KV_VALUE_BYTES;
use crate::ids::SessionId;
use crate::ports::{ProjectRepo, TokioClock};
use crate::testing::{FakeKvRepo, FakeProjectRepo, FakeSpawner, FakeTrustRepo};

fn facade_with_kv(projects: Arc<FakeProjectRepo>) -> Facade {
    Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .kv_repo(Arc::new(FakeKvRepo::new()))
        .build(),
    )
}

fn scoped_facade() -> (Facade, SessionId) {
    let projects = Arc::new(FakeProjectRepo::new());
    projects
        .upsert(Path::new("/tmp/soloist-kv-test"), Some("p"), None)
        .expect("seed one project");
    let facade = facade_with_kv(projects);
    let session = facade.open_session(PeerCredentials::unauthenticated());
    (facade, session)
}

#[test]
fn kv_set_with_no_project_scope_is_refused() {
    let facade = facade_with_kv(Arc::new(FakeProjectRepo::new()));
    let session = facade.open_session(PeerCredentials::unauthenticated());
    // Two or more projects → no automatic scope
    assert!(matches!(
        facade.scoped(session).kv_set("k".into(), json!(1)),
        Err(CoordinationError::NoProjectScope)
    ));
}

#[test]
fn kv_get_returns_none_for_absent_key() {
    let (facade, session) = scoped_facade();
    assert_eq!(
        facade.scoped(session).kv_get("absent".into()).unwrap(),
        None
    );
}

#[test]
fn kv_set_and_get_round_trip() {
    let (facade, session) = scoped_facade();
    facade
        .scoped(session)
        .kv_set("x".into(), json!({"n": 1}))
        .unwrap();
    assert_eq!(
        facade.scoped(session).kv_get("x".into()).unwrap(),
        Some(json!({"n": 1}))
    );
}

#[test]
fn kv_delete_returns_true_when_present() {
    let (facade, session) = scoped_facade();
    facade
        .scoped(session)
        .kv_set("x".into(), json!(true))
        .unwrap();
    assert!(facade.scoped(session).kv_delete("x".into()).unwrap());
    assert_eq!(facade.scoped(session).kv_get("x".into()).unwrap(), None);
}

#[test]
fn kv_delete_returns_false_when_absent() {
    let (facade, session) = scoped_facade();
    assert!(!facade.scoped(session).kv_delete("missing".into()).unwrap());
}

#[test]
fn kv_set_rejects_a_value_over_the_byte_cap_and_writes_nothing() {
    let (facade, session) = scoped_facade();
    // A JSON string of the cap length serializes to cap + 2 bytes (the quotes) — over the limit.
    let oversized = json!("x".repeat(MAX_KV_VALUE_BYTES));
    assert!(matches!(
        facade.scoped(session).kv_set("k".into(), oversized),
        Err(CoordinationError::PayloadTooLarge { .. })
    ));
    assert_eq!(
        facade.scoped(session).kv_get("k".into()).unwrap(),
        None,
        "a rejected write must persist nothing"
    );
}

#[test]
fn kv_set_accepts_a_value_exactly_at_the_byte_cap() {
    let (facade, session) = scoped_facade();
    // A JSON string of (cap - 2) chars serializes to exactly cap bytes with its quotes.
    let at_cap = json!("x".repeat(MAX_KV_VALUE_BYTES - 2));
    assert_eq!(at_cap.to_string().len(), MAX_KV_VALUE_BYTES);
    facade
        .scoped(session)
        .kv_set("k".into(), at_cap.clone())
        .unwrap();
    assert_eq!(
        facade.scoped(session).kv_get("k".into()).unwrap(),
        Some(at_cap)
    );
}

#[test]
fn kv_list_returns_entries_ordered_by_key() {
    let (facade, session) = scoped_facade();
    facade.scoped(session).kv_set("b".into(), json!(2)).unwrap();
    facade.scoped(session).kv_set("a".into(), json!(1)).unwrap();
    let entries = facade.scoped(session).kv_list().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "a");
    assert_eq!(entries[1].key, "b");
}
