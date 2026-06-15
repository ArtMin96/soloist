//! Per-process terminal buffers: a bounded raw byte scrollback and a bounded rendered
//! line scrollback, both maintained from a single PTY read stream.
//!
//! The two views answer different needs (ref the parity matrix): the **raw** buffer is
//! replayed verbatim to a terminal emulator on attach and exposes control sequences;
//! the **rendered** buffer is the plain-text projection for logs, search, and
//! `get_process_output`. Keeping one read loop drive both is what avoids divergence.

use std::collections::VecDeque;

use vte::Parser;

use super::parser::Renderer;
use super::ring::Ring;
use super::{LogLine, RenderedScreen, TerminalSignal};

/// Default raw scrollback cap in bytes: enough to replay a recent screen on attach,
/// bounded so a chatty process cannot grow it without limit.
const RAW_SCROLLBACK_BYTES: usize = 256 * 1024;
/// Default rendered scrollback cap in lines: the logs/search history depth.
const LOG_LINES: usize = 5_000;

/// A byte buffer capped at `cap` bytes that drops the oldest bytes once exceeded — the
/// verbatim, escape-sequence-preserving record of what a process emitted.
struct RawScrollback {
    cap: usize,
    bytes: VecDeque<u8>,
}

impl RawScrollback {
    fn new(cap: usize) -> Self {
        Self {
            cap: cap.max(1),
            bytes: VecDeque::new(),
        }
    }

    fn extend(&mut self, data: &[u8]) {
        self.bytes.extend(data.iter().copied());
        if self.bytes.len() > self.cap {
            let excess = self.bytes.len() - self.cap;
            self.bytes.drain(..excess);
        }
    }

    fn to_vec(&self) -> Vec<u8> {
        self.bytes.iter().copied().collect()
    }
}

/// Both views of one process's terminal output, maintained together: a raw byte
/// scrollback (verbatim, for replay) and a rendered line scrollback (escape sequences
/// applied, for plain text). Both are bounded; the parser carries state across chunks
/// so a sequence split over two reads is still decoded correctly.
pub(crate) struct TerminalBuffers {
    raw: RawScrollback,
    log: Ring<LogLine>,
    line: Vec<char>,
    cursor: usize,
    parser: Parser,
}

impl Default for TerminalBuffers {
    fn default() -> Self {
        Self::new(RAW_SCROLLBACK_BYTES, LOG_LINES)
    }
}

impl TerminalBuffers {
    /// Buffers with explicit raw-byte and rendered-line caps. The defaults cover the
    /// production path; tests use small caps to exercise eviction.
    pub(crate) fn new(raw_cap: usize, log_cap: usize) -> Self {
        Self {
            raw: RawScrollback::new(raw_cap),
            log: Ring::new(log_cap),
            line: Vec::new(),
            cursor: 0,
            parser: Parser::new(),
        }
    }

    /// Feeds a chunk of raw PTY bytes through both buffers, returning the semantic
    /// signals (title changes, bells) observed in it. The raw scrollback records the
    /// bytes verbatim; the rendered line model advances over the same bytes.
    pub(crate) fn ingest(&mut self, data: &[u8]) -> Vec<TerminalSignal> {
        self.raw.extend(data);
        let Self {
            log,
            line,
            cursor,
            parser,
            ..
        } = self;
        let mut renderer = Renderer {
            line,
            cursor,
            log,
            signals: Vec::new(),
        };
        parser.advance(&mut renderer, data);
        renderer.signals
    }

    /// The raw byte scrollback, for verbatim replay to a terminal emulator on attach.
    pub(crate) fn raw(&self) -> Vec<u8> {
        self.raw.to_vec()
    }

    /// The rendered output: every retained scrollback line plus the in-progress line.
    pub(crate) fn rendered(&self) -> RenderedScreen {
        let mut lines: Vec<String> = self.log.iter().map(|l| l.text.clone()).collect();
        if !self.line.is_empty() {
            lines.push(self.line.iter().collect());
        }
        RenderedScreen { lines }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut b = TerminalBuffers::new(64 * 1024, 3);
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
    fn the_raw_scrollback_never_exceeds_its_byte_cap() {
        let mut b = TerminalBuffers::new(8, 5_000);
        ingest(&mut b, b"0123456789");
        // Capped to the most recent 8 bytes.
        assert_eq!(b.raw(), b"23456789".to_vec());
    }

    #[test]
    fn an_osc_title_and_a_bell_surface_as_signals() {
        let mut b = TerminalBuffers::default();
        let signals = ingest(&mut b, b"\x1b]0;my title\x07ding\x07");
        assert!(signals
            .iter()
            .any(|s| matches!(s, TerminalSignal::Title(t) if t == "my title")));
        assert!(signals.iter().any(|s| matches!(s, TerminalSignal::Bell)));
    }
}
