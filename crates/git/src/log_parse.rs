//! Turning `git log -z --format=…` output into the core's commit vocabulary.
//!
//! Every field is NUL-separated and `-z` makes the record separator a NUL too, so the whole answer
//! is one flat stream of fields read five at a time. That matters because the two fields a person
//! wrote — the author's name and the subject — may contain anything at all except a NUL, so no
//! other separator is safe.
//!
//! Nothing here reads prose. The parent list is counted, never inspected, which is the whole of how
//! a merge is recognised: a commit joining more than one line of history has more than one parent,
//! whatever its message happens to say.

use soloist_core::CommitEntry;

/// The fields each record carries, in the order the format asks for them.
const FIELDS: usize = 5;

/// The format one record is printed in: object name, author name, author date, parent list, subject.
/// Every field NUL-separated; with `-z` the records are too.
pub(crate) const LOG_FORMAT: &str = "--format=%H%x00%an%x00%at%x00%P%x00%s";

/// Reads one invocation's output into the commits it reported, newest first as version control
/// listed them.
///
/// A record that is short, or whose date is not a number, is dropped rather than guessed at: the
/// answer is a list, so one unreadable entry costs that entry and nothing else.
pub(crate) fn parse(output: &[u8]) -> Vec<CommitEntry> {
    let text = String::from_utf8_lossy(output);
    let fields: Vec<&str> = text.split('\0').collect();
    fields.chunks_exact(FIELDS).filter_map(entry).collect()
}

/// One commit from its five fields, or `None` when the date it states is not one.
fn entry(record: &[&str]) -> Option<CommitEntry> {
    Some(CommitEntry {
        id: record[0].to_string(),
        author: record[1].to_string(),
        authored_at: record[2].parse().ok()?,
        merge: record[3].split_whitespace().count() > 1,
        subject: record[4].to_string(),
    })
}

#[cfg(test)]
#[path = "log_parse_tests.rs"]
mod tests;
