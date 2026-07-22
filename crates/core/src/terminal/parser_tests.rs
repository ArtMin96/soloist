use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use vte::Parser;

use super::*;

/// A scrollback deep enough that no test loses a line it asserts on.
const LOG_CAP: usize = 16;

/// How long a render may take before a test calls it wedged rather than slow. Generous
/// enough that a loaded machine never trips it: the work under test is bounded by the
/// line cap, which renders in milliseconds.
const RENDER_GRACE: Duration = Duration::from_secs(10);

/// Feeds `bytes` through the renderer and returns every rendered line — those flushed to
/// the scrollback, then the still-in-progress one.
fn render(bytes: &[u8]) -> Vec<String> {
    let mut line: Vec<char> = Vec::new();
    let mut cursor = 0;
    let mut log = Ring::new(LOG_CAP);
    let mut parser = Parser::new();
    {
        let mut renderer = Renderer {
            line: &mut line,
            cursor: &mut cursor,
            log: &mut log,
            signals: Vec::new(),
        };
        parser.advance(&mut renderer, bytes);
    }
    let mut lines: Vec<String> = log.iter().map(|entry| entry.text.clone()).collect();
    lines.push(line.iter().collect());
    lines
}

/// Renders on a worker thread and fails — rather than hanging the suite — if the render
/// has not finished within [`RENDER_GRACE`].
fn render_within_grace(bytes: Vec<u8>) -> Vec<String> {
    let (done, rendered) = mpsc::channel();
    thread::spawn(move || {
        let _ = done.send(render(&bytes));
    });
    rendered
        .recv_timeout(RENDER_GRACE)
        .expect("the render terminates instead of spinning")
}

#[test]
fn a_tab_pads_to_the_next_tab_stop() {
    assert_eq!(render(b"ab\tc"), vec!["ab      c".to_string()]);
    // A cursor already on a stop advances a whole tab width rather than standing still.
    assert_eq!(
        render(b"12345678\tx"),
        vec!["12345678        x".to_string()]
    );
}

#[test]
fn a_tab_that_crosses_the_line_cap_terminates() {
    // The line is one tab stop short of the cap, so expanding the tab reaches it: the
    // flush that follows resets the cursor to zero, which a cursor-watching loop would
    // read as "still short of the stop" forever.
    let mut bytes = vec![b'A'; MAX_LINE_CHARS - TAB_WIDTH];
    bytes.push(b'\t');

    let lines = render_within_grace(bytes);

    // The padding filled the line to the cap, which flushed it; nothing is left over.
    assert_eq!(
        lines.len(),
        2,
        "one flushed line and an empty in-progress one"
    );
    assert_eq!(lines[0].chars().count(), MAX_LINE_CHARS);
    assert!(lines[1].is_empty());
}
