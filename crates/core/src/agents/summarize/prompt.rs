//! Building the summarizer prompt from a compact rendered-text snapshot.
//!
//! Solo sends a *compact* rendered-text snapshot (not the full transcript) to the summarizer; this
//! mirrors that — the fixed instruction plus a bounded tail of the agent's rendered output.

/// The fixed instruction sent with every snapshot. Terse and output-only, so a small, fast model
/// returns a usable single line rather than a paragraph.
const INSTRUCTION: &str = "In one short line, summarize what the coding agent in this terminal is \
currently doing. Reply with only the summary — no preamble, no quotes.";

/// How many of the most recent rendered lines the snapshot includes — enough to summarize the
/// current activity without sending the whole scrollback (a "compact" snapshot).
pub(super) const SNAPSHOT_LINES: usize = 40;

/// A ceiling on the snapshot's size in bytes, applied after taking the last [`SNAPSHOT_LINES`]
/// lines, so a burst of very long lines can't send an unbounded prompt.
const MAX_SNAPSHOT_BYTES: usize = 4000;

/// Composes the summarizer prompt: the fixed instruction followed by the snapshot, bounded to
/// [`MAX_SNAPSHOT_BYTES`] by keeping the most recent (tail) bytes on a char boundary.
pub(super) fn build_prompt(snapshot: &[String]) -> String {
    let mut body = snapshot.join("\n");
    if body.len() > MAX_SNAPSHOT_BYTES {
        let mut start = body.len() - MAX_SNAPSHOT_BYTES;
        while !body.is_char_boundary(start) {
            start += 1;
        }
        body.drain(..start);
    }
    format!("{INSTRUCTION}\n\n{body}")
}

#[cfg(test)]
#[path = "prompt_tests.rs"]
mod tests;
