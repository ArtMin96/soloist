use super::*;

#[test]
fn every_kind_persists_under_the_tag_it_serializes_to() {
    // The `kind` column and the JSON wire form are read by different readers of the same rows, so
    // the two spellings agreeing is the contract — not that each is internally consistent. Derived
    // serde would drift silently from a hand-written `as_str` arm.
    for kind in TemplateKind::ALL {
        let wire = serde_json::to_string(&kind).expect("serialize");
        assert_eq!(
            wire,
            format!("\"{}\"", kind.as_str()),
            "{kind:?} persists as {:?} but serializes as {wire}",
            kind.as_str()
        );
        assert_eq!(
            TemplateKind::from_db(kind.as_str()),
            Some(kind),
            "a persisted {kind:?} must parse back"
        );
    }
}

#[test]
fn a_kind_the_column_does_not_name_parses_to_nothing() {
    // A row written by a newer build, a typo, or a corrupted column must be reportable rather than
    // panic the read path, so every unrecognised value is `None`.
    for unknown in ["", "prompts", "Prompt", "PROMPT", " prompt", "agent"] {
        assert_eq!(
            TemplateKind::from_db(unknown),
            None,
            "{unknown:?} is not a kind this build stores"
        );
    }
}

#[test]
fn each_kind_exports_under_its_own_pinned_format_tag() {
    // A saved export carries this tag and is re-read by whatever holds the file later, so the exact
    // string per kind is the contract — comparing an export against `export_format()` would compare
    // production to itself and survive any rename.
    assert_eq!(
        TemplateKind::Prompt.export_format(),
        "soloist.prompt-template/v1"
    );
    assert_eq!(
        TemplateKind::Scratchpad.export_format(),
        "soloist.scratchpad-template/v1"
    );
    assert_eq!(
        TemplateKind::Todo.export_format(),
        "soloist.todo-template/v1"
    );

    // No two kinds may share a tag, or a re-create could not tell which library an export belongs in.
    let tags: Vec<&str> = TemplateKind::ALL
        .iter()
        .map(|kind| kind.export_format())
        .collect();
    for (index, tag) in tags.iter().enumerate() {
        assert!(
            !tags[..index].contains(tag),
            "{tag} is claimed by more than one kind"
        );
    }
}

#[test]
fn every_scope_serializes_to_its_wire_tag() {
    for (scope, tag) in [
        (TemplateScope::Global, "\"global\""),
        (TemplateScope::Project, "\"project\""),
    ] {
        assert_eq!(serde_json::to_string(&scope).expect("serialize"), tag);
        assert_eq!(
            serde_json::from_str::<TemplateScope>(tag).expect("deserialize"),
            scope
        );
    }
}
