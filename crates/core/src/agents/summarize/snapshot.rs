//! The snapshot source: a process's most recent rendered output, read when it goes idle.

use crate::ids::ProcessId;

/// Reads a process's most recent rendered output lines — the compact snapshot summarized when an
/// agent goes idle. A narrow read seam over the C2 terminal buffers: the summary reactor (C4)
/// depends only on "recent output for a process", not on the whole supervisor, so it composes and
/// tests without one. Implemented over the supervisor at the composition root; a fake in tests.
pub trait OutputSnapshot: Send + Sync {
    /// The most recent `max_lines` rendered lines for `id`, oldest first — empty when the process
    /// is gone or has produced nothing.
    fn recent_lines(&self, id: ProcessId, max_lines: usize) -> Vec<String>;
}
