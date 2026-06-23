//! Read-only terminal-output queries (context C8) — the output surface a remote caller
//! (MCP today, the HTTP API later) reads a process's logs and ports through.
//!
//! These are open reads, like `list_processes`: they expose any process's output unfiltered
//! rather than scope-gating it, matching the rest of the read surface. Each routes to one C2
//! accessor (which reads the C3 terminal buffers) and bounds the reply twice — by line/match
//! *count* and by total *bytes* — so a busy process can never return an unbounded payload. The
//! one buffer *mutation*, `clear_output`, is a scoped action and lives in `scoped.rs`.

use super::Facade;
use crate::ids::ProcessId;

/// Rendered output lines returned when the caller names no `lines` — a useful recent slice
/// without shipping the whole scrollback.
const DEFAULT_OUTPUT_LINES: usize = 1_000;
/// The most rendered lines any one `get_process_output` returns, regardless of the request —
/// the rendered scrollback's own depth, so a caller can read all of it but no more.
const MAX_OUTPUT_LINES: usize = 5_000;
/// Search matches returned when the caller names no `limit`.
const DEFAULT_SEARCH_MATCHES: usize = 100;
/// The most matches any one search returns, so a frequent term cannot return an unbounded reply.
const MAX_SEARCH_MATCHES: usize = 1_000;
/// The most bytes a rendered output or search reply may total. A single rendered line is
/// capped (64 KiB) but a reply can hold thousands, so a line count alone does not bound the
/// payload: without a byte cap an output read on a process with many long lines could exceed
/// the transport's per-message frame and drop the connection. Kept well under that frame
/// limit; lines past it are trimmed. The raw reads need no equivalent — they are already
/// bounded by the much smaller raw-scrollback byte cap.
const MAX_REPLY_BYTES: usize = 1024 * 1024;

/// Which end of a line list survives a [`within_reply_budget`] trim.
#[derive(Clone, Copy)]
enum Keep {
    /// Keep the most recent lines — a rendered tail's natural anchor.
    Newest,
    /// Keep the earliest lines — an ordered match list reads from its start.
    Earliest,
}

/// Trims `lines` so their total size (each line plus a newline) stays within
/// [`MAX_REPLY_BYTES`], dropping from the end `keep` does not anchor. A single line can never
/// exceed the budget (the renderer caps a line well below it), so at least one line always
/// survives when any exist.
fn within_reply_budget(lines: Vec<String>, keep: Keep) -> Vec<String> {
    let ordered: Vec<String> = match keep {
        Keep::Newest => lines.into_iter().rev().collect(),
        Keep::Earliest => lines,
    };
    let mut total = 0usize;
    let mut kept: Vec<String> = Vec::new();
    for line in ordered {
        total += line.len() + 1;
        if total > MAX_REPLY_BYTES {
            break;
        }
        kept.push(line);
    }
    if matches!(keep, Keep::Newest) {
        kept.reverse();
    }
    kept
}

impl Facade {
    /// The most recent rendered output lines of a process (escape sequences applied),
    /// bounded to `lines` (defaulting to [`DEFAULT_OUTPUT_LINES`], capped at
    /// [`MAX_OUTPUT_LINES`]) and to [`MAX_REPLY_BYTES`] total — when both bite, the most
    /// recent lines are kept. `None` if no such process is registered; an empty list if it
    /// is registered but has never started.
    pub fn process_output(&self, id: ProcessId, lines: Option<usize>) -> Option<Vec<String>> {
        self.process_view(id)?;
        let n = lines.unwrap_or(DEFAULT_OUTPUT_LINES).min(MAX_OUTPUT_LINES);
        let tail = self.supervisor().rendered_tail(id, n).unwrap_or_default();
        Some(within_reply_budget(tail, Keep::Newest))
    }

    /// A process's raw byte output (control sequences included), bounded by the raw
    /// scrollback's own byte cap. `None` if no such process is registered; an empty buffer
    /// if it is registered but has never started.
    pub fn process_raw_output(&self, id: ProcessId) -> Option<Vec<u8>> {
        self.process_view(id)?;
        Some(self.supervisor().pty_scrollback(id).unwrap_or_default())
    }

    /// Up to `limit` rendered output lines of a process containing `query` (a case-sensitive
    /// substring), defaulting to [`DEFAULT_SEARCH_MATCHES`], capped at [`MAX_SEARCH_MATCHES`]
    /// and at [`MAX_REPLY_BYTES`] total — when the byte cap bites, the earliest matches are
    /// kept. `None` if no such process is registered.
    pub fn search_output(
        &self,
        id: ProcessId,
        query: &str,
        limit: Option<usize>,
    ) -> Option<Vec<String>> {
        self.process_view(id)?;
        let n = limit
            .unwrap_or(DEFAULT_SEARCH_MATCHES)
            .min(MAX_SEARCH_MATCHES);
        let matches = self
            .supervisor()
            .search_output(id, query, n)
            .unwrap_or_default();
        Some(within_reply_budget(matches, Keep::Earliest))
    }

    /// Up to `limit` raw output lines of a process containing `query`, defaulting to
    /// [`DEFAULT_SEARCH_MATCHES`] and capped at [`MAX_SEARCH_MATCHES`]. No byte cap is needed:
    /// the matches come from the raw scrollback, which is itself byte-capped well under the
    /// reply budget. `None` if no such process is registered.
    pub fn search_raw_output(
        &self,
        id: ProcessId,
        query: &str,
        limit: Option<usize>,
    ) -> Option<Vec<String>> {
        self.process_view(id)?;
        let n = limit
            .unwrap_or(DEFAULT_SEARCH_MATCHES)
            .min(MAX_SEARCH_MATCHES);
        Some(
            self.supervisor()
                .search_raw_output(id, query, n)
                .unwrap_or_default(),
        )
    }

    /// A process's currently discovered listening ports. `None` if no such process is
    /// registered; an empty list if it has none.
    pub fn process_ports(&self, id: ProcessId) -> Option<Vec<u16>> {
        self.process_view(id).map(|view| view.ports)
    }

    /// Acknowledges a terminal-perf flush for a process, returning whether it is registered.
    /// A no-op in Soloist: the rendered and raw buffers are written synchronously as output
    /// is read, so a query always sees the latest — the only output coalescing is the
    /// frontend's per-frame terminal repaint, which never affects what these tools read.
    pub fn flush_terminal_perf(&self, id: ProcessId) -> bool {
        self.process_view(id).is_some()
    }
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
