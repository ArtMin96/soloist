use std::sync::Arc;

use super::*;
use crate::ids::ProjectId;
use crate::testing::{FakeScratchpadRepo, MockClock};

/// A representative Markdown body — a couple of headings over a line of prose.
fn body() -> String {
    "## Objective\nShip v1 to the .deb channel\n\n## Status\nsoak running".to_owned()
}

fn scratchpads() -> Scratchpads {
    Scratchpads::new(
        Arc::new(FakeScratchpadRepo::new()),
        Arc::new(MockClock::new()),
    )
}

const PROJECT: ProjectId = ProjectId::from_raw(1);

#[test]
fn write_creates_at_revision_one_then_read_renders_it() {
    let pads = scratchpads();
    let created = pads
        .write(PROJECT, "release-plan", body(), None)
        .expect("create succeeds");
    assert_eq!(created.revision, 1);
    assert_eq!(created.name, "release-plan");
    assert_eq!(created.body, body());
    // The rendering titles the body by the scratchpad's name (its handle), not embedded in the body.
    assert!(created.rendered.starts_with("# release-plan\n\n"));
    assert!(created.rendered.contains("## Objective"));

    let read = pads
        .read(PROJECT, "release-plan")
        .expect("read succeeds")
        .expect("the scratchpad exists");
    assert_eq!(read, created);
}

#[test]
fn a_blank_body_is_valid() {
    // A blank document is valid — only the name handle and the size cap are enforced.
    let pads = scratchpads();
    let created = pads
        .write(PROJECT, "empty", String::new(), None)
        .expect("a blank body writes");
    assert_eq!(created.body, "");
    assert_eq!(created.revision, 1);
}

#[test]
fn a_blank_name_is_rejected_before_it_persists() {
    let pads = scratchpads();
    assert!(matches!(
        pads.write(PROJECT, "   ", body(), None),
        Err(WriteError::Invalid(message)) if message.contains("name")
    ));
    assert!(
        pads.read(PROJECT, "   ").unwrap().is_none(),
        "a rejected write must not create the scratchpad"
    );
}

#[test]
fn a_body_over_the_byte_cap_is_rejected_but_one_at_the_cap_is_accepted() {
    let pads = scratchpads();
    assert!(matches!(
        pads.write(PROJECT, "big", "x".repeat(MAX_SCRATCHPAD_CONTENT_BYTES + 1), None),
        Err(WriteError::Invalid(message)) if message.contains("exceeds")
    ));
    assert!(
        pads.read(PROJECT, "big").unwrap().is_none(),
        "an over-cap write must not persist"
    );

    let at_cap = pads
        .write(
            PROJECT,
            "big",
            "x".repeat(MAX_SCRATCHPAD_CONTENT_BYTES),
            None,
        )
        .expect("a body exactly at the cap is accepted");
    assert_eq!(at_cap.body.len(), MAX_SCRATCHPAD_CONTENT_BYTES);
}

#[test]
fn write_at_the_current_revision_updates_and_bumps_it() {
    let pads = scratchpads();
    pads.write(PROJECT, "plan", body(), None).expect("create");

    let updated = pads
        .write(PROJECT, "plan", "## Status\ntagged".to_owned(), Some(1))
        .expect("update at the current revision succeeds");
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.body, "## Status\ntagged");
}

#[test]
fn a_stale_write_is_a_conflict_and_changes_nothing() {
    let pads = scratchpads();
    pads.write(PROJECT, "plan", body(), None).expect("create");
    pads.write(PROJECT, "plan", body(), Some(1))
        .expect("first update");

    // The document is now at revision 2; a writer still holding revision 1 is refused.
    let conflict = pads
        .write(PROJECT, "plan", body(), Some(1))
        .expect_err("a stale revision conflicts");
    assert!(matches!(
        conflict,
        WriteError::Conflict {
            expected: Some(1),
            actual: Some(2)
        }
    ));
    // The newer edit stands.
    assert_eq!(
        pads.read(PROJECT, "plan").unwrap().unwrap().revision,
        2,
        "the conflicting write must not have applied"
    );
}

#[test]
fn creating_over_an_existing_name_conflicts() {
    let pads = scratchpads();
    pads.write(PROJECT, "plan", body(), None).expect("create");
    let conflict = pads
        .write(PROJECT, "plan", body(), None)
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
fn updating_a_missing_scratchpad_conflicts_with_no_record() {
    let pads = scratchpads();
    let conflict = pads
        .write(PROJECT, "absent", body(), Some(3))
        .expect_err("updating a missing scratchpad conflicts");
    assert!(matches!(
        conflict,
        WriteError::Conflict {
            expected: Some(3),
            actual: None
        }
    ));
}

#[test]
fn a_summary_gist_is_the_first_non_heading_body_line() {
    let pads = scratchpads();
    pads.write(PROJECT, "plan", body(), None).expect("create");
    let summary = pads
        .list(PROJECT)
        .expect("list")
        .into_iter()
        .next()
        .expect("one summary");
    assert_eq!(summary.gist, "Ship v1 to the .deb channel");
}

#[test]
fn rename_moves_the_handle_and_keeps_the_durable_id() {
    let pads = scratchpads();
    let created = pads.write(PROJECT, "old", body(), None).expect("create");

    let renamed = pads.rename(PROJECT, "old", "new").expect("rename succeeds");
    assert_eq!(renamed.name, "new");
    assert_eq!(
        renamed.id, created.id,
        "the durable id is stable across a rename"
    );
    assert!(pads.read(PROJECT, "old").unwrap().is_none());
    assert!(pads.read(PROJECT, "new").unwrap().is_some());
}

#[test]
fn rename_reports_missing_and_taken() {
    let pads = scratchpads();
    pads.write(PROJECT, "a", body(), None).expect("create a");
    pads.write(PROJECT, "b", body(), None).expect("create b");

    assert!(matches!(
        pads.rename(PROJECT, "missing", "x"),
        Err(RenameError::NotFound)
    ));
    assert!(matches!(
        pads.rename(PROJECT, "a", "b"),
        Err(RenameError::NameTaken)
    ));
}

#[test]
fn tags_add_dedupe_remove_and_list_distinct() {
    let pads = scratchpads();
    pads.write(PROJECT, "a", body(), None).expect("create a");
    pads.write(PROJECT, "b", body(), None).expect("create b");

    let tagged = pads
        .add_tags(
            PROJECT,
            "a",
            &["release".into(), "release".into(), "p1".into()],
        )
        .expect("add succeeds")
        .expect("a exists");
    assert_eq!(tagged.tags, vec!["p1".to_string(), "release".to_string()]);

    pads.add_tags(PROJECT, "b", &["p1".into()]).expect("tag b");
    let distinct = pads.tags(PROJECT).expect("tags list");
    assert_eq!(distinct, vec!["p1".to_string(), "release".to_string()]);

    let untagged = pads
        .remove_tags(PROJECT, "a", &["release".into()])
        .expect("remove succeeds")
        .expect("a exists");
    assert_eq!(untagged.tags, vec!["p1".to_string()]);
}

#[test]
fn archive_is_a_flag_not_a_delete() {
    let pads = scratchpads();
    pads.write(PROJECT, "a", body(), None).expect("create");

    let archived = pads
        .set_archived(PROJECT, "a", true)
        .expect("archive succeeds")
        .expect("a exists");
    assert!(archived.archived);
    // Still present and readable.
    assert!(pads.read(PROJECT, "a").unwrap().is_some());

    let restored = pads
        .set_archived(PROJECT, "a", false)
        .expect("restore")
        .expect("a exists");
    assert!(!restored.archived);
}

#[test]
fn delete_removes_the_scratchpad() {
    let pads = scratchpads();
    pads.write(PROJECT, "a", body(), None).expect("create");
    assert!(pads.delete(PROJECT, "a").expect("delete"));
    assert!(!pads.delete(PROJECT, "a").expect("second delete is a no-op"));
    assert!(pads.read(PROJECT, "a").unwrap().is_none());
}

#[test]
fn list_is_scoped_to_the_project_and_ordered_by_name() {
    let pads = scratchpads();
    pads.write(PROJECT, "zebra", body(), None).expect("create");
    pads.write(PROJECT, "alpha", body(), None).expect("create");
    pads.write(ProjectId::from_raw(2), "other", body(), None)
        .expect("create in another project");

    let names: Vec<String> = pads
        .list(PROJECT)
        .expect("list")
        .into_iter()
        .map(|summary| summary.name)
        .collect();
    assert_eq!(names, vec!["alpha".to_string(), "zebra".to_string()]);
}

#[test]
fn transfer_moves_a_scratchpad_to_the_new_scope_keeping_its_identity() {
    const OTHER: ProjectId = ProjectId::from_raw(2);
    let pads = scratchpads();

    let written = pads
        .write(PROJECT, "release-plan", body(), None)
        .expect("write a scratchpad in the source project");
    pads.add_tags(PROJECT, "release-plan", &["v1".to_string()])
        .expect("tag it");
    let before = pads
        .read(PROJECT, "release-plan")
        .expect("read")
        .expect("exists before the move");

    let after = pads
        .transfer(PROJECT, "release-plan", OTHER)
        .expect("transfer succeeds")
        .scratchpad;

    // The durable id, name, tags, body, and revision survive the relocation.
    assert_eq!(after.id, written.id);
    assert_eq!(after.name, "release-plan");
    assert_eq!(after.tags, before.tags);
    assert_eq!(after.body, before.body);
    assert_eq!(after.revision, before.revision);

    // The scratchpad is now readable only from the new scope.
    assert_eq!(
        pads.read(OTHER, "release-plan")
            .expect("read from the new scope"),
        Some(after)
    );
    assert_eq!(
        pads.read(PROJECT, "release-plan")
            .expect("read from the old scope"),
        None,
        "the scratchpad no longer reads from the project it left"
    );
}

#[test]
fn transfer_refuses_when_the_target_name_is_taken() {
    const OTHER: ProjectId = ProjectId::from_raw(2);
    let pads = scratchpads();
    pads.write(PROJECT, "plan", body(), None).expect("source");
    pads.write(OTHER, "plan", body(), None)
        .expect("a same-named scratchpad already in the target");

    assert!(matches!(
        pads.transfer(PROJECT, "plan", OTHER),
        Err(RenameError::NameTaken)
    ));
    // The refusal leaves the source scratchpad in place.
    assert!(pads.read(PROJECT, "plan").expect("read").is_some());
}
