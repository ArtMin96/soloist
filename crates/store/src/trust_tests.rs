//! The trust repository's durable behaviour: what a grant records, what survives an upgrade, and
//! what revoking takes away.

use super::*;
use crate::SqliteStore;
use soloist_core::{content_hash, ProcessId, ProjectRepo};
use tempfile::tempdir;

fn project_with_trust(store: &SqliteStore, root: &str) -> ProjectId {
    store
        .upsert(std::path::Path::new(root), None, None)
        .expect("project for trust fk")
        .id
}

#[test]
fn trust_persists_across_reopen() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("soloist.db");
    let variant = content_hash(b"npm run dev|/app|");
    let project = {
        let store = SqliteStore::open(&db).expect("open");
        let project = project_with_trust(&store, "/projects/app");
        store
            .set_trusted(project, &variant, "npm run dev")
            .expect("trust");
        project
    };

    let reopened = SqliteStore::open(&db).expect("reopen");
    assert!(
        reopened.is_trusted(project, &variant).expect("query"),
        "trust must survive a restart"
    );
}

#[test]
fn revoke_and_scope_behave() {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let a = project_with_trust(&store, "/p/a");
    let b = project_with_trust(&store, "/p/b");
    let variant = content_hash(b"shared command");

    store
        .set_trusted(a, &variant, "shared command")
        .expect("trust a");
    assert!(store.is_trusted(a, &variant).expect("a trusted"));
    assert!(
        !store.is_trusted(b, &variant).expect("b untrusted"),
        "trust is per project"
    );

    store.revoke(a, &variant).expect("revoke");
    assert!(!store.is_trusted(a, &variant).expect("a revoked"));
}

#[test]
fn project_trust_is_per_project_and_survives_a_restart() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("soloist.db");
    let (trusted, untrusted) = {
        let store = SqliteStore::open(&db).expect("open");
        let trusted = project_with_trust(&store, "/p/trusted");
        let untrusted = project_with_trust(&store, "/p/untrusted");
        store.set_project_trusted(trusted).expect("trust project");
        (trusted, untrusted)
    };

    let reopened = SqliteStore::open(&db).expect("reopen");
    assert!(reopened.is_project_trusted(trusted).expect("query"));
    assert!(
        !reopened.is_project_trusted(untrusted).expect("query"),
        "authorising one project must not authorise another"
    );

    reopened.revoke_project(trusted).expect("revoke");
    assert!(!reopened.is_project_trusted(trusted).expect("query"));
}

#[test]
fn removing_a_project_cascades_its_project_trust() {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let project = project_with_trust(&store, "/p/cascade-project");
    store.set_project_trusted(project).expect("trust project");

    store.remove(project).expect("remove project");
    assert!(
        !store.is_project_trusted(project).expect("query"),
        "authorisation must cascade-delete with its project"
    );
}

#[test]
fn removing_a_project_cascades_its_trust() {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let project = project_with_trust(&store, "/p/cascade");
    let variant = content_hash(b"command");
    store
        .set_trusted(project, &variant, "command")
        .expect("trust");

    store.remove(project).expect("remove project");
    assert!(
        !store.is_trusted(project, &variant).expect("query"),
        "trust rows must cascade-delete with their project"
    );
}

#[test]
fn a_grant_records_its_requester_and_reason() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("soloist.db");
    let variant = content_hash(b"npm run build|web|");
    let requester = ProcessId::from_raw(42);
    let project = {
        let store = SqliteStore::open(&db).expect("open");
        let project = project_with_trust(&store, "/projects/asked");
        store
            .set_trusted_with_provenance(
                project,
                &variant,
                "npm run build",
                requester,
                "the release build",
                1_700,
            )
            .expect("grant with provenance");
        project
    };

    let reopened = SqliteStore::open(&db).expect("reopen");

    assert_eq!(
        reopened.list_grants(project).expect("list grants"),
        vec![TrustGrant {
            variant_hash: variant.to_hex(),
            command: Some("npm run build".into()),
            requested_by: Some(requester),
            reason: Some("the release build".into()),
            granted_at_unix_millis: Some(1_700),
        }],
        "a grant made on a process's behalf must say so, or the user cannot review what they \
         approved for someone else"
    );
    assert!(reopened.is_trusted(project, &variant).expect("query"));
}

#[test]
fn an_existing_grant_survives_the_migration_as_user_authored() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("soloist.db");
    let variant = content_hash(b"npm run dev|");
    // A database as a build before provenance left it: the trust table has only the two columns
    // that version knew about, and the recorded schema version says so.
    let project = {
        let conn = rusqlite::Connection::open(&db).expect("open raw");
        crate::migrate::migrate(&conn).expect("migrate to current");
        for column in [
            "command",
            "requested_by",
            "reason",
            "granted_at_unix_millis",
        ] {
            conn.execute_batch(&format!("ALTER TABLE trust DROP COLUMN {column};"))
                .expect("undo the provenance columns");
        }
        conn.execute(
            "INSERT INTO projects (root, name, icon, position)
             VALUES ('/projects/legacy', NULL, NULL, 0)",
            [],
        )
        .expect("seed a project");
        let project: i64 = conn
            .query_row("SELECT id FROM projects", [], |row| row.get(0))
            .expect("read the project id");
        conn.execute(
            "INSERT INTO trust (project_id, variant_hash) VALUES (?1, ?2)",
            (project, variant.to_hex()),
        )
        .expect("seed a grant the user authored");
        conn.pragma_update(None, "user_version", 21i64)
            .expect("record the older schema version");
        ProjectId::from_raw(project as u64)
    };

    let upgraded = SqliteStore::open(&db).expect("reopen, which migrates");

    assert!(
        upgraded.is_trusted(project, &variant).expect("query"),
        "an upgrade must not cost the user a grant they already made"
    );
    assert_eq!(
        upgraded.list_grants(project).expect("list grants"),
        vec![TrustGrant {
            variant_hash: variant.to_hex(),
            command: None,
            requested_by: None,
            reason: None,
            granted_at_unix_millis: None,
        }],
        "a grant that predates provenance is one the user authored, and must read back that way"
    );
}

#[test]
fn revoking_a_grant_makes_the_variant_untrusted_again() {
    let dir = tempdir().expect("temp dir");
    let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
    let project = project_with_trust(&store, "/projects/revoked");
    let variant = content_hash(b"npm run build|web|");
    store
        .set_trusted_with_provenance(
            project,
            &variant,
            "npm run build",
            ProcessId::from_raw(7),
            "because",
            1,
        )
        .expect("grant");

    let listed = store.list_grants(project).expect("list grants");
    assert_eq!(listed.len(), 1);
    let key = Hash::from_hex(&listed[0].variant_hash).expect("parse the listed key");
    store.revoke(project, &key).expect("revoke");

    assert!(store.list_grants(project).expect("list grants").is_empty());
    assert!(!store.is_trusted(project, &variant).expect("query"));
}
