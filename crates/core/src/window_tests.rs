use super::*;

/// The pair of quotes `serde_json` wraps a string in, which is not part of the body's cost.
const SURROUNDING_QUOTES: usize = 2;

/// Between them these hold every escape JSON draws a distinction between: the characters with a
/// letter of their own, the control characters without one, a character JSON leaves alone
/// despite being outside ASCII, and one it leaves alone despite being a delete.
const ESCAPING_SAMPLES: [&str; 6] = [
    "plain ascii text",
    "a \"quoted\" and a back\\slash",
    "tab\there and a line\nbreak\r",
    "backspace \u{8}, form feed \u{c}",
    "bell \u{7}, escape \u{1b}, unit separator \u{1f}",
    "café, 🙂, and \u{7f}",
];

/// A text no single chunk of [`SPLIT_CAPACITY`] holds, one line of which is itself too long for
/// a chunk of its own.
const SPLIT_LINES: [&str; 4] = ["alpha", "bravo", "charlie delta echo foxtrot", "golf"];
const SPLIT_CAPACITY: usize = 10;

/// The text one piece covers.
fn piece_text<'a>(piece: &Piece, lines: &[&'a str]) -> &'a str {
    let line: &'a str = lines[piece.line];
    &line[piece.bytes.clone()]
}

/// The body of one planned chunk, its pieces joined the way the body joins them.
fn body_text(planned: &Planned, lines: &[&str]) -> String {
    planned
        .body
        .iter()
        .map(|piece| piece_text(piece, lines))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every chunk in turn, each reached through the window the one before it handed out, rejoined
/// by the rule a reader follows: a chunk that begins part-way through a line continues the one
/// before it rather than starting a new one.
fn reassembled(lines: &[&str], capacity: usize, range: Option<LineRange>) -> String {
    let mut text = String::new();
    let mut window = ReadWindow {
        chunk: FIRST_CHUNK,
        range,
    };
    loop {
        let planned = plan(lines, &window, capacity).expect("every planned chunk is retrievable");
        if window.chunk > FIRST_CHUNK && !planned.chunk.cut.starts_mid_line {
            text.push('\n');
        }
        text.push_str(&body_text(&planned, lines));
        match planned.chunk.next {
            Some(next) => window = next,
            None => return text,
        }
    }
}

#[test]
fn a_character_costs_what_serde_charges_to_escape_it() {
    for sample in ESCAPING_SAMPLES {
        let encoded = serde_json::to_string(sample).expect("a string always encodes");
        let modelled: usize = sample.chars().map(escaped_char_cost).sum();

        assert_eq!(
            encoded.len() - SURROUNDING_QUOTES,
            modelled,
            "{sample:?} encodes as {encoded:?}"
        );
    }
}

#[test]
fn a_text_within_the_capacity_is_a_single_chunk() {
    let lines = ["alpha", "bravo", "charlie"];

    let planned = plan(&lines, &ReadWindow::default(), 1_000).expect("plans");

    assert_eq!(planned.chunk.index, FIRST_CHUNK);
    assert_eq!(planned.chunk.of, 1);
    assert_eq!(planned.chunk.next, None);
    assert_eq!(planned.chunk.lines, LineRange { from: 1, to: 3 });
    assert_eq!(planned.chunk.total_lines, 3);
    assert_eq!(planned.chunk.cut, Cut::default());
    assert_eq!(planned.chunk.total_bytes, lines.join("\n").len());
    assert_eq!(planned.chunk.bytes, planned.chunk.total_bytes);
    assert_eq!(body_text(&planned, &lines), lines.join("\n"));
}

#[test]
fn the_newline_between_two_lines_is_charged_against_the_capacity() {
    let lines = ["aaaa", "bbbb", "ccc"];
    let capacity = lines.iter().map(|line| line.len()).sum::<usize>();

    let planned = plan(&lines, &ReadWindow::default(), capacity).expect("plans");

    assert_eq!(planned.chunk.lines, LineRange { from: 1, to: 2 });
    assert_eq!(planned.chunk.of, 2);
    assert_eq!(
        planned.chunk.next,
        Some(ReadWindow {
            chunk: 2,
            range: None
        })
    );
}

#[test]
fn every_chunk_of_a_split_text_is_retrievable_and_together_they_are_the_text() {
    let first = plan(&SPLIT_LINES, &ReadWindow::default(), SPLIT_CAPACITY).expect("plans");
    assert!(first.chunk.of > 1, "the text should not fit one chunk");

    for index in FIRST_CHUNK..=first.chunk.of {
        let window = ReadWindow {
            chunk: index,
            range: None,
        };
        let planned = plan(&SPLIT_LINES, &window, SPLIT_CAPACITY).expect("chunk is retrievable");

        assert_eq!(planned.chunk.index, index);
        assert_eq!(planned.chunk.of, first.chunk.of);
        assert_eq!(planned.chunk.bytes, body_text(&planned, &SPLIT_LINES).len());
    }

    assert_eq!(
        reassembled(&SPLIT_LINES, SPLIT_CAPACITY, None),
        SPLIT_LINES.join("\n")
    );
}

#[test]
fn a_chunk_the_plan_does_not_hold_is_refused() {
    let of = plan(&SPLIT_LINES, &ReadWindow::default(), SPLIT_CAPACITY)
        .expect("plans")
        .chunk
        .of;

    let past_the_end = ReadWindow {
        chunk: of + 1,
        range: None,
    };
    let before_the_start = ReadWindow {
        chunk: 0,
        range: None,
    };

    assert_eq!(
        plan(&SPLIT_LINES, &past_the_end, SPLIT_CAPACITY),
        Err(WindowError::ChunkOutOfRange {
            requested: of + 1,
            of
        })
    );
    assert_eq!(
        plan(&SPLIT_LINES, &before_the_start, SPLIT_CAPACITY),
        Err(WindowError::ChunkOutOfRange { requested: 0, of })
    );
}

#[test]
fn an_oversize_line_is_cut_between_characters_and_continues_in_the_next_chunk() {
    let line = "aaéaa🙂bbb";
    let lines = [line];
    let capacity = 8;

    let first = plan(&lines, &ReadWindow::default(), capacity).expect("plans");
    let next = first.chunk.next.expect("a chunk follows");
    let second = plan(&lines, &next, capacity).expect("plans");

    assert_eq!(first.chunk.of, 2);
    assert_eq!(first.chunk.lines, LineRange { from: 1, to: 1 });
    assert_eq!(second.chunk.lines, LineRange { from: 1, to: 1 });
    assert_eq!(
        first.chunk.cut,
        Cut {
            starts_mid_line: false,
            ends_mid_line: true
        }
    );
    assert_eq!(
        second.chunk.cut,
        Cut {
            starts_mid_line: true,
            ends_mid_line: false
        }
    );
    for planned in [&first, &second] {
        for piece in &planned.body {
            assert!(
                line.is_char_boundary(piece.bytes.start),
                "{piece:?} starts inside a character"
            );
            assert!(
                line.is_char_boundary(piece.bytes.end),
                "{piece:?} ends inside a character"
            );
        }
    }
    assert_eq!(reassembled(&lines, capacity, None), line);
}

#[test]
fn a_cut_follows_what_a_line_costs_escaped_rather_than_the_bytes_it_occupies() {
    let line = "\"\\".repeat(5);
    let lines = [line.as_str()];
    let capacity = line.len();

    let first = plan(&lines, &ReadWindow::default(), capacity).expect("plans");

    assert_eq!(first.chunk.of, 2);
    assert_eq!(first.chunk.bytes, 5);
    assert_eq!(body_text(&first, &lines), &line[..5]);
    assert!(first.chunk.cut.ends_mid_line);
    assert_eq!(reassembled(&lines, capacity, None), line);
}

#[test]
fn a_range_narrows_what_is_planned_but_not_how_it_is_numbered() {
    let lines = ["one", "two", "three", "four", "five"];
    let range = LineRange { from: 2, to: 4 };
    let window = ReadWindow {
        chunk: FIRST_CHUNK,
        range: Some(range),
    };

    let planned = plan(&lines, &window, 1_000).expect("plans");

    assert_eq!(planned.chunk.of, 1);
    assert_eq!(planned.chunk.next, None);
    assert_eq!(planned.chunk.lines, range);
    assert_eq!(planned.chunk.total_lines, 5);
    assert_eq!(planned.chunk.total_bytes, lines.join("\n").len());
    assert_eq!(planned.chunk.bytes, lines[1..4].join("\n").len());
    assert_eq!(body_text(&planned, &lines), lines[1..4].join("\n"));
}

#[test]
fn the_window_that_reaches_the_next_chunk_keeps_the_range() {
    let lines = ["one", "two", "three", "four", "five"];
    let range = LineRange { from: 2, to: 4 };
    let window = ReadWindow {
        chunk: FIRST_CHUNK,
        range: Some(range),
    };
    let capacity = 9;

    let planned = plan(&lines, &window, capacity).expect("plans");

    assert_eq!(planned.chunk.of, 3);
    assert_eq!(
        planned.chunk.next,
        Some(ReadWindow {
            chunk: 2,
            range: Some(range)
        })
    );
    assert_eq!(
        reassembled(&lines, capacity, Some(range)),
        lines[1..4].join("\n")
    );
}

#[test]
fn a_range_the_text_does_not_have_is_refused() {
    let lines = ["one", "two", "three"];

    for requested in [
        LineRange { from: 0, to: 2 },
        LineRange { from: 3, to: 2 },
        LineRange { from: 1, to: 4 },
    ] {
        let window = ReadWindow {
            chunk: FIRST_CHUNK,
            range: Some(requested),
        };

        assert_eq!(
            plan(&lines, &window, 1_000),
            Err(WindowError::LineRangeOutOfRange {
                requested,
                total_lines: 3
            })
        );
    }
}

#[test]
fn a_text_with_no_lines_is_refused() {
    assert_eq!(
        plan(&[], &ReadWindow::default(), 1_000),
        Err(WindowError::Empty)
    );
}

#[test]
fn a_capacity_no_chunk_could_advance_under_is_refused() {
    let lines = ["a"];

    assert_eq!(
        plan(&lines, &ReadWindow::default(), MIN_CAPACITY - 1),
        Err(WindowError::NoCapacity {
            capacity: MIN_CAPACITY - 1,
            minimum: MIN_CAPACITY
        })
    );
}

#[test]
fn the_widest_characters_still_advance_a_chunk_at_a_time_at_the_smallest_capacity() {
    let line = "\u{1}\u{2}\u{3}";
    let lines = [line];

    let planned = plan(&lines, &ReadWindow::default(), MIN_CAPACITY).expect("plans");

    assert_eq!(planned.chunk.of, 3);
    assert_eq!(planned.chunk.bytes, 1);
    assert_eq!(reassembled(&lines, MIN_CAPACITY, None), line);
}

#[test]
fn planning_the_same_window_twice_answers_the_same() {
    let window = ReadWindow {
        chunk: 3,
        range: None,
    };

    let once = plan(&SPLIT_LINES, &window, SPLIT_CAPACITY).expect("plans");
    let again = plan(&SPLIT_LINES, &window, SPLIT_CAPACITY).expect("plans");

    assert_eq!(once, again);
}
