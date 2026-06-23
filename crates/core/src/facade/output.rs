//! Read-only terminal-output queries (context C8) — the output surface a remote caller
//! (MCP today, the HTTP API later) reads a process's logs and ports through.
//!
//! These are open reads, like `list_processes`: they expose any process's output unfiltered
//! rather than scope-gating it, matching the rest of the read surface. Each routes to one C2
//! accessor (which reads the C3 terminal buffers) and bounds the reply — a tail of lines, a
//! capped match count — so a busy process can never return an unbounded payload. The one
//! buffer *mutation*, `clear_output`, is a scoped action and lives in `scoped.rs`.

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

impl Facade {
    /// The most recent rendered output lines of a process (escape sequences applied),
    /// bounded to `lines` (defaulting to [`DEFAULT_OUTPUT_LINES`], capped at
    /// [`MAX_OUTPUT_LINES`]). `None` if no such process is registered; an empty list if it
    /// is registered but has never started.
    pub fn process_output(&self, id: ProcessId, lines: Option<usize>) -> Option<Vec<String>> {
        self.process_view(id)?;
        let n = lines.unwrap_or(DEFAULT_OUTPUT_LINES).min(MAX_OUTPUT_LINES);
        Some(self.supervisor().rendered_tail(id, n).unwrap_or_default())
    }

    /// A process's raw byte output (control sequences included), bounded by the raw
    /// scrollback's own byte cap. `None` if no such process is registered; an empty buffer
    /// if it is registered but has never started.
    pub fn process_raw_output(&self, id: ProcessId) -> Option<Vec<u8>> {
        self.process_view(id)?;
        Some(self.supervisor().pty_scrollback(id).unwrap_or_default())
    }

    /// Up to `limit` rendered output lines of a process containing `query` (a case-sensitive
    /// substring), defaulting to [`DEFAULT_SEARCH_MATCHES`] and capped at
    /// [`MAX_SEARCH_MATCHES`]. `None` if no such process is registered.
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
        Some(
            self.supervisor()
                .search_output(id, query, n)
                .unwrap_or_default(),
        )
    }

    /// Up to `limit` raw output lines of a process containing `query`, defaulting to
    /// [`DEFAULT_SEARCH_MATCHES`] and capped at [`MAX_SEARCH_MATCHES`]. `None` if no such
    /// process is registered.
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
