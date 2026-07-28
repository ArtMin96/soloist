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

/// Feeds `bytes` through the renderer and returns the semantic signals it observed.
fn signals(bytes: &[u8]) -> Vec<TerminalSignal> {
    let mut line: Vec<char> = Vec::new();
    let mut cursor = 0;
    let mut log = Ring::new(LOG_CAP);
    let mut parser = Parser::new();
    let mut renderer = Renderer {
        line: &mut line,
        cursor: &mut cursor,
        log: &mut log,
        signals: Vec::new(),
    };
    parser.advance(&mut renderer, bytes);
    renderer.signals
}

/// The one notification `bytes` raised, or `None` when it raised none. Fails when it raised
/// more than one, so a test asserting on "the" notification cannot pass on the first of several.
fn notification(bytes: &[u8]) -> Option<TerminalSignal> {
    let mut signals = signals(bytes);
    assert!(
        signals.len() <= 1,
        "expected at most one signal: {signals:?}"
    );
    signals.pop()
}

fn notify(title: Option<&str>, body: &str) -> Option<TerminalSignal> {
    Some(TerminalSignal::Notify {
        title: title.map(str::to_owned),
        body: body.to_owned(),
    })
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
fn osc_9_yields_a_notify_signal_with_no_title() {
    // The iTerm2-compatible form carries a message and nothing else, so the surface that renders
    // it names the process itself.
    assert_eq!(notification(b"\x1b]9;hello\x07"), notify(None, "hello"));
}

#[test]
fn osc_777_yields_title_and_body() {
    assert_eq!(
        notification(b"\x1b]777;notify;Build;done\x07"),
        notify(Some("Build"), "done"),
    );
    // An empty title is no title, so the surface names the process rather than showing a blank
    // heading above the message.
    assert_eq!(
        notification(b"\x1b]777;notify;;done\x07"),
        notify(None, "done"),
    );
}

#[test]
fn osc_99_one_shot_decodes_plain_and_base64() {
    assert_eq!(
        notification(b"\x1b]99;p=body;Build finished\x07"),
        notify(None, "Build finished"),
    );
    assert_eq!(
        notification(b"\x1b]99;p=body:e=1;QnVpbGQgZmluaXNoZWQ=\x07"),
        notify(None, "Build finished"),
    );
    // A payload of multi-byte characters survives the round trip intact.
    assert_eq!(
        notification(b"\x1b]99;e=1;aMOpbGxv\x07"),
        notify(None, "h\u{e9}llo"),
    );
    // Kitty defaults an unlabelled payload to the title; Soloist reads it as the message either
    // way, so its canonical one-shot reaches the user rather than being dropped for want of a body.
    assert_eq!(
        notification(b"\x1b]99;;Hello world\x07"),
        notify(None, "Hello world"),
    );
    // Metadata a newer protocol version may add is ignored rather than treated as a reason to
    // drop the message, so a script that sets a key Soloist does not know still gets through.
    assert_eq!(
        notification(b"\x1b]99;a=report:u=1;Build finished\x07"),
        notify(None, "Build finished"),
    );
}

#[test]
fn osc_99_multipart_is_ignored() {
    // A chunk of a larger notification: showing it would mean showing half a message.
    assert_eq!(notification(b"\x1b]99;i=1;Build\x07"), None);
    assert_eq!(notification(b"\x1b]99;i=1:p=body;finished\x07"), None);
    // `d=0` says more chunks follow, so this payload is incomplete even with no identifier.
    assert_eq!(notification(b"\x1b]99;d=0;Build\x07"), None);
}

#[test]
fn osc_99_ignores_payloads_that_are_not_the_message() {
    // An icon is base64 image bytes and a close request carries no message at all; rendering
    // either as notification text would show the user garbage.
    assert_eq!(notification(b"\x1b]99;p=icon;\x07"), None);
    assert_eq!(notification(b"\x1b]99;p=icon:e=1;zqk=\x07"), None);
    assert_eq!(notification(b"\x1b]99;p=close;1\x07"), None);
    assert_eq!(notification(b"\x1b]99;p=buttons;Yes\x07"), None);
}

#[test]
fn malformed_osc_does_not_panic() {
    // Every one of these is arbitrary output from an untrusted process: none may raise a
    // notification, and none may take the read loop down with it.
    for bytes in [
        // Truncated: no message at all, then no parameters at all.
        b"\x1b]9\x07".as_slice(),
        b"\x1b]9;\x07",
        b"\x1b]777\x07",
        b"\x1b]777;\x07",
        b"\x1b]777;notify\x07",
        b"\x1b]777;notify;Build\x07",
        // A title with no message reaches the user as a notification with nothing to say.
        b"\x1b]777;notify;Build;\x07",
        b"\x1b]99\x07",
        b"\x1b]99;\x07",
        b"\x1b]99;p=body\x07",
        b"\x1b]99;p=body;\x07",
        // Not the `notify` sub-command, so not a notification — guessing what else it meant
        // would be worse than ignoring it.
        b"\x1b]777;query;Build;done\x07",
        b"\x1b]777;;Build;done\x07",
        // Invalid base64 decodes to nothing rather than to something lossy.
        b"\x1b]99;e=1;not base64!!\x07",
        b"\x1b]99;e=1;QQ\x07",
        b"\x1b]99;e=1;\x07",
        // An OSC number that is not one of the notification sequences.
        b"\x1b]12345;notify;Build;done\x07",
        // Unterminated: the sequence never dispatches, so nothing is half-delivered.
        b"\x1b]777;notify;Build;done",
    ] {
        assert_eq!(
            notification(bytes),
            None,
            "{:?}",
            String::from_utf8_lossy(bytes)
        );
    }
}

#[test]
fn a_message_keeps_the_semicolons_inside_it() {
    // The message is the last field of the sequence, so it runs to the end rather than stopping
    // at the first separator inside it.
    assert_eq!(
        notification(b"\x1b]9;done; 0 errors\x07"),
        notify(None, "done; 0 errors"),
    );
    assert_eq!(
        notification(b"\x1b]777;notify;Build;done; 0 errors\x07"),
        notify(Some("Build"), "done; 0 errors"),
    );
}

#[test]
fn bare_bell_still_yields_bell() {
    assert_eq!(signals(b"\x07").len(), 1);
    assert!(matches!(signals(b"\x07")[0], TerminalSignal::Bell));
}

#[test]
fn oversized_payload_is_truncated() {
    let huge = "x".repeat(MAX_NOTIFY_BODY_CHARS * 4);
    let mut bytes = b"\x1b]777;notify;".to_vec();
    bytes.extend_from_slice(&huge.as_bytes()[..MAX_NOTIFY_TITLE_CHARS * 4]);
    bytes.push(b';');
    bytes.extend_from_slice(huge.as_bytes());
    bytes.push(0x07);

    let Some(TerminalSignal::Notify { title, body }) = notification(&bytes) else {
        panic!("an oversized payload is truncated, not dropped");
    };

    assert_eq!(
        title.map(|t| t.chars().count()),
        Some(MAX_NOTIFY_TITLE_CHARS)
    );
    assert_eq!(body.chars().count(), MAX_NOTIFY_BODY_CHARS);
}

#[test]
fn an_oversized_base64_payload_is_truncated() {
    // Truncating the encoded bytes on a whole-quantum boundary keeps the prefix decodable, so an
    // outsized base64 message is shortened like a plain one rather than discarded whole. The
    // trailing garbage is past the cut and never copied, so it cannot invalidate what did fit —
    // which is what tells this apart from decoding the payload whole and shortening the result.
    let encoded = "QUJD".repeat(MAX_NOTIFY_BODY_CHARS * 2) + "!!!";
    let mut bytes = b"\x1b]99;e=1;".to_vec();
    bytes.extend_from_slice(encoded.as_bytes());
    bytes.push(0x07);

    let Some(TerminalSignal::Notify { body, .. }) = notification(&bytes) else {
        panic!("an oversized base64 payload is truncated, not dropped");
    };

    assert_eq!(body.chars().count(), MAX_NOTIFY_BODY_CHARS);
    assert!(body.starts_with("ABC"));
}

#[test]
fn a_notification_sequence_leaves_no_text_in_the_rendered_line() {
    // The rendered pane is the plain-text projection: the raw scrollback keeps every byte, but a
    // notification sequence must not surface there as garbage.
    assert_eq!(
        render(b"before\x1b]777;notify;Build;done\x07after"),
        vec!["beforeafter".to_string()],
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
