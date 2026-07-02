use std::sync::Arc;

use super::*;
use crate::testing::FakePromptTemplateRepo;

const P: ProjectId = ProjectId::from_raw(1);

fn templates() -> PromptTemplates {
    PromptTemplates::new(Arc::new(FakePromptTemplateRepo::new()))
}

#[test]
fn placeholders_are_trimmed_deduped_and_in_first_occurrence_order() {
    assert_eq!(
        placeholders("Fix {{ bug }} in {{file}}, then verify {{bug}} again"),
        vec!["bug".to_owned(), "file".to_owned()]
    );
}

#[test]
fn placeholders_ignore_empty_unclosed_and_multiline_candidates() {
    assert_eq!(
        placeholders("empty {{}} blank {{  }} unclosed {{oops"),
        Vec::<String>::new()
    );
    assert_eq!(placeholders("spans {{a\nb}} lines"), Vec::<String>::new());
    assert_eq!(placeholders("no markers at all"), Vec::<String>::new());
}

#[test]
fn a_malformed_candidate_consumes_its_span_without_rescanning() {
    // The first `}}` closes `a{{b`, which still contains a brace, so nothing is a
    // placeholder — including the `{{b` inside the rejected span.
    assert_eq!(placeholders("{{a{{b}} c}}"), Vec::<String>::new());
    // Text after the rejected span scans normally.
    assert_eq!(placeholders("{{a{{b}} then {{ok}}"), vec!["ok".to_owned()]);
}

#[test]
fn create_then_read_round_trips_and_derives_the_view() {
    let templates = templates();

    let created = templates
        .create(
            Some(P),
            "review",
            Some("PR review prompt"),
            "Review {{diff}} for {{focus}}",
        )
        .expect("create");

    assert_eq!(created.name, "review");
    assert_eq!(created.scope, PromptScope::Project);
    assert_eq!(created.revision, 1);
    assert_eq!(
        created.placeholders,
        vec!["diff".to_owned(), "focus".to_owned()]
    );
    let read = templates
        .read(Some(P), "review")
        .expect("read")
        .expect("present");
    assert_eq!(read, created);
}

#[test]
fn update_at_the_current_revision_bumps_it_and_a_stale_update_conflicts() {
    let templates = templates();
    templates
        .create(None, "triage", None, "Triage {{issue}}")
        .expect("create");

    let updated = templates
        .update(
            None,
            "triage",
            Some("desc"),
            "Triage {{issue}} by {{severity}}",
            1,
        )
        .expect("update at the read revision");
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.scope, PromptScope::Global);

    let stale = templates
        .update(None, "triage", None, "clobber", 1)
        .expect_err("a stale revision is refused");
    assert!(matches!(
        stale,
        PromptTemplateWriteError::Conflict {
            expected: Some(1),
            actual: Some(2),
        }
    ));
    let kept = templates
        .read(None, "triage")
        .expect("read")
        .expect("present");
    assert_eq!(kept.body, "Triage {{issue}} by {{severity}}");
}

#[test]
fn creating_over_an_existing_name_conflicts() {
    let templates = templates();
    templates
        .create(Some(P), "review", None, "one")
        .expect("create");

    let err = templates
        .create(Some(P), "review", None, "two")
        .expect_err("a taken name conflicts");
    assert!(matches!(
        err,
        PromptTemplateWriteError::Conflict {
            expected: None,
            actual: Some(1),
        }
    ));
}

#[test]
fn updating_a_missing_template_conflicts_with_no_record() {
    let err = templates()
        .update(Some(P), "ghost", None, "body", 3)
        .expect_err("updating nothing conflicts");
    assert!(matches!(
        err,
        PromptTemplateWriteError::Conflict {
            expected: Some(3),
            actual: None,
        }
    ));
}

#[test]
fn an_omitted_description_keeps_the_stored_one_and_a_blank_one_clears_it() {
    let templates = templates();
    templates
        .create(None, "triage", Some("sort the queue"), "Triage {{issue}}")
        .expect("create");

    let kept = templates
        .update(None, "triage", None, "Triage {{issue}} fast", 1)
        .expect("update without a description");
    assert_eq!(kept.description.as_deref(), Some("sort the queue"));

    let cleared = templates
        .update(None, "triage", Some("   "), "Triage {{issue}} faster", 2)
        .expect("update with a blank description");
    assert_eq!(cleared.description, None);
}

#[test]
fn a_blank_description_on_create_is_stored_as_none() {
    let templates = templates();

    let view = templates
        .create(Some(P), "review", Some("  "), "body")
        .expect("create");

    assert_eq!(view.description, None);
}

#[test]
fn an_overlong_name_or_description_is_rejected_before_it_persists() {
    let templates = templates();

    let long_name = "n".repeat(MAX_PROMPT_TEMPLATE_NAME + 1);
    let err = templates
        .create(Some(P), &long_name, None, "body")
        .expect_err("an overlong name is rejected");
    assert!(err.to_string().contains("name exceeds"));

    let long_description = "d".repeat(MAX_PROMPT_TEMPLATE_DESCRIPTION + 1);
    let err = templates
        .create(Some(P), "review", Some(&long_description), "body")
        .expect_err("an overlong description is rejected");
    assert!(err.to_string().contains("description exceeds"));
    assert!(templates.list(Some(P)).expect("list").is_empty());
}

#[test]
fn a_malformed_template_is_rejected_before_it_persists() {
    let templates = templates();

    let err = templates
        .create(Some(P), "  ", "".into(), "  ")
        .expect_err("blank name and body are rejected");
    let message = err.to_string();
    assert!(message.contains("name is empty"));
    assert!(message.contains("body is empty"));
    assert!(templates.list(Some(P)).expect("list").is_empty());

    let oversized = "x".repeat(MAX_PROMPT_TEMPLATE_BODY + 1);
    assert!(matches!(
        templates.create(Some(P), "big", None, &oversized),
        Err(PromptTemplateWriteError::Invalid(_))
    ));
}

#[test]
fn the_same_name_lives_independently_in_global_and_project_scope() {
    let templates = templates();
    templates
        .create(None, "review", None, "global {{a}}")
        .expect("global");
    templates
        .create(Some(P), "review", None, "project {{b}}")
        .expect("project");

    assert_eq!(
        templates
            .read(None, "review")
            .expect("read")
            .expect("present")
            .body,
        "global {{a}}"
    );
    assert_eq!(
        templates
            .read(Some(P), "review")
            .expect("read")
            .expect("present")
            .body,
        "project {{b}}"
    );
    assert!(templates.delete(None, "review").expect("delete global"));
    assert!(
        templates.read(Some(P), "review").expect("read").is_some(),
        "deleting the global one leaves the project's"
    );
}

#[test]
fn list_is_scoped_and_summaries_carry_no_body() {
    let templates = templates();
    templates
        .create(Some(P), "b-second", None, "two {{x}}")
        .expect("create");
    templates
        .create(Some(P), "a-first", None, "one")
        .expect("create");
    templates
        .create(None, "global", None, "three")
        .expect("create");

    let listed = templates.list(Some(P)).expect("list");
    assert_eq!(
        listed.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        vec!["a-first", "b-second"],
        "ordered by name, global rows absent"
    );
    assert_eq!(listed[1].placeholders, vec!["x".to_owned()]);
}

#[test]
fn export_wraps_the_template_in_the_portable_envelope() {
    let templates = templates();
    templates
        .create(Some(P), "review", Some("desc"), "Review {{diff}}")
        .expect("create");

    let exported = templates
        .export(Some(P), "review")
        .expect("export")
        .expect("present");

    assert_eq!(exported.format, PROMPT_TEMPLATE_EXPORT_FORMAT);
    assert_eq!(exported.name, "review");
    assert_eq!(exported.description.as_deref(), Some("desc"));
    assert_eq!(exported.body, "Review {{diff}}");
    assert!(templates
        .export(Some(P), "ghost")
        .expect("export")
        .is_none());
}
