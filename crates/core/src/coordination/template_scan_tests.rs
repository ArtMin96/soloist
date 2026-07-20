use super::*;

/// The text every token stands for, concatenated — a scan that loses or duplicates a byte of the
/// body shows up here even when no placeholder is affected.
fn as_written(body: &str) -> String {
    scan(body).map(Token::verbatim).collect()
}

/// The placeholder names in the order the scan meets them, duplicates kept.
fn scanned_names(body: &str) -> Vec<&str> {
    scan(body)
        .filter_map(|token| match token {
            Token::Placeholder { name, .. } => Some(name),
            _ => None,
        })
        .collect()
}

#[test]
fn a_body_without_markers_is_one_literal() {
    assert_eq!(
        scan("no markers at all").collect::<Vec<_>>(),
        vec![Token::Literal("no markers at all")]
    );
    assert_eq!(scan("").next(), None);
}

#[test]
fn a_marker_splits_the_body_into_literal_placeholder_literal() {
    assert_eq!(
        scan("Fix {{ bug }} now").collect::<Vec<_>>(),
        vec![
            Token::Literal("Fix "),
            Token::Placeholder {
                name: "bug",
                raw: "{{ bug }}"
            },
            Token::Literal(" now"),
        ]
    );
}

#[test]
fn an_escaped_marker_is_literal_text_and_names_nothing() {
    assert_eq!(
        scan(r"\{{x}}").collect::<Vec<_>>(),
        vec![Token::EscapedOpen, Token::Literal("x}}")]
    );
    assert_eq!(as_written(r"\{{x}}"), "{{x}}");
    assert_eq!(scanned_names(r"\{{x}}"), Vec::<&str>::new());
}

#[test]
fn a_doubled_backslash_is_a_literal_backslash_before_a_real_placeholder() {
    assert_eq!(
        scan(r"\\{{x}}").collect::<Vec<_>>(),
        vec![
            Token::Literal(r"\"),
            Token::Placeholder {
                name: "x",
                raw: "{{x}}"
            },
        ]
    );
    assert_eq!(scanned_names(r"\\{{x}}"), vec!["x"]);
}

#[test]
fn a_backslash_that_does_not_precede_a_marker_is_untouched() {
    assert_eq!(
        as_written(r"C:\path\to {{file}} \ end"),
        r"C:\path\to {{file}} \ end"
    );
    assert_eq!(scanned_names(r"C:\path\to {{file}} \ end"), vec!["file"]);
    // A backslash after the marker opens nothing and is not consumed.
    assert_eq!(as_written(r"{{a}}\"), r"{{a}}\");
}

#[test]
fn backslash_runs_pair_off_before_a_marker() {
    // Three: one pair collapses to a literal backslash, the odd one escapes the marker.
    assert_eq!(
        scan(r"\\\{{x}}").collect::<Vec<_>>(),
        vec![
            Token::Literal(r"\"),
            Token::EscapedOpen,
            Token::Literal("x}}"),
        ]
    );
    // Four: two pairs, no escape, a real placeholder.
    assert_eq!(
        scan(r"\\\\{{x}}").collect::<Vec<_>>(),
        vec![
            Token::Literal(r"\\"),
            Token::Placeholder {
                name: "x",
                raw: "{{x}}"
            },
        ]
    );
}

#[test]
fn adjacent_markers_are_two_placeholders_with_no_literal_between_them() {
    assert_eq!(
        scan("{{a}}{{b}}").collect::<Vec<_>>(),
        vec![
            Token::Placeholder {
                name: "a",
                raw: "{{a}}"
            },
            Token::Placeholder {
                name: "b",
                raw: "{{b}}"
            },
        ]
    );
}

#[test]
fn a_single_brace_pair_is_not_a_marker() {
    assert_eq!(
        scan("{x} and {a}{b}").collect::<Vec<_>>(),
        vec![Token::Literal("{x} and {a}{b}")]
    );
}

#[test]
fn a_tripled_brace_keeps_the_extra_brace_out_of_the_name() {
    // `{{{a}}` closes at the first `}}`, and its inner text `{a` still holds a brace, so the span
    // is literal; the trailing `}` after it is literal too.
    assert_eq!(
        scan("{{{a}}}").collect::<Vec<_>>(),
        vec![Token::Literal("{{{a}}"), Token::Literal("}")]
    );
    assert_eq!(as_written("{{{a}}}"), "{{{a}}}");
}

#[test]
fn a_trailing_close_marker_stays_outside_the_placeholder() {
    assert_eq!(
        scan("{{a}}}").collect::<Vec<_>>(),
        vec![
            Token::Placeholder {
                name: "a",
                raw: "{{a}}"
            },
            Token::Literal("}"),
        ]
    );
}

#[test]
fn an_unclosed_marker_is_literal_to_the_end_of_the_body() {
    assert_eq!(
        scan("unclosed {{oops").collect::<Vec<_>>(),
        vec![Token::Literal("unclosed "), Token::Literal("{{oops")]
    );
    assert_eq!(as_written("unclosed {{oops"), "unclosed {{oops");
    // The escape still applies when nothing closes the marker.
    assert_eq!(as_written(r"a \\{{oops"), r"a \{{oops");
}

#[test]
fn an_empty_or_multiline_candidate_is_literal_and_its_span_is_not_rescanned() {
    assert_eq!(
        scan("{{}} {{  }}").collect::<Vec<_>>(),
        vec![
            Token::Literal("{{}}"),
            Token::Literal(" "),
            Token::Literal("{{  }}"),
        ]
    );
    assert_eq!(scanned_names("spans {{a\nb}} lines"), Vec::<&str>::new());
    // The rejected span swallows the inner `{{b`, which is never rescanned.
    assert_eq!(scanned_names("{{a{{b}} c}}"), Vec::<&str>::new());
    assert_eq!(scanned_names("{{a{{b}} then {{ok}}"), vec!["ok"]);
}

#[test]
fn a_non_ascii_name_survives_the_scan_intact() {
    let body = "Проверь {{ файл }} и {{файл}} — «{{résumé}}» 🌍 {{名前}}";
    assert_eq!(scanned_names(body), vec!["файл", "файл", "résumé", "名前"]);
    assert_eq!(as_written(body), body);
}

#[test]
fn a_non_ascii_literal_around_an_escaped_marker_survives_the_scan() {
    let body = "日本語 \\{{名前}} ✅";
    assert_eq!(scanned_names(body), Vec::<&str>::new());
    assert_eq!(as_written(body), "日本語 {{名前}} ✅");
}

#[test]
fn the_scan_reproduces_every_body_it_reads_apart_from_consumed_escapes() {
    for body in [
        "plain text",
        "{{a}}",
        "a {{b}} c {{d}}",
        "{{a}}{{b}}",
        "{{{a}}}",
        "{{a}}}",
        "{{a{{b}} c}}",
        "unclosed {{oops",
        "{{}}",
        "spans {{a\nb}} lines",
        "非ASCII {{名前}} テキスト",
        r"C:\path {{a}}",
    ] {
        assert_eq!(as_written(body), body, "round trip of {body:?}");
    }
}

#[test]
fn scanned_order_matches_the_order_of_appearance() {
    assert_eq!(
        scanned_names("{{z}} then {{a}} then {{z}} then {{m}}"),
        vec!["z", "a", "z", "m"],
        "the stream keeps appearance order and repeats, so its readers cannot reorder"
    );
}
