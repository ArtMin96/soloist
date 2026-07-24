use std::sync::Arc;

use super::*;
use crate::ids::ProjectId;
use crate::testing::{FakeDiagramRepo, MockClock};

/// A representative Mermaid source — a diagram-kind directive over a couple of edges.
fn source() -> String {
    "flowchart TD\n  A[Start] --> B{Ready?}\n  B -->|yes| C[Ship]".to_owned()
}

fn diagrams() -> Diagrams {
    Diagrams::new(Arc::new(FakeDiagramRepo::new()), Arc::new(MockClock::new()))
}

const PROJECT: ProjectId = ProjectId::from_raw(1);

#[test]
fn write_creates_at_revision_one_then_read_round_trips_the_source() {
    let diagrams = diagrams();
    let created = diagrams
        .write(PROJECT, "flow", source(), None)
        .expect("create succeeds");
    assert_eq!(created.revision, 1);
    assert_eq!(created.name, "flow");
    // The Mermaid source is stored and returned verbatim — the core never renders it.
    assert_eq!(created.source, source());

    let read = diagrams
        .read(PROJECT, "flow")
        .expect("read succeeds")
        .expect("the diagram exists");
    assert_eq!(read, created);
}

#[test]
fn a_blank_source_is_valid() {
    // A blank document is valid — only the name handle and the size cap are enforced.
    let diagrams = diagrams();
    let created = diagrams
        .write(PROJECT, "empty", String::new(), None)
        .expect("a blank source writes");
    assert_eq!(created.source, "");
    assert_eq!(created.revision, 1);
}

#[test]
fn a_blank_name_is_rejected_before_it_persists() {
    let diagrams = diagrams();
    assert!(matches!(
        diagrams.write(PROJECT, "   ", source(), None),
        Err(WriteError::Invalid(message)) if message.contains("name")
    ));
    assert!(
        diagrams.read(PROJECT, "   ").unwrap().is_none(),
        "a rejected write must not create the diagram"
    );
}

#[test]
fn a_source_over_the_byte_cap_is_rejected_but_one_at_the_cap_is_accepted() {
    let diagrams = diagrams();
    assert!(matches!(
        diagrams.write(PROJECT, "big", "x".repeat(MAX_DIAGRAM_SOURCE_BYTES + 1), None),
        Err(WriteError::Invalid(message)) if message.contains("exceeds")
    ));
    assert!(
        diagrams.read(PROJECT, "big").unwrap().is_none(),
        "an over-cap write must not persist"
    );

    let at_cap = diagrams
        .write(PROJECT, "big", "x".repeat(MAX_DIAGRAM_SOURCE_BYTES), None)
        .expect("a source exactly at the cap is accepted");
    assert_eq!(at_cap.source.len(), MAX_DIAGRAM_SOURCE_BYTES);
}

#[test]
fn write_at_the_current_revision_updates_and_bumps_it() {
    let diagrams = diagrams();
    diagrams
        .write(PROJECT, "flow", source(), None)
        .expect("create");

    let updated = diagrams
        .write(PROJECT, "flow", "graph LR\n  A --> B".to_owned(), Some(1))
        .expect("update at the current revision succeeds");
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.source, "graph LR\n  A --> B");
}

#[test]
fn a_stale_write_is_a_conflict_and_changes_nothing() {
    let diagrams = diagrams();
    diagrams
        .write(PROJECT, "flow", source(), None)
        .expect("create");
    diagrams
        .write(PROJECT, "flow", source(), Some(1))
        .expect("first update");

    // The document is now at revision 2; a writer still holding revision 1 is refused.
    let conflict = diagrams
        .write(PROJECT, "flow", source(), Some(1))
        .expect_err("a stale revision conflicts");
    assert!(matches!(
        conflict,
        WriteError::Conflict {
            expected: Some(1),
            actual: Some(2)
        }
    ));
    assert_eq!(
        diagrams.read(PROJECT, "flow").unwrap().unwrap().revision,
        2,
        "the conflicting write must not have applied"
    );
}

#[test]
fn creating_over_an_existing_name_conflicts() {
    let diagrams = diagrams();
    diagrams
        .write(PROJECT, "flow", source(), None)
        .expect("create");
    let conflict = diagrams
        .write(PROJECT, "flow", source(), None)
        .expect_err("a second create conflicts");
    assert!(matches!(
        conflict,
        WriteError::Conflict {
            expected: None,
            actual: Some(1)
        }
    ));
}

#[test]
fn updating_a_missing_diagram_conflicts_with_no_record() {
    let diagrams = diagrams();
    let conflict = diagrams
        .write(PROJECT, "absent", source(), Some(3))
        .expect_err("updating a missing diagram conflicts");
    assert!(matches!(
        conflict,
        WriteError::Conflict {
            expected: Some(3),
            actual: None
        }
    ));
}

#[test]
fn a_summary_gist_is_the_first_non_blank_source_line() {
    let diagrams = diagrams();
    // A leading blank line is skipped; the first non-blank line (the diagram directive) is the gist.
    diagrams
        .write(PROJECT, "flow", format!("\n{}", source()), None)
        .expect("create");
    let summary = diagrams
        .list(PROJECT)
        .expect("list")
        .into_iter()
        .next()
        .expect("one summary");
    assert_eq!(summary.gist, "flowchart TD");
}

#[test]
fn contains_answers_membership_per_project_not_per_id() {
    let diagrams = diagrams();
    let mine = ProjectId::from_raw(1);
    let theirs = ProjectId::from_raw(2);
    let created = diagrams
        .write(theirs, "flow", source(), None)
        .expect("create in the other project");

    assert!(diagrams.contains(theirs, created.id).expect("own project"));
    assert!(
        !diagrams.contains(mine, created.id).expect("other project"),
        "a real row must not count as a member of a project that does not own it"
    );
}

#[test]
fn rename_moves_the_handle_and_keeps_the_durable_id() {
    let diagrams = diagrams();
    let created = diagrams
        .write(PROJECT, "old", source(), None)
        .expect("create");

    let renamed = diagrams
        .rename(PROJECT, "old", "new")
        .expect("rename succeeds");
    assert_eq!(renamed.name, "new");
    assert_eq!(
        renamed.id, created.id,
        "the durable id is stable across a rename"
    );
    assert!(diagrams.read(PROJECT, "old").unwrap().is_none());
    assert!(diagrams.read(PROJECT, "new").unwrap().is_some());
}

#[test]
fn rename_reports_missing_and_taken() {
    let diagrams = diagrams();
    diagrams
        .write(PROJECT, "a", source(), None)
        .expect("create a");
    diagrams
        .write(PROJECT, "b", source(), None)
        .expect("create b");

    assert!(matches!(
        diagrams.rename(PROJECT, "missing", "x"),
        Err(RenameError::NotFound)
    ));
    assert!(matches!(
        diagrams.rename(PROJECT, "a", "b"),
        Err(RenameError::NameTaken)
    ));
}

#[test]
fn tags_add_dedupe_remove_and_list_distinct() {
    let diagrams = diagrams();
    diagrams
        .write(PROJECT, "a", source(), None)
        .expect("create a");
    diagrams
        .write(PROJECT, "b", source(), None)
        .expect("create b");

    let tagged = diagrams
        .add_tags(PROJECT, "a", &["arch".into(), "arch".into(), "v1".into()])
        .expect("add succeeds")
        .expect("a exists");
    assert_eq!(tagged.tags, vec!["arch".to_string(), "v1".to_string()]);

    diagrams
        .add_tags(PROJECT, "b", &["v1".into()])
        .expect("tag b");
    let distinct = diagrams.tags(PROJECT).expect("tags list");
    assert_eq!(distinct, vec!["arch".to_string(), "v1".to_string()]);

    let untagged = diagrams
        .remove_tags(PROJECT, "a", &["arch".into()])
        .expect("remove succeeds")
        .expect("a exists");
    assert_eq!(untagged.tags, vec!["v1".to_string()]);
}

#[test]
fn archive_is_a_flag_not_a_delete() {
    let diagrams = diagrams();
    diagrams
        .write(PROJECT, "a", source(), None)
        .expect("create");

    let archived = diagrams
        .set_archived(PROJECT, "a", true)
        .expect("archive succeeds")
        .expect("a exists");
    assert!(archived.archived);
    // Still present and readable.
    assert!(diagrams.read(PROJECT, "a").unwrap().is_some());

    let restored = diagrams
        .set_archived(PROJECT, "a", false)
        .expect("restore")
        .expect("a exists");
    assert!(!restored.archived);
}

#[test]
fn delete_removes_the_diagram() {
    let diagrams = diagrams();
    diagrams
        .write(PROJECT, "a", source(), None)
        .expect("create");
    assert!(diagrams.delete(PROJECT, "a").expect("delete"));
    assert!(!diagrams
        .delete(PROJECT, "a")
        .expect("second delete is a no-op"));
    assert!(diagrams.read(PROJECT, "a").unwrap().is_none());
}

#[test]
fn list_is_scoped_to_the_project_and_ordered_by_name() {
    let diagrams = diagrams();
    diagrams
        .write(PROJECT, "zebra", source(), None)
        .expect("create");
    diagrams
        .write(PROJECT, "alpha", source(), None)
        .expect("create");
    diagrams
        .write(ProjectId::from_raw(2), "other", source(), None)
        .expect("create in another project");

    let names: Vec<String> = diagrams
        .list(PROJECT)
        .expect("list")
        .into_iter()
        .map(|summary| summary.name)
        .collect();
    assert_eq!(names, vec!["alpha".to_string(), "zebra".to_string()]);
}
