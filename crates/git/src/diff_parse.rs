//! Turning `git diff --numstat --patch` output into the core's diff vocabulary.
//!
//! Two machine forms arrive in one invocation. The counted form leads, and is read for one
//! thing only: a path version control treats as bytes rather than text prints `-` for both of
//! its counts, which is a fact rather than a sentence — so nothing here depends on the wording,
//! or the language, of the line the patch form prints for the same file.
//!
//! The patch that follows is split where its segments begin. The split exists so a very long
//! diff can be handed over as far as its first hunks and no further: joined back in order it is
//! byte-for-byte what version control produced, and cut anywhere in that order it is still a
//! patch rather than the middle of one.

use soloist_core::RawFileDiff;

/// What version control prints for each count of a path it treats as binary.
const BINARY_COUNT: &[u8] = b"-";

/// What begins one hunk of a unified diff. A line of a file's own content is always prefixed
/// with a space, a `+`, or a `-`, so this at the start of a line is never file content.
const HUNK: &[u8] = b"@@ ";

/// What begins one path's block of a unified diff.
const FILE: &[u8] = b"diff --git ";

/// Reads one invocation's output: whether the path is binary, and its patch split into the
/// header every hunk has to follow, and the hunks themselves.
pub(crate) fn parse(output: &[u8]) -> RawFileDiff {
    let mut binary = false;
    let mut header: Vec<u8> = Vec::new();
    let mut hunks: Vec<Vec<u8>> = Vec::new();
    let mut in_patch = false;

    for line in output.split_inclusive(|&byte| byte == b'\n') {
        if !in_patch {
            if !line.starts_with(FILE) {
                binary |= is_binary_record(line);
                continue;
            }
            in_patch = true;
        }
        // A second path's block, where one appears, starts a segment of its own rather than
        // being swallowed into the hunk above it.
        let starts_segment =
            line.starts_with(HUNK) || (line.starts_with(FILE) && !hunks.is_empty());
        match hunks.last_mut() {
            Some(hunk) if !starts_segment => hunk.extend_from_slice(line),
            _ if starts_segment => hunks.push(line.to_vec()),
            _ => header.extend_from_slice(line),
        }
    }

    RawFileDiff {
        binary,
        header: text(&header),
        hunks: hunks.iter().map(|hunk| text(hunk)).collect(),
    }
}

/// Whether a counted record reports a path as binary: both of its counts are a dash.
fn is_binary_record(record: &[u8]) -> bool {
    let mut counts = record.split(|&byte| byte == b'\t');
    counts.next() == Some(BINARY_COUNT) && counts.next() == Some(BINARY_COUNT)
}

/// Patch bytes as text. A source file that is not valid UTF-8 is carried lossily, since the
/// read model it feeds crosses a boundary that could not represent it faithfully either.
fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(test)]
#[path = "diff_parse_tests.rs"]
mod tests;
