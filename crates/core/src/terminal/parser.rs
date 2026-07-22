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

use vte::Perform;

use super::ring::Ring;
use super::{LogLine, TerminalSignal};

/// Tab stops every eight columns — the conventional terminal default.
const TAB_WIDTH: usize = 8;

/// The most characters the in-progress line may hold before it is force-flushed, so a
/// process that prints megabytes with no newline cannot grow the current line without
/// bound (the scrollback itself is separately capped).
const MAX_LINE_CHARS: usize = 64 * 1024;

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
        // OSC 0 (icon name + window title), 1 (icon name), and 2 (window title) all
        // carry a title string in the second parameter.
        if let [kind, title, ..] = params {
            if matches!(*kind, b"0" | b"1" | b"2") {
                self.signals.push(TerminalSignal::Title(
                    String::from_utf8_lossy(title).into_owned(),
                ));
            }
        }
    }
}

#[cfg(test)]
#[path = "parser_tests.rs"]
mod tests;
