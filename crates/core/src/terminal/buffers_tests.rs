use super::*;

/// Buffers over an effectively unbounded global budget, so a test exercises the
/// per-process caps in isolation.
fn buffers(raw_cap: usize, log_cap: usize) -> TerminalBuffers {
    TerminalBuffers::new(
        raw_cap,
        log_cap,
        Arc::new(ScrollbackBudget::new(usize::MAX)),
    )
}

fn ingest(buffers: &mut TerminalBuffers, bytes: &[u8]) -> Vec<TerminalSignal> {
    buffers.ingest(bytes)
}

#[test]
fn rendered_strips_escapes_while_raw_keeps_them() {
    let mut b = TerminalBuffers::default();
    // A red "hi" followed by a reset, then a newline.
    let stream = b"\x1b[31mhi\x1b[0m\n";
    ingest(&mut b, stream);

    // Rendered text has the colour escapes applied (removed); raw keeps them.
    assert_eq!(b.rendered().lines, vec!["hi".to_string()]);
    assert_eq!(b.raw(), stream.to_vec());
}

#[test]
fn carriage_return_overwrites_in_place_like_a_progress_bar() {
    let mut b = TerminalBuffers::default();
    ingest(&mut b, b"50%\r100%\n");
    // The second write overwrote the first on the same line.
    assert_eq!(b.rendered().lines, vec!["100%".to_string()]);
}

#[test]
fn the_log_ring_never_exceeds_its_cap() {
    // A tiny rendered cap so eviction is observable.
    let mut b = buffers(64 * 1024, 3);
    for n in 0..10 {
        ingest(&mut b, format!("line {n}\n").as_bytes());
    }
    // Only the last three lines are retained.
    assert_eq!(
        b.rendered().lines,
        vec!["line 7", "line 8", "line 9"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
    );
}

#[test]
fn tail_returns_at_most_the_requested_recent_lines() {
    let mut b = TerminalBuffers::default();
    for n in 0..10 {
        ingest(&mut b, format!("line {n}\n").as_bytes());
    }
    // Bounded: never more than asked, and it is the most recent slice.
    assert_eq!(
        b.tail(3),
        vec!["line 7", "line 8", "line 9"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
    );
    // Asking for more than exist returns only what exists, never padding.
    assert_eq!(b.tail(100).len(), 10);
    // Zero lines is empty, not the whole buffer.
    assert!(b.tail(0).is_empty());
}

#[test]
fn the_raw_scrollback_never_exceeds_its_byte_cap() {
    let mut b = buffers(8, 5_000);
    ingest(&mut b, b"0123456789");
    // Capped to the most recent 8 bytes.
    assert_eq!(b.raw(), b"23456789".to_vec());
}

#[test]
fn the_global_budget_bounds_total_raw_bytes_across_buffers() {
    let budget = Arc::new(ScrollbackBudget::new(16));
    let mut a = TerminalBuffers::new(1024, 5_000, budget.clone());
    let mut b = TerminalBuffers::new(1024, 5_000, budget.clone());
    // Neither hits its own 1 KB cap, but the shared 16-byte global cap forces the
    // writers to shed oldest bytes so the aggregate never exceeds it.
    ingest(&mut a, &[b'a'; 10]);
    ingest(&mut b, &[b'b'; 10]);
    assert!(
        a.raw().len() + b.raw().len() <= 16,
        "aggregate raw bytes stay within the global budget"
    );
}

#[test]
fn dropping_a_buffer_frees_its_bytes_from_the_global_budget() {
    let budget = Arc::new(ScrollbackBudget::new(1_000));
    let mut a = TerminalBuffers::new(1024, 5_000, budget.clone());
    ingest(&mut a, &[b'x'; 100]);
    assert_eq!(budget.total.load(Ordering::Relaxed), 100);
    drop(a);
    assert_eq!(
        budget.total.load(Ordering::Relaxed),
        0,
        "a dropped buffer releases its bytes"
    );
}

#[test]
fn an_osc_title_and_a_bell_surface_as_signals() {
    let mut b = TerminalBuffers::default();
    // OSC title set (BEL-terminated), printable text, then a standalone bell.
    let signals = ingest(&mut b, b"\x1b]0;my title\x07ding\x07");
    assert!(signals
        .iter()
        .any(|s| matches!(s, TerminalSignal::Title(t) if t == "my title")));
    // Exactly one bell: the OSC's BEL terminator is consumed as the string
    // terminator, not rung; only the standalone BEL after "ding" rings.
    assert_eq!(
        signals
            .iter()
            .filter(|s| matches!(s, TerminalSignal::Bell))
            .count(),
        1
    );
}

#[test]
fn search_rendered_returns_only_matching_lines_bounded() {
    let mut b = TerminalBuffers::default();
    ingest(&mut b, b"error: boom\nok\nerror: bang\nfine\n");
    // A partial (not-yet-newline) line is part of the visible output and is searched too.
    ingest(&mut b, b"error: tail");

    let hits = b.search_rendered("error", 10);
    assert_eq!(hits, vec!["error: boom", "error: bang", "error: tail"]);
    // A miss returns nothing; the search is case-sensitive.
    assert!(b.search_rendered("ERROR", 10).is_empty());
    // The limit bounds the result to the most we asked for.
    assert_eq!(b.search_rendered("error", 1), vec!["error: boom"]);
}

#[test]
fn search_raw_finds_lines_keeping_control_sequences() {
    let mut b = TerminalBuffers::default();
    // A coloured "error" line: the rendered view drops the escapes, the raw search keeps them.
    ingest(&mut b, b"\x1b[31merror: red\x1b[0m\nplain\n");
    let hits = b.search_raw("error", 10);
    assert_eq!(hits.len(), 1);
    assert!(
        hits[0].contains("\x1b[31m"),
        "the raw match keeps the control sequences around it: {:?}",
        hits[0]
    );
}

#[test]
fn clear_empties_both_views_and_releases_the_budget() {
    let budget = Arc::new(ScrollbackBudget::new(1_000));
    let mut b = TerminalBuffers::new(64 * 1024, 5_000, budget.clone());
    ingest(&mut b, b"keep me\nand me\n");
    let before = b.output_seq();
    assert!(budget.total.load(Ordering::Relaxed) > 0);

    b.clear();

    assert!(b.rendered().lines.is_empty(), "rendered view is emptied");
    assert!(b.raw().is_empty(), "raw scrollback is emptied");
    assert_eq!(
        budget.total.load(Ordering::Relaxed),
        0,
        "cleared bytes are released from the shared budget"
    );
    // The monotonic output counter is preserved so idle detection is unaffected by a clear.
    assert_eq!(b.output_seq(), before);
}
