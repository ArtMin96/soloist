use std::sync::Arc;

use super::*;
use crate::template::TemplateKind;
use crate::testing::FakeTemplateRepo;

const P: ProjectId = ProjectId::from_raw(1);
const OTHER: ProjectId = ProjectId::from_raw(2);
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
fn an_escaped_marker_declares_no_placeholder_but_a_doubled_backslash_still_does() {
    assert_eq!(placeholders(r"literal \{{x}} only"), Vec::<String>::new());
    assert_eq!(
        placeholders(r"a backslash \\{{x}} then a name"),
        vec!["x".to_owned()]
    );
    assert_eq!(
        placeholders(r"{{first}} then \{{skipped}} then {{second}}"),
        vec!["first".to_owned(), "second".to_owned()]
    );
}

#[test]
fn a_non_ascii_placeholder_name_is_declared_whole() {
    assert_eq!(
        placeholders("Проверь {{ файл }} и «{{résumé}}» 🌍 {{файл}}"),
        vec!["файл".to_owned(), "résumé".to_owned()]
    );
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
}

#[test]
fn the_body_cap_is_bytes_at_its_exact_boundary_while_the_name_cap_is_characters() {
    let templates = templates();

    // A body of exactly the cap fits; one byte more does not.
    let at_cap = "x".repeat(MAX_TEMPLATE_BODY);
    templates
        .create(PROMPT, Some(P), "at-cap", None, &at_cap)
        .expect("a body of exactly the cap is accepted");
    let err = templates
        .create(PROMPT, Some(P), "over-cap", None, &format!("{at_cap}x"))
        .expect_err("one byte past the cap is refused");
    assert!(err.to_string().contains("body exceeds"));

    // The body is measured in bytes, so a body far under the cap in characters is still refused
    // for what it costs to store...
    let multibyte = "é".repeat(MAX_TEMPLATE_BODY / 2 + 1);
    assert!(multibyte.chars().count() < MAX_TEMPLATE_BODY);
    let err = templates
        .create(PROMPT, Some(P), "multibyte", None, &multibyte)
        .expect_err("a multibyte body past the byte cap is refused");
    assert!(err.to_string().contains("body exceeds"));

    // ...while the name is measured in characters, so a name that is double the cap in bytes is
    // accepted at exactly the cap in characters.
    let multibyte_name = "é".repeat(MAX_TEMPLATE_NAME);
    assert!(multibyte_name.len() > MAX_TEMPLATE_NAME);
    templates
        .create(PROMPT, Some(P), &multibyte_name, None, "body")
        .expect("a name of exactly the cap in characters is accepted");

    assert_eq!(
        templates
            .list(PROMPT, Some(P))
            .expect("list")
            .iter()
            .map(|row| row.name.clone())
            .collect::<Vec<_>>(),
        vec!["at-cap".to_owned(), multibyte_name],
        "neither refused body reached the store"
    );
}

#[test]
fn a_body_declaring_more_distinct_placeholders_than_the_cap_is_refused() {
    let templates = templates();
    let declaring = |count: usize| {
        (0..count)
            .map(|index| format!("{{{{p{index}}}}}"))
            .collect::<String>()
    };

    // Exactly the cap fits, and every name it declares is derived back out...
    let created = templates
        .create(
            PROMPT,
            Some(P),
            "at-cap",
            None,
            &declaring(MAX_PLACEHOLDERS_PER_BODY),
        )
        .expect("a body declaring exactly the cap is accepted");
    assert_eq!(created.placeholders.len(), MAX_PLACEHOLDERS_PER_BODY);

    // ...one name more does not.
    let err = templates
        .create(
            PROMPT,
            Some(P),
            "over-cap",
            None,
            &declaring(MAX_PLACEHOLDERS_PER_BODY + 1),
        )
        .expect_err("one placeholder past the cap is refused");
    assert!(err.to_string().contains(&format!(
        "more than {MAX_PLACEHOLDERS_PER_BODY} distinct placeholders"
    )));

    // The cap counts distinct names, matching what a listing allocates, so repeating one name is
    // never what pushes a body over it.
    let repeated = templates
        .create(
            PROMPT,
            Some(P),
            "repeated",
            None,
            &"{{one}}".repeat(MAX_PLACEHOLDERS_PER_BODY * 10),
        )
        .expect("one name repeated past the cap declares a single placeholder");
    assert_eq!(repeated.placeholders, vec!["one".to_owned()]);

    assert_eq!(
        templates
            .list(PROMPT, Some(P))
            .expect("list")
            .iter()
            .map(|row| row.name.clone())
            .collect::<Vec<_>>(),
        vec!["at-cap".to_owned(), "repeated".to_owned()],
        "the refused body never reached the store"
    );
}

/// Fills `(PROMPT, P)` to exactly [`MAX_TEMPLATES_PER_SCOPE`] rows.
fn templates_at_the_scope_ceiling() -> Templates {
    let templates = templates();
    for index in 0..MAX_TEMPLATES_PER_SCOPE {
        templates
            .create(PROMPT, Some(P), &format!("t{index:04}"), None, "body")
            .expect("a create below the ceiling");
    }
    templates
}

#[test]
fn a_scope_at_its_template_ceiling_refuses_another_create_until_one_is_deleted() {
    let templates = templates_at_the_scope_ceiling();

    let err = templates
        .create(PROMPT, Some(P), "one-too-many", None, "body")
        .expect_err("a create at the ceiling is refused");
    assert!(err
        .to_string()
        .contains(&format!("maximum of {MAX_TEMPLATES_PER_SCOPE} templates")));
    assert_eq!(
        templates.list(PROMPT, Some(P)).expect("list").len(),
        MAX_TEMPLATES_PER_SCOPE,
        "the refused create never reached the store"
    );

    assert!(templates.delete(PROMPT, Some(P), "t0000").expect("delete"));
    templates
        .create(PROMPT, Some(P), "one-too-many", None, "body")
        .expect("deleting a template makes room for another");
}

#[test]
fn the_template_ceiling_binds_one_kind_and_scope_and_never_blocks_an_edit() {
    let templates = templates_at_the_scope_ceiling();

    // An update replaces a row rather than adding one, so a full scope still edits.
    let edited = templates
        .update(PROMPT, Some(P), "t0001", None, "edited", 1)
        .expect("an update at the ceiling");
    assert_eq!(edited.body, "edited");

    // Every other group counts its own rows: the global scope, another project, another kind.
    for (kind, project) in [
        (PROMPT, None),
        (PROMPT, Some(OTHER)),
        (TemplateKind::Scratchpad, Some(P)),
    ] {
        templates
            .create(kind, project, "elsewhere", None, "body")
            .expect("a full group never blocks another");
    }
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

    // The tag is pinned literally, not compared against `export_format()` — the envelope is built
    // from that very call, so comparing the two would compare production to itself.
    assert_eq!(exported.format, "soloist.prompt-template/v1");
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

#[test]
fn a_delete_drops_the_warm_cache_so_the_next_list_loses_the_row() {
    let (templates, repo) = templates_over_counting_repo();
    for name in ["keep", "doomed"] {
        templates
            .create(PROMPT, Some(P), name, None, "body")
            .expect("create");
    }
    // Warm the cache *before* the delete — a cold or bypassed read would repopulate from the store
    // and pass whether or not the delete invalidated anything.
    assert_eq!(
        templates
            .list(PROMPT, Some(P))
            .expect("warm the cache")
            .len(),
        2
    );
    let scans = repo.list_calls();

    assert!(templates
        .delete(PROMPT, Some(P), "doomed")
        .expect("delete the row"));

    // The next list must show the removal. Served from a cache the delete failed to drop, it would
    // still hand back the deleted template — and the seeding resolve off the same entry would hand
    // back its body.
    assert_eq!(
        templates
            .list(PROMPT, Some(P))
            .expect("re-list after the delete")
            .into_iter()
            .map(|row| row.name)
            .collect::<Vec<_>>(),
        vec!["keep".to_owned()]
    );
    assert_eq!(
        repo.list_calls(),
        scans + 1,
        "the invalidated entry rescanned once"
    );
}

#[test]
fn a_delete_that_removed_nothing_leaves_the_cache_warm() {
    let (templates, repo) = templates_over_counting_repo();
    templates
        .create(PROMPT, Some(P), "keep", None, "body")
        .expect("create");
    templates.list(PROMPT, Some(P)).expect("warm the cache");
    let scans = repo.list_calls();

    assert!(!templates
        .delete(PROMPT, Some(P), "ghost")
        .expect("delete a name the scope does not hold"));

    templates.list(PROMPT, Some(P)).expect("list again");
    assert_eq!(
        repo.list_calls(),
        scans,
        "a delete that removed nothing must not cost the scope its cached rows"
    );
}

#[test]
fn a_padded_name_addresses_the_same_template_every_write_and_read_does() {
    let templates = templates();

    // The name is trimmed on the way in, so the stored handle is the bare one...
    let created = templates
        .create(PROMPT, Some(P), "  review\t", None, "body")
        .expect("create under a padded name");
    assert_eq!(created.name, "review");

    // ...and every path that addresses a template by name trims the same way, or a caller that
    // pasted a name with a stray space would read, edit, export, or delete nothing.
    assert_eq!(
        templates
            .read(PROMPT, Some(P), " review ")
            .expect("read")
            .expect("a padded name reaches the stored row")
            .id,
        created.id
    );
    assert_eq!(
        templates
            .export(PROMPT, Some(P), "\nreview ")
            .expect("export")
            .expect("a padded name reaches the stored row")
            .name,
        "review"
    );
    let updated = templates
        .update(PROMPT, Some(P), " review", None, "edited", created.revision)
        .expect("update under a padded name");
    assert_eq!(updated.id, created.id);
    assert_eq!(updated.body, "edited");
    assert!(templates
        .delete(PROMPT, Some(P), "review  ")
        .expect("delete under a padded name"));
    assert!(templates.list(PROMPT, Some(P)).expect("list").is_empty());
}

#[test]
fn an_omitted_description_is_kept_across_an_update_under_a_padded_name() {
    let templates = templates();
    templates
        .create(PROMPT, Some(P), "review", Some("PR review"), "body")
        .expect("create");

    // The keep-read that resolves an omitted description addresses the template by name too, so it
    // must trim like the write it feeds — otherwise a padded update silently clears the description
    // it was meant to preserve.
    let updated = templates
        .update(PROMPT, Some(P), "  review  ", None, "edited", 1)
        .expect("update under a padded name");
    assert_eq!(updated.description.as_deref(), Some("PR review"));
}

#[test]
fn two_projects_of_one_kind_are_cached_and_listed_apart() {
    let (templates, repo) = templates_over_counting_repo();
    templates
        .create(PROMPT, Some(P), "review", None, "review {{diff}}")
        .expect("create in P");
    templates
        .create(PROMPT, Some(OTHER), "ship", None, "ship it")
        .expect("create in OTHER");

    // Each project's scope is its own cache entry, so warming one never answers for the other.
    let mine: Vec<String> = templates
        .list(PROMPT, Some(P))
        .expect("list P")
        .into_iter()
        .map(|row| row.name)
        .collect();
    let theirs: Vec<String> = templates
        .list(PROMPT, Some(OTHER))
        .expect("list OTHER")
        .into_iter()
        .map(|row| row.name)
        .collect();
    assert_eq!(mine, vec!["review".to_owned()]);
    assert_eq!(theirs, vec!["ship".to_owned()]);
    assert_eq!(repo.list_calls(), 2, "each project's scope scanned once");

    // A write in one project rescans only that project's entry; the other stays warm and correct.
    templates
        .create(PROMPT, Some(OTHER), "release", None, "release it")
        .expect("second create in OTHER");
    assert_eq!(
        templates.list(PROMPT, Some(OTHER)).expect("re-list").len(),
        2,
        "the writing project sees its new template"
    );
    assert_eq!(repo.list_calls(), 3, "only the written scope rescanned");
    assert_eq!(
        templates
            .list(PROMPT, Some(P))
            .expect("list P again")
            .into_iter()
            .map(|row| row.name)
            .collect::<Vec<_>>(),
        vec!["review".to_owned()],
        "a write in another project never leaks into this one's rows"
    );
    assert_eq!(repo.list_calls(), 3, "this project's cache stayed warm");
}

#[test]
fn forgetting_a_project_drops_its_entries_of_every_kind_and_no_others() {
    let (templates, repo) = templates_over_counting_repo();
    for kind in [PROMPT, TemplateKind::Scratchpad] {
        templates
            .create(kind, Some(P), "mine", None, "body")
            .expect("create in P");
        templates.list(kind, Some(P)).expect("warm P");
    }
    templates
        .create(PROMPT, Some(OTHER), "theirs", None, "body")
        .expect("create in OTHER");
    templates.list(PROMPT, Some(OTHER)).expect("warm OTHER");
    templates.list(PROMPT, None).expect("warm global");
    let scans = repo.list_calls();

    templates.forget_project(P);

    // Every one of that project's entries is gone, whatever the kind...
    templates.list(PROMPT, Some(P)).expect("re-list P prompts");
    templates
        .list(TemplateKind::Scratchpad, Some(P))
        .expect("re-list P scratchpad shapes");
    assert_eq!(
        repo.list_calls(),
        scans + 2,
        "both of P's entries rescanned"
    );

    // ...while another project's rows and the global scope are untouched.
    templates.list(PROMPT, Some(OTHER)).expect("list OTHER");
    templates.list(PROMPT, None).expect("list global");
    assert_eq!(
        repo.list_calls(),
        scans + 2,
        "forgetting one project leaves every other scope warm"
    );
}
