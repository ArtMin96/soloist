use soloist_core::{ProjectId, ProjectSettings, SettingsRepo};
use tempfile::tempdir;

use crate::SqliteStore;

const P: ProjectId = ProjectId::from_raw(1);
const Q: ProjectId = ProjectId::from_raw(2);

/// A store with two projects seeded, so the `project_settings.project_id` foreign key is satisfied.
fn store_with_projects() -> SqliteStore {
    let store = SqliteStore::open_in_memory().expect("in-memory store");
    for (project, root, name) in [(P, "/p", "P"), (Q, "/q", "Q")] {
        store
            .lock()
            .execute(
                "INSERT INTO projects (id, root, name) VALUES (?1, ?2, ?3)",
                (project.get() as i64, root, name),
            )
            .expect("seed project");
    }
    store
}

/// Settings distinct from the defaults, so a round-trip proves the stored record (not the default)
/// came back.
fn non_default() -> ProjectSettings {
    ProjectSettings {
        auto_start_gate: true,
        auto_trust_command_changes: true,
        editor_override: Some("zed".into()),
        crash_exit_alerts: false,
        ..Default::default()
    }
}

#[test]
fn load_on_a_fresh_store_returns_none() {
    // Nothing stored yet, so the aggregate applies the documented defaults.
    let store = store_with_projects();
    assert_eq!(SettingsRepo::load(&store, &P).unwrap(), None);
}

#[test]
fn save_then_load_round_trips() {
    let store = store_with_projects();
    let settings = non_default();
    store.save(&P, &settings).unwrap();
    assert_eq!(store.load(&P).unwrap(), Some(settings));
}

#[test]
fn save_replaces_the_single_record_per_project() {
    // `project_id` is the primary key: a second save for the same project overwrites the first.
    let store = store_with_projects();
    store.save(&P, &ProjectSettings::default()).unwrap();
    store.save(&P, &non_default()).unwrap();

    assert_eq!(store.load(&P).unwrap(), Some(non_default()));
    let count: i64 = store
        .lock()
        .query_row(
            "SELECT COUNT(*) FROM project_settings WHERE project_id = ?1",
            [P.get() as i64],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "one project keeps exactly one settings record");
}

#[test]
fn settings_are_keyed_per_project() {
    let store = store_with_projects();
    store.save(&P, &non_default()).unwrap();

    assert_eq!(store.load(&P).unwrap(), Some(non_default()));
    assert_eq!(
        store.load(&Q).unwrap(),
        None,
        "a second project has no record of its own"
    );
}

#[test]
fn settings_survive_a_store_reopen() {
    // Per-project settings are durable: they persist across an app restart.
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("soloist.db");
    let settings = non_default();
    {
        let store = SqliteStore::open(&db).expect("open");
        store
            .lock()
            .execute(
                "INSERT INTO projects (id, root, name) VALUES (?1, ?2, ?3)",
                (P.get() as i64, "/p", "P"),
            )
            .expect("seed project");
        store.save(&P, &settings).unwrap();
    }

    let store = SqliteStore::open(&db).expect("reopen");
    assert_eq!(
        store.load(&P).unwrap(),
        Some(settings),
        "the per-project settings record survives the reopen"
    );
}

#[test]
fn removing_a_project_cascades_to_its_settings() {
    // The `project_id` foreign key cascades, so dropping a project drops its local settings.
    let store = store_with_projects();
    store.save(&P, &non_default()).unwrap();

    store
        .lock()
        .execute("DELETE FROM projects WHERE id = ?1", [P.get() as i64])
        .expect("delete project P");

    assert_eq!(
        store.load(&P).unwrap(),
        None,
        "the cascade removed the orphaned settings row"
    );
}
