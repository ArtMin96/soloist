//! The terminal stream renderer: a [`vte::Perform`] that turns raw PTY bytes into a
//! line-oriented rendered model.
//!
//! `vte` owns the hard part — a correct, incremental UTF-8 + escape-sequence state
//! machine that survives sequences split across reads — and calls back into
//! [`Renderer`] with decoded printable characters, control bytes, and OSC strings.
//! The rendered model is deliberately line-oriented rather than a full cell grid: the
//! raw scrollback preserves every byte for a true terminal emulator (xterm.js), while
//! this rendered view is the plain-text projection that logs, search, and
//! `get_process_output` consume.

use base64::Engine as _;
use vte::Perform;

use super::ring::Ring;
use super::{LogLine, TerminalSignal};

/// Tab stops every eight columns — the conventional terminal default.
const TAB_WIDTH: usize = 8;

/// The most characters the in-progress line may hold before it is force-flushed, so a
/// process that prints megabytes with no newline cannot grow the current line without
/// bound (the scrollback itself is separately capped).
const MAX_LINE_CHARS: usize = 64 * 1024;

/// The most characters a script-supplied notification title may carry. A heading longer than
/// this is truncated rather than copied: the text is arbitrary process output, and no surface
/// shows more than a short heading anyway.
const MAX_NOTIFY_TITLE_CHARS: usize = 120;
/// The most characters a script-supplied notification message may carry, truncated for the same
/// reason a title is.
const MAX_NOTIFY_BODY_CHARS: usize = 1_000;
/// The widest a single character encodes to in UTF-8, so this many bytes per character always
/// cover a character cap however the payload is encoded.
const MAX_UTF8_CHAR_BYTES: usize = 4;

/// Applies the printable text and control effects of a byte stream to a rendered line
/// model and collects the semantic [`TerminalSignal`]s (title, bell) it observes.
///
/// The rendering rules — the heart of how output looks — are:
/// * a **printable character** overwrites at the cursor, or extends the line when the
///   cursor is at its end, then advances the cursor;
/// * a **carriage return** (`\r`) moves the cursor to column zero without clearing, so
///   a progress bar or spinner redrawn on the same line overwrites in place;
/// * a **newline** (`\n`) flushes the current line into the scrollback and starts a
///   fresh one;
/// * a **tab** (`\t`) advances to the next tab stop, padding with spaces;
/// * a **bell** (`BEL`) and an **OSC title** set are surfaced as signals;
/// * colour/cursor escape sequences are consumed without leaking into the text.
pub(super) struct Renderer<'a> {
    pub line: &'a mut Vec<char>,
    pub cursor: &'a mut usize,
    pub log: &'a mut Ring<LogLine>,
    pub signals: Vec<TerminalSignal>,
}

impl Renderer<'_> {
    /// Commits the current line to the scrollback and resets to a fresh, empty line.
    fn flush_line(&mut self) {
        let text: String = self.line.iter().collect();
        self.log.push(LogLine { text });
        self.line.clear();
        *self.cursor = 0;
    }

    /// Records a notification a process raised for itself, dropping one with no message: an
    /// empty body would reach the user as a notification with nothing to say. An empty title is
    /// no title, so the surface names the process instead of showing a blank heading.
    fn notify(&mut self, title: Option<String>, body: String) {
        if body.is_empty() {
            return;
        }
        self.signals.push(TerminalSignal::Notify {
            title: title.filter(|title| !title.is_empty()),
            body,
        });
    }
}

impl Perform for Renderer<'_> {
    fn print(&mut self, c: char) {
        if *self.cursor < self.line.len() {
            self.line[*self.cursor] = c;
        } else {
            self.line.push(c);
        }
        *self.cursor += 1;
        if self.line.len() >= MAX_LINE_CHARS {
            self.flush_line();
        }
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => self.flush_line(),
            b'\r' => *self.cursor = 0,
            b'\t' => {
                // The number of spaces is fixed before the first one is printed: `print`
                // flushes the line at `MAX_LINE_CHARS` and resets the cursor to zero, so a
                // loop watching the cursor for a stop it had already passed would never end.
                let pad = TAB_WIDTH - *self.cursor % TAB_WIDTH;
                for _ in 0..pad {
                    self.print(' ');
                }
            }
            0x07 => self.signals.push(TerminalSignal::Bell),
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        let Some((kind, rest)) = params.split_first() else {
            return;
        };
        match *kind {
            // OSC 0 (icon name + window title), 1 (icon name), and 2 (window title) all
            // carry a title string in the second parameter.
            b"0" | b"1" | b"2" => {
                if let [title, ..] = rest {
                    self.signals.push(TerminalSignal::Title(
                        String::from_utf8_lossy(title).into_owned(),
                    ));
                }
            }
            // OSC 9 (iTerm2-compatible) carries a message and nothing else.
            b"9" => self.notify(None, bounded_text(rest, MAX_NOTIFY_BODY_CHARS)),
            // OSC 777 (libnotify-compatible) carries a title and a message, under a sub-command
            // that must be `notify` — any other is a different feature, not a notification.
            b"777" => {
                if let [sub, title, body @ ..] = rest {
                    if matches!(*sub, b"notify") {
                        self.notify(
                            Some(bounded_text(&[title], MAX_NOTIFY_TITLE_CHARS)),
                            bounded_text(body, MAX_NOTIFY_BODY_CHARS),
                        );
                    }
                }
            }
            // OSC 99 (Kitty-compatible) carries metadata describing the payload that follows.
            b"99" => {
                if let [metadata, payload @ ..] = rest {
                    if let Some(body) = kitty_message(metadata, payload) {
                        self.notify(None, body);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Decodes the notification message spread across `params`, rejoined with the `;` that split
/// them and truncated to `chars` characters.
///
/// The message is the last field of every one of these sequences, so it runs to the end rather
/// than stopping at a separator a script wrote inside it. Only the leading bytes the cap can use
/// are copied: the payload is arbitrary process output, and a process that emits megabytes must
/// not make the parser materialize them.
fn bounded_text(params: &[&[u8]], chars: usize) -> String {
    let budget = byte_budget(chars);
    let mut bytes: Vec<u8> = Vec::new();
    for (n, param) in params.iter().enumerate() {
        if n > 0 {
            bytes.push(b';');
        }
        let room = budget.saturating_sub(bytes.len());
        if room == 0 {
            break;
        }
        bytes.extend_from_slice(&param[..param.len().min(room)]);
    }
    String::from_utf8_lossy(&bytes)
        .chars()
        .take(chars)
        .collect()
}

/// How many bytes always cover a `chars`-character cap, whatever those characters are.
fn byte_budget(chars: usize) -> usize {
    chars.saturating_mul(MAX_UTF8_CHAR_BYTES)
}

/// The message a one-shot OSC 99 sequence carries, or `None` when it carries none.
///
/// The metadata is colon-separated `key=value` pairs. A payload that is a chunk of a multipart
/// notification (`i=<id>`, or `d=0` for "more follows") is ignored outright rather than
/// half-assembled, and so is one that is not the notification's text at all — an icon is image
/// bytes, and a close request or a button list has no message to show.
///
/// Kitty defaults an unlabelled payload to the notification's title; Soloist reads it as the
/// message either way, because a surface that renders this always has a title to fall back on
/// (the process's own label) but nothing to say without a message.
fn kitty_message(metadata: &[u8], payload: &[&[u8]]) -> Option<String> {
    let mut base64 = false;
    for field in metadata.split(|byte| *byte == b':') {
        let mut halves = field.splitn(2, |byte| *byte == b'=');
        let key = halves.next()?;
        let value = halves.next().unwrap_or_default();
        match key {
            b"i" => return None,
            b"d" if value == b"0" => return None,
            b"e" => base64 = value == b"1",
            b"p" if !matches!(value, b"" | b"title" | b"body") => return None,
            _ => {}
        }
    }
    if !base64 {
        return Some(bounded_text(payload, MAX_NOTIFY_BODY_CHARS));
    }
    // A base64 payload has no separators of its own, so it is one parameter. Truncating it on a
    // whole-quantum boundary keeps the prefix decodable, so an outsized message is shortened
    // like a plain one instead of being discarded for being too long.
    let encoded = payload.first().copied().unwrap_or_default();
    let budget = byte_budget(MAX_NOTIFY_BODY_CHARS).div_ceil(3) * 4;
    let encoded = &encoded[..encoded.len().min(budget)];
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    Some(bounded_text(&[&decoded], MAX_NOTIFY_BODY_CHARS))
}

#[cfg(test)]
#[path = "parser_tests.rs"]
mod tests;
