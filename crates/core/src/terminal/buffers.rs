//! Per-process terminal buffers: a bounded raw byte scrollback and a bounded rendered
//! line scrollback, both maintained from a single PTY read stream.
//!
//! The two views answer different needs (ref the parity matrix): the **raw** buffer is
//! replayed verbatim to a terminal emulator on attach and exposes control sequences;
//! the **rendered** buffer is the plain-text projection for logs, search, and
//! `get_process_output`. Keeping one read loop drive both is what avoids divergence.
//!
//! Memory is bounded twice: each process's raw scrollback has its own byte cap, and a
//! [`ScrollbackBudget`] shared across every process caps the *aggregate* raw bytes, so a
//! fleet of chatty processes cannot grow memory without limit even when each stays under
//! its own cap.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use vte::Parser;

use super::parser::Renderer;
use super::ring::Ring;
use super::{LogLine, RenderedScreen, TerminalSignal};

/// Default raw scrollback cap in bytes: enough to replay a recent screen on attach,
/// bounded so a chatty process cannot grow it without limit.
const RAW_SCROLLBACK_BYTES: usize = 256 * 1024;
/// Default rendered scrollback cap in lines: the logs/search history depth.
const LOG_LINES: usize = 5_000;
/// Default aggregate raw-scrollback cap across *all* processes. Sized so a typical
/// fleet (each well under its own 256 KB cap) never trims, while an extreme number of
/// chatty processes is still bounded — the global ceiling the longevity rules require.
const GLOBAL_RAW_SCROLLBACK_BYTES: usize = 16 * 1024 * 1024;

/// A counter of total raw scrollback bytes across every process, with a global cap.
/// Shared (behind an `Arc`) by every [`TerminalBuffers`] a supervisor creates: each
/// buffer adds the bytes it retains and releases them on drop, and sheds its own oldest
/// bytes when the aggregate is over budget. Lock-free — a relaxed atomic is enough for a
/// soft memory ceiling.
pub(crate) struct ScrollbackBudget {
    total: AtomicUsize,
    cap: usize,
}

impl ScrollbackBudget {
    /// A budget capping the aggregate raw scrollback at `cap` bytes (clamped to ≥ 1).
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            total: AtomicUsize::new(0),
            cap: cap.max(1),
        }
    }

    fn add(&self, n: usize) {
        self.total.fetch_add(n, Ordering::Relaxed);
    }

    fn sub(&self, n: usize) {
        self.total.fetch_sub(n, Ordering::Relaxed);
    }

    /// How many bytes the aggregate currently exceeds the global cap by (0 if under).
    fn overflow(&self) -> usize {
        self.total.load(Ordering::Relaxed).saturating_sub(self.cap)
    }
}

impl Default for ScrollbackBudget {
    fn default() -> Self {
        Self::new(GLOBAL_RAW_SCROLLBACK_BYTES)
    }
}

/// A byte buffer capped at `cap` bytes that drops the oldest bytes once exceeded — the
/// verbatim, escape-sequence-preserving record of what a process emitted. It also
/// accounts its retained bytes against a shared [`ScrollbackBudget`], shedding more of
/// its own oldest bytes when the global aggregate is over budget.
struct RawScrollback {
    cap: usize,
    bytes: VecDeque<u8>,
    budget: Arc<ScrollbackBudget>,
}

impl RawScrollback {
    fn new(cap: usize, budget: Arc<ScrollbackBudget>) -> Self {
        Self {
            cap: cap.max(1),
            bytes: VecDeque::new(),
            budget,
        }
    }

    fn extend(&mut self, data: &[u8]) {
        self.bytes.extend(data.iter().copied());
        self.budget.add(data.len());
        // This process's own cap: drop the oldest bytes beyond its ceiling.
        if self.bytes.len() > self.cap {
            self.drop_front(self.bytes.len() - self.cap);
        }
        // Global cap: when the aggregate across all processes is over budget, the
        // writing buffer sheds its oldest bytes until the total is back under.
        let overflow = self.budget.overflow().min(self.bytes.len());
        if overflow > 0 {
            self.drop_front(overflow);
        }
    }

    /// Drops the `n` oldest bytes, keeping the shared budget in step.
    fn drop_front(&mut self, n: usize) {
        self.bytes.drain(..n);
        self.budget.sub(n);
    }

    fn to_vec(&self) -> Vec<u8> {
        self.bytes.iter().copied().collect()
    }

    fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl Drop for RawScrollback {
    fn drop(&mut self) {
        // Release this process's retained bytes from the shared budget when its
        // terminal buffers go away (e.g. replaced on a fresh start), so the aggregate
        // reflects only live buffers.
        self.budget.sub(self.bytes.len());
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
    /// A monotonic count of bytes ever ingested — the cheap output-activity signal the
    /// idle classifier polls (it compares successive values, so the absolute value and a
    /// relaunch reusing these buffers are both fine).
    output_seq: u64,
    /// The most recent OSC terminal title the process set — the signal the title-based
    /// idle heuristics read.
    last_title: Option<String>,
}

impl Default for TerminalBuffers {
    fn default() -> Self {
        // A standalone budget — for unit tests that exercise one buffer in isolation.
        Self::new(
            RAW_SCROLLBACK_BYTES,
            LOG_LINES,
            Arc::new(ScrollbackBudget::default()),
        )
    }
}

impl TerminalBuffers {
    /// Buffers with explicit raw-byte and rendered-line caps over a shared budget. The
    /// defaults cover the production path; tests use small caps to exercise eviction.
    pub(crate) fn new(raw_cap: usize, log_cap: usize, budget: Arc<ScrollbackBudget>) -> Self {
        Self {
            raw: RawScrollback::new(raw_cap, budget),
            log: Ring::new(log_cap),
            line: Vec::new(),
            cursor: 0,
            parser: Parser::new(),
            output_seq: 0,
            last_title: None,
        }
    }

    /// Production buffers: default per-process caps, sharing the supervisor-wide raw
    /// scrollback `budget` so total memory across all processes is bounded too.
    pub(crate) fn shared(budget: Arc<ScrollbackBudget>) -> Self {
        Self::new(RAW_SCROLLBACK_BYTES, LOG_LINES, budget)
    }

    /// Feeds a chunk of raw PTY bytes through both buffers, returning the semantic
    /// signals (title changes, bells) observed in it. The raw scrollback records the
    /// bytes verbatim; the rendered line model advances over the same bytes.
    pub(crate) fn ingest(&mut self, data: &[u8]) -> Vec<TerminalSignal> {
        self.raw.extend(data);
        self.output_seq = self.output_seq.saturating_add(data.len() as u64);
        let signals = {
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
        };
        // Retain the latest title so a poll can read it without replaying the stream.
        if let Some(title) = signals.iter().rev().find_map(|signal| match signal {
            TerminalSignal::Title(title) => Some(title.clone()),
            TerminalSignal::Bell => None,
        }) {
            self.last_title = Some(title);
        }
        signals
    }

    /// A monotonic byte count of all output ingested over this process's life — the
    /// output-activity signal the idle classifier compares between samples.
    pub(crate) fn output_seq(&self) -> u64 {
        self.output_seq
    }

    /// The most recent OSC terminal title set, if any — read by the title-based idle
    /// heuristics.
    pub(crate) fn last_title(&self) -> Option<String> {
        self.last_title.clone()
    }

    /// The raw byte scrollback, for verbatim replay to a terminal emulator on attach.
    pub(crate) fn raw(&self) -> Vec<u8> {
        self.raw.to_vec()
    }

    /// Whether any output has been recorded yet. Used to decide whether a relaunch
    /// should mark a restart boundary: there is nothing to separate on the first run.
    pub(crate) fn has_output(&self) -> bool {
        !self.raw.is_empty()
    }

    /// The rendered output: every retained scrollback line plus the in-progress line.
    pub(crate) fn rendered(&self) -> RenderedScreen {
        let mut lines: Vec<String> = self.log.iter().map(|l| l.text.clone()).collect();
        if !self.line.is_empty() {
            lines.push(self.line.iter().collect());
        }
        RenderedScreen { lines }
    }

    /// The most recent `n` rendered lines, oldest first — the committed log tail plus the
    /// in-progress line (where a not-yet-newline-terminated prompt sits). Reads only the
    /// tail rather than cloning the whole scrollback, for the idle classifier's frequent
    /// polling.
    pub(crate) fn tail(&self, n: usize) -> Vec<String> {
        if n == 0 {
            return Vec::new();
        }
        let has_partial = !self.line.is_empty();
        let from_log = if has_partial { n - 1 } else { n };
        let mut lines: Vec<String> = self.log.tail(from_log).map(|l| l.text.clone()).collect();
        if has_partial {
            lines.push(self.line.iter().collect());
        }
        lines
    }
}

#[cfg(test)]
mod tests {
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
}
