use std::sync::Arc;

use super::*;
use crate::template::TemplateKind;
use crate::testing::FakeTemplateRepo;

const P: ProjectId = ProjectId::from_raw(1);
const PROMPT: TemplateKind = TemplateKind::Prompt;

fn templates() -> Templates {
    Templates::new(Arc::new(FakeTemplateRepo::new()))
}

/// A [`Templates`] over a fake whose store reads we can count, for the cache tests.
fn templates_over_counting_repo() -> (Templates, Arc<FakeTemplateRepo>) {
    let repo = Arc::new(FakeTemplateRepo::new());
    (Templates::new(repo.clone()), repo)
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
            PROMPT,
            Some(P),
            "review",
            Some("PR review prompt"),
            "Review {{diff}} for {{focus}}",
        )
        .expect("create");

    assert_eq!(created.name, "review");
    assert_eq!(created.kind, TemplateKind::Prompt);
    assert_eq!(created.scope, TemplateScope::Project);
    assert_eq!(created.revision, 1);
    assert_eq!(
        created.placeholders,
        vec!["diff".to_owned(), "focus".to_owned()]
    );
    let read = templates
        .read(PROMPT, Some(P), "review")
        .expect("read")
        .expect("present");
    assert_eq!(read, created);
}

#[test]
fn update_at_the_current_revision_bumps_it_and_a_stale_update_conflicts() {
    let templates = templates();
    templates
        .create(PROMPT, None, "triage", None, "Triage {{issue}}")
        .expect("create");

    let updated = templates
        .update(
            PROMPT,
            None,
            "triage",
            Some("desc"),
            "Triage {{issue}} by {{severity}}",
            1,
        )
        .expect("update at the read revision");
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.scope, TemplateScope::Global);

    let stale = templates
        .update(PROMPT, None, "triage", None, "clobber", 1)
        .expect_err("a stale revision is refused");
    assert!(matches!(
        stale,
        TemplateWriteError::Conflict {
            expected: Some(1),
            actual: Some(2),
        }
    ));
    let kept = templates
        .read(PROMPT, None, "triage")
        .expect("read")
        .expect("present");
    assert_eq!(kept.body, "Triage {{issue}} by {{severity}}");
}

#[test]
fn creating_over_an_existing_name_conflicts() {
    let templates = templates();
    templates
        .create(PROMPT, Some(P), "review", None, "one")
        .expect("create");

    let err = templates
        .create(PROMPT, Some(P), "review", None, "two")
        .expect_err("a taken name conflicts");
    assert!(matches!(
        err,
        TemplateWriteError::Conflict {
            expected: None,
            actual: Some(1),
        }
    ));
}

#[test]
fn updating_a_missing_template_conflicts_with_no_record() {
    let err = templates()
        .update(PROMPT, Some(P), "ghost", None, "body", 3)
        .expect_err("updating nothing conflicts");
    assert!(matches!(
        err,
        TemplateWriteError::Conflict {
            expected: Some(3),
            actual: None,
        }
    ));
}

#[test]
fn an_omitted_description_keeps_the_stored_one_and_a_blank_one_clears_it() {
    let templates = templates();
    templates
        .create(
            PROMPT,
            None,
            "triage",
            Some("sort the queue"),
            "Triage {{issue}}",
        )
        .expect("create");

    let kept = templates
        .update(PROMPT, None, "triage", None, "Triage {{issue}} fast", 1)
        .expect("update without a description");
    assert_eq!(kept.description.as_deref(), Some("sort the queue"));

    let cleared = templates
        .update(
            PROMPT,
            None,
            "triage",
            Some("   "),
            "Triage {{issue}} faster",
            2,
        )
        .expect("update with a blank description");
    assert_eq!(cleared.description, None);
}

#[test]
fn a_blank_description_on_create_is_stored_as_none() {
    let templates = templates();

    let view = templates
        .create(PROMPT, Some(P), "review", Some("  "), "body")
        .expect("create");

    assert_eq!(view.description, None);
}

#[test]
fn an_overlong_name_or_description_is_rejected_before_it_persists() {
    let templates = templates();

    let long_name = "n".repeat(MAX_TEMPLATE_NAME + 1);
    let err = templates
        .create(PROMPT, Some(P), &long_name, None, "body")
        .expect_err("an overlong name is rejected");
    assert!(err.to_string().contains("name exceeds"));

    let long_description = "d".repeat(MAX_TEMPLATE_DESCRIPTION + 1);
    let err = templates
        .create(PROMPT, Some(P), "review", Some(&long_description), "body")
        .expect_err("an overlong description is rejected");
    assert!(err.to_string().contains("description exceeds"));
    assert!(templates.list(PROMPT, Some(P)).expect("list").is_empty());
}

#[test]
fn a_malformed_template_is_rejected_before_it_persists() {
    let templates = templates();

    let err = templates
        .create(PROMPT, Some(P), "  ", "".into(), "  ")
        .expect_err("blank name and body are rejected");
    let message = err.to_string();
    assert!(message.contains("name is empty"));
    assert!(message.contains("body is empty"));
    assert!(templates.list(PROMPT, Some(P)).expect("list").is_empty());

    let oversized = "x".repeat(MAX_TEMPLATE_BODY + 1);
    assert!(matches!(
        templates.create(PROMPT, Some(P), "big", None, &oversized),
        Err(TemplateWriteError::Invalid(_))
    ));
}

#[test]
fn the_same_name_lives_independently_in_global_and_project_scope() {
    let templates = templates();
    templates
        .create(PROMPT, None, "review", None, "global {{a}}")
        .expect("global");
    templates
        .create(PROMPT, Some(P), "review", None, "project {{b}}")
        .expect("project");

    assert_eq!(
        templates
            .read(PROMPT, None, "review")
            .expect("read")
            .expect("present")
            .body,
        "global {{a}}"
    );
    assert_eq!(
        templates
            .read(PROMPT, Some(P), "review")
            .expect("read")
            .expect("present")
            .body,
        "project {{b}}"
    );
    assert!(templates
        .delete(PROMPT, None, "review")
        .expect("delete global"));
    assert!(
        templates
            .read(PROMPT, Some(P), "review")
            .expect("read")
            .is_some(),
        "deleting the global one leaves the project's"
    );
}

#[test]
fn the_same_name_lives_independently_across_kinds() {
    let templates = templates();
    templates
        .create(TemplateKind::Prompt, Some(P), "design", None, "prompt body")
        .expect("prompt");
    templates
        .create(
            TemplateKind::Scratchpad,
            Some(P),
            "design",
            None,
            "# scratchpad shape",
        )
        .expect("scratchpad");

    // The same name under two kinds is two independent rows, each addressed by its kind.
    assert_eq!(
        templates
            .read(TemplateKind::Prompt, Some(P), "design")
            .expect("read")
            .expect("present")
            .body,
        "prompt body"
    );
    assert_eq!(
        templates
            .read(TemplateKind::Scratchpad, Some(P), "design")
            .expect("read")
            .expect("present")
            .body,
        "# scratchpad shape"
    );
    // A prompt-scoped list never carries the scratchpad row.
    let prompts = templates.list(TemplateKind::Prompt, Some(P)).expect("list");
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].kind, TemplateKind::Prompt);
}

#[test]
fn list_is_scoped_and_summaries_carry_no_body() {
    let templates = templates();
    templates
        .create(PROMPT, Some(P), "b-second", None, "two {{x}}")
        .expect("create");
    templates
        .create(PROMPT, Some(P), "a-first", None, "one")
        .expect("create");
    templates
        .create(PROMPT, None, "global", None, "three")
        .expect("create");

    let listed = templates.list(PROMPT, Some(P)).expect("list");
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
        .create(PROMPT, Some(P), "review", Some("desc"), "Review {{diff}}")
        .expect("create");

    let exported = templates
        .export(PROMPT, Some(P), "review")
        .expect("export")
        .expect("present");

    assert_eq!(exported.format, TemplateKind::Prompt.export_format());
    assert_eq!(exported.name, "review");
    assert_eq!(exported.description.as_deref(), Some("desc"));
    assert_eq!(exported.body, "Review {{diff}}");
    assert!(templates
        .export(PROMPT, Some(P), "ghost")
        .expect("export")
        .is_none());
}

#[test]
fn a_warm_list_is_served_from_the_cache_without_a_second_store_scan() {
    let (templates, repo) = templates_over_counting_repo();
    templates
        .create(TemplateKind::Scratchpad, None, "design", None, "# shape")
        .expect("create");

    // First list warms the cache; the second is served from it (no extra store scan).
    let first = templates
        .list(TemplateKind::Scratchpad, None)
        .expect("first list");
    let second = templates
        .list(TemplateKind::Scratchpad, None)
        .expect("second list");
    assert_eq!(first, second);
    assert_eq!(
        repo.list_calls(),
        1,
        "the warm read hits the cache, not the store"
    );

    // A resolve during seeding also rides the same cache — still no new scan.
    let resolved = templates
        .resolve(TemplateKind::Scratchpad, None, first[0].id)
        .expect("resolve")
        .expect("present");
    assert_eq!(resolved.body, "# shape");
    assert_eq!(repo.list_calls(), 1, "resolve is a cache read");
}

#[test]
fn a_write_invalidates_only_its_own_kind_and_scope_cache() {
    let (templates, repo) = templates_over_counting_repo();
    templates
        .create(TemplateKind::Scratchpad, None, "one", None, "# one")
        .expect("create");
    templates
        .list(TemplateKind::Scratchpad, None)
        .expect("warm scratchpad-global cache");
    templates
        .list(TemplateKind::Todo, None)
        .expect("warm todo-global cache");
    assert_eq!(repo.list_calls(), 2, "each kind's first list scanned once");

    // Writing a scratchpad-global template invalidates that entry, so its next list rescans...
    templates
        .create(TemplateKind::Scratchpad, None, "two", None, "# two")
        .expect("create");
    let scratchpads = templates
        .list(TemplateKind::Scratchpad, None)
        .expect("re-list after write");
    assert_eq!(scratchpads.len(), 2, "the new template is now visible");
    assert_eq!(repo.list_calls(), 3, "the invalidated cache rescanned once");

    // ...while the untouched todo-global cache stays warm (no rescan).
    templates
        .list(TemplateKind::Todo, None)
        .expect("todo list still warm");
    assert_eq!(
        repo.list_calls(),
        3,
        "an unrelated cache entry is untouched"
    );
}
