use std::sync::Arc;

use super::*;
use crate::ids::ProjectId;
use crate::testing::FakeScratchpadRepo;

/// A well-formed disciplined document the validation accepts.
fn doc() -> ScratchpadDoc {
    ScratchpadDoc {
        objective: "Ship v1 to the .deb channel".into(),
        context: "RC is cut; soak is pending".into(),
        plan: vec!["Cut RC".into(), "Soak 6h".into(), "Tag + bundle".into()],
        acceptance_criteria: vec!["soak green".into(), "installs on 22.04".into()],
        risks: vec!["glibc mismatch on 20.04".into()],
        status: "soak running".into(),
        notes: None,
    }
}

fn scratchpads() -> Scratchpads {
    Scratchpads::new(Arc::new(FakeScratchpadRepo::new()))
}

const PROJECT: ProjectId = ProjectId::from_raw(1);

#[test]
fn validate_accepts_a_well_formed_document() {
    assert!(doc().validate().is_ok());
}

#[test]
fn validate_names_every_missing_section_at_once() {
    let empty = ScratchpadDoc {
        objective: "  ".into(),
        context: String::new(),
        plan: Vec::new(),
        acceptance_criteria: Vec::new(),
        risks: Vec::new(),
        status: String::new(),
        notes: None,
    };
    let message = empty.validate().expect_err("a blank document is rejected");
    for fragment in [
        "objective",
        "context",
        "status",
        "plan",
        "acceptance_criteria",
        "risks",
    ] {
        assert!(
            message.contains(fragment),
            "the message should name `{fragment}`: {message}"
        );
    }
}

#[test]
fn validate_rejects_a_blank_list_entry() {
    let mut doc = doc();
    doc.plan = vec!["Cut RC".into(), "   ".into()];
    assert!(doc
        .validate()
        .expect_err("a blank plan step is rejected")
        .contains("plan steps must not be blank"));
}

#[test]
fn render_lays_out_the_canonical_sections_titled_by_name() {
    let rendered = doc().render("release-plan");
    assert!(rendered.starts_with("# release-plan"));
    for heading in [
        "## Objective",
        "## Context",
        "## Plan",
        "## Acceptance criteria",
        "## Risks",
        "## Status",
    ] {
        assert!(rendered.contains(heading), "missing {heading}:\n{rendered}");
    }
    // The plan is an ordered (numbered) path; criteria are checkbox items.
    assert!(rendered.contains("1. Cut RC"));
    assert!(rendered.contains("- [ ] soak green"));
    // Notes is omitted when absent.
    assert!(!rendered.contains("## Notes"));
}

#[test]
fn render_includes_notes_only_when_present() {
    let mut doc = doc();
    doc.notes = Some("blocked on the CI runner".into());
    let rendered = doc.render("release-plan");
    assert!(rendered.contains("## Notes\nblocked on the CI runner"));
}

#[test]
fn write_creates_at_revision_one_then_read_renders_it() {
    let pads = scratchpads();
    let created = pads
        .write(PROJECT, "release-plan", doc(), None)
        .expect("create succeeds");
    assert_eq!(created.revision, 1);
    assert_eq!(created.name, "release-plan");
    assert!(created.rendered.starts_with("# release-plan"));

    let read = pads
        .read(PROJECT, "release-plan")
        .expect("read succeeds")
        .expect("the scratchpad exists");
    assert_eq!(read, created);
}

#[test]
fn write_at_the_current_revision_updates_and_bumps_it() {
    let pads = scratchpads();
    pads.write(PROJECT, "plan", doc(), None).expect("create");
    let mut next = doc();
    next.status = "tagged".into();

    let updated = pads
        .write(PROJECT, "plan", next, Some(1))
        .expect("update at the current revision succeeds");
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.doc.status, "tagged");
}

#[test]
fn a_stale_write_is_a_conflict_and_changes_nothing() {
    let pads = scratchpads();
    pads.write(PROJECT, "plan", doc(), None).expect("create");
    pads.write(PROJECT, "plan", doc(), Some(1))
        .expect("first update");

    // The document is now at revision 2; a writer still holding revision 1 is refused.
    let conflict = pads
        .write(PROJECT, "plan", doc(), Some(1))
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
    pads.write(PROJECT, "plan", doc(), None).expect("create");
    let conflict = pads
        .write(PROJECT, "plan", doc(), None)
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
        .write(PROJECT, "absent", doc(), Some(3))
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
fn a_malformed_write_is_rejected_before_it_persists() {
    let pads = scratchpads();
    let mut bad = doc();
    bad.objective = "   ".into();
    assert!(matches!(
        pads.write(PROJECT, "plan", bad, None),
        Err(WriteError::Invalid(_))
    ));
    assert!(
        pads.read(PROJECT, "plan").unwrap().is_none(),
        "a rejected write must not create the scratchpad"
    );
}

#[test]
fn rename_moves_the_handle_and_keeps_the_durable_id() {
    let pads = scratchpads();
    let created = pads.write(PROJECT, "old", doc(), None).expect("create");

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
    pads.write(PROJECT, "a", doc(), None).expect("create a");
    pads.write(PROJECT, "b", doc(), None).expect("create b");

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
    pads.write(PROJECT, "a", doc(), None).expect("create a");
    pads.write(PROJECT, "b", doc(), None).expect("create b");

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
    pads.write(PROJECT, "a", doc(), None).expect("create");

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
    pads.write(PROJECT, "a", doc(), None).expect("create");
    assert!(pads.delete(PROJECT, "a").expect("delete"));
    assert!(!pads.delete(PROJECT, "a").expect("second delete is a no-op"));
    assert!(pads.read(PROJECT, "a").unwrap().is_none());
}

#[test]
fn list_is_scoped_to_the_project_and_ordered_by_name() {
    let pads = scratchpads();
    pads.write(PROJECT, "zebra", doc(), None).expect("create");
    pads.write(PROJECT, "alpha", doc(), None).expect("create");
    pads.write(ProjectId::from_raw(2), "other", doc(), None)
        .expect("create in another project");

    let names: Vec<String> = pads
        .list(PROJECT)
        .expect("list")
        .into_iter()
        .map(|summary| summary.name)
        .collect();
    assert_eq!(names, vec!["alpha".to_string(), "zebra".to_string()]);
}
