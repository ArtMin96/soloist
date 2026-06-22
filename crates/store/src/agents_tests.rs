use super::*;
use crate::SqliteStore;
use tempfile::tempdir;

#[test]
fn list_returns_the_seeded_builtin_providers_in_order() {
    let store = SqliteStore::open_in_memory().expect("open");
    assert_eq!(
        store.list().expect("list"),
        AgentTool::builtin_defaults(),
        "a fresh store lists exactly the seeded built-in providers, in order"
    );
}

#[test]
fn agent_tools_persist_across_reopen_without_reseeding() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("soloist.db");
    let count = {
        let store = SqliteStore::open(&db).expect("open");
        store.list().expect("list").len()
    };
    assert_eq!(count, AgentTool::builtin_defaults().len());

    // Reopening re-runs migrate (a no-op at the current version), so the seed is not
    // duplicated — the registry is stable across restarts.
    let reopened = SqliteStore::open(&db).expect("reopen");
    assert_eq!(
        reopened.list().expect("list").len(),
        AgentTool::builtin_defaults().len(),
        "reopening must not re-seed the built-in providers"
    );
}
