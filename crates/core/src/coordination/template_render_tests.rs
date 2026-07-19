use std::sync::Arc;

use super::*;
use crate::coordination::placeholders;
use crate::template::TemplateKind;
use crate::testing::FakeTemplateRepo;

const P: ProjectId = ProjectId::from_raw(1);

fn templates() -> Templates {
    Templates::new(Arc::new(FakeTemplateRepo::new()))
}

/// A prompt template in the global scope, ready to render.
fn with_prompt(name: &str, body: &str) -> Templates {
    let templates = templates();
    templates
        .create(TemplateKind::Prompt, None, name, None, body)
        .expect("seed the prompt template");
    templates
}

fn values(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

fn request(name: &str, pairs: &[(&str, &str)]) -> RenderRequest {
    RenderRequest {
        name: name.to_owned(),
        values: values(pairs),
        ..RenderRequest::default()
    }
}

/// Renders `body` with `pairs` straight through the pure stages, skipping the store — the
/// declared names come from the same derivation a read performs.
fn render(body: &str, pairs: &[(&str, &str)]) -> RenderedPrompt {
    render_with(body, pairs, MissingPolicy::LeaveVerbatim).expect("render the body")
}

fn render_with(
    body: &str,
    pairs: &[(&str, &str)],
    policy: MissingPolicy,
) -> Result<RenderedPrompt, RenderError> {
    render_body(
        body,
        &placeholders(body),
        &RenderRequest {
            values: values(pairs),
            policy,
            ..RenderRequest::default()
        },
    )
}

#[test]
fn a_supplied_value_replaces_every_occurrence_of_its_marker() {
    let rendered = render(
        "Fix {{ bug }} in {{file}}, then verify {{bug}}",
        &[("bug", "the crash"), ("file", "main.rs")],
    );
    assert_eq!(
        rendered.text,
        "Fix the crash in main.rs, then verify the crash"
    );
    assert!(rendered.unfilled.is_empty());
    assert!(rendered.unknown.is_empty());
}

#[test]
fn an_unfilled_placeholder_stays_in_the_text_and_is_reported() {
    let rendered = render("review {{file}} for {{concern}}", &[("file", "main.rs")]);
    // The gap travels with the artifact: a reader sees what was never filled in, where an empty
    // string would read as a complete instruction with a target to guess at.
    assert_eq!(rendered.text, "review main.rs for {{concern}}");
    assert_eq!(rendered.unfilled, vec!["concern".to_owned()]);
}

#[test]
fn an_unfilled_placeholder_keeps_the_spacing_it_was_written_with() {
    // The marker's whole span is restored, not a re-spelling of its trimmed name.
    assert_eq!(render("a {{ spaced }} b", &[]).text, "a {{ spaced }} b");
}

#[test]
fn a_value_naming_no_placeholder_is_reported_as_unknown() {
    let rendered = render("review {{diff}}", &[("diff", "d"), ("dif", "typo")]);
    assert_eq!(rendered.text, "review d");
    assert_eq!(rendered.unknown, vec!["dif".to_owned()]);
    assert!(rendered.unfilled.is_empty());
}

#[test]
fn unfilled_follows_the_body_and_unknown_follows_the_values() {
    let rendered = render("{{zebra}} {{apple}}", &[("zoo", "z"), ("ant", "a")]);
    assert_eq!(
        rendered.unfilled,
        vec!["zebra".to_owned(), "apple".to_owned()]
    );
    assert_eq!(rendered.unknown, vec!["ant".to_owned(), "zoo".to_owned()]);
}

#[test]
fn an_escaped_marker_is_literal_and_is_neither_filled_nor_reported() {
    let rendered = render(r"literal \{{x}} and {{y}}", &[("x", "NO"), ("y", "yes")]);
    assert_eq!(rendered.text, "literal {{x}} and yes");
    // `x` declares no placeholder, so supplying it is a caller mistake worth surfacing.
    assert_eq!(rendered.unknown, vec!["x".to_owned()]);
    assert!(rendered.unfilled.is_empty());
}

#[test]
fn a_value_containing_a_marker_is_inert() {
    // Substituted text is never fed back into the scan, so a caller's data cannot inject a
    // placeholder — neither one that would be filled nor one that would be reported.
    let rendered = render("outer {{a}}", &[("a", "{{b}}"), ("b", "LEAKED")]);
    assert_eq!(rendered.text, "outer {{b}}");
    assert_eq!(rendered.unknown, vec!["b".to_owned()]);
    assert!(rendered.unfilled.is_empty());
}

#[test]
fn a_value_recreating_its_own_marker_does_not_recurse() {
    assert_eq!(render("{{a}}", &[("a", "{{a}}")]).text, "{{a}}");
}

#[test]
fn values_are_emitted_exactly_as_given() {
    // A prompt carries code; escaping its markup would corrupt the payload.
    let rendered = render(
        "```\n{{code}}\n```",
        &[("code", "if a < b && c > d { \"q\" } // &amp;")],
    );
    assert_eq!(
        rendered.text,
        "```\nif a < b && c > d { \"q\" } // &amp;\n```"
    );
}

#[test]
fn a_non_ascii_name_and_value_render_on_character_boundaries() {
    let rendered = render("héllo {{naïve}} 🎉", &[("naïve", "wörld 🌍")]);
    assert_eq!(rendered.text, "héllo wörld 🌍 🎉");
}

#[test]
fn strict_refuses_a_render_with_any_unfilled_placeholder() {
    let refused = render_with(
        "{{a}} {{b}} {{c}}",
        &[("b", "filled")],
        MissingPolicy::Strict,
    );
    assert!(
        matches!(&refused, Err(RenderError::MissingValues(names)) if names == &["a".to_owned(), "c".to_owned()]),
        "expected the missing names, got {refused:?}"
    );
}

#[test]
fn strict_renders_once_every_placeholder_has_a_value() {
    let rendered = render_with(
        "{{a}} {{b}}",
        &[("a", "1"), ("b", "2")],
        MissingPolicy::Strict,
    )
    .expect("a fully supplied render");
    assert_eq!(rendered.text, "1 2");
}

#[test]
fn strict_ignores_an_unknown_value_that_fills_nothing() {
    let rendered = render_with("{{a}}", &[("a", "1"), ("b", "2")], MissingPolicy::Strict)
        .expect("an unknown value is not a missing one");
    assert_eq!(rendered.unknown, vec!["b".to_owned()]);
}

#[test]
fn a_render_at_the_cap_is_allowed_and_one_byte_over_is_refused() {
    let at_cap = render("{{v}}", &[("v", &"x".repeat(MAX_RENDERED_PROMPT))]);
    assert_eq!(at_cap.text.len(), MAX_RENDERED_PROMPT);

    let over = render_with(
        "{{v}}",
        &[("v", &"x".repeat(MAX_RENDERED_PROMPT + 1))],
        MissingPolicy::LeaveVerbatim,
    );
    assert!(
        matches!(
            over,
            Err(RenderError::RenderedTooLarge { bytes, cap })
                if bytes == MAX_RENDERED_PROMPT + 1 && cap == MAX_RENDERED_PROMPT
        ),
        "expected the cap refusal, got {over:?}"
    );
}

#[test]
fn many_markers_of_one_value_are_capped_together() {
    // The bound is on the rendered total, not on any single value: a body well under its own cap
    // can still multiply a modest value past the ceiling.
    let body = "{{v}}".repeat(1_000);
    let value = "y".repeat(MAX_RENDERED_PROMPT / 500);
    let over = render_with(&body, &[("v", &value)], MissingPolicy::LeaveVerbatim);
    assert!(
        matches!(over, Err(RenderError::RenderedTooLarge { .. })),
        "expected the cap refusal, got {over:?}"
    );
}

#[test]
fn render_reads_the_named_template_from_the_addressed_scope() {
    let templates = with_prompt("review", "review {{file}}");
    let rendered = templates
        .render(None, &request("review", &[("file", "main.rs")]))
        .expect("render the global template");
    assert_eq!(rendered.text, "review main.rs");

    // The same name in a project scope is a different template, and the global one is not visible
    // from it.
    let refused = templates.render(Some(P), &request("review", &[]));
    assert!(
        matches!(refused, Err(RenderError::TemplateNotFound)),
        "expected the project scope to be empty, got {refused:?}"
    );
}

#[test]
fn render_refuses_a_name_that_is_not_a_prompt_template() {
    let templates = with_prompt("shape", "prompt {{a}}");
    templates
        .create(
            TemplateKind::Scratchpad,
            None,
            "notes",
            None,
            "scratchpad {{a}}",
        )
        .expect("seed a scratchpad template");
    // Rendering is defined for prompts only: the scratchpad and todo kinds seed a body verbatim.
    let refused = templates.render(None, &request("notes", &[("a", "filled")]));
    assert!(
        matches!(refused, Err(RenderError::TemplateNotFound)),
        "expected only prompt templates to render, got {refused:?}"
    );
}

#[test]
fn render_refuses_an_unknown_name() {
    let templates = with_prompt("review", "review {{file}}");
    let refused = templates.render(None, &request("nothing-here", &[]));
    assert!(
        matches!(refused, Err(RenderError::TemplateNotFound)),
        "expected the unknown name to be refused, got {refused:?}"
    );
}

#[test]
fn every_name_render_fills_is_a_name_the_caller_was_asked_for() {
    // The one guarantee the shared scan buys: the reported list and the substituted list are the
    // same derivation, so filling exactly what was reported leaves nothing behind.
    let body = r"{{a}} \{{b}} {{ c }} {{a}} {{}} {{d{{e}} {{f}}";
    let declared = placeholders(body);
    let filled: Vec<(&str, &str)> = declared.iter().map(|name| (name.as_str(), "V")).collect();
    let rendered = render(body, &filled);
    assert!(rendered.unfilled.is_empty());
    assert!(rendered.unknown.is_empty());
    assert!(
        !rendered.text.contains("V}}"),
        "a reported name was left unsubstituted: {}",
        rendered.text
    );
}
