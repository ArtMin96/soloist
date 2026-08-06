//! Turning `git diff --numstat --patch` output into the core's diff vocabulary.
//!
//! Two machine forms arrive in one invocation. The counted form leads, and is read for one
//! thing only: a path version control treats as bytes rather than text prints `-` for both of
//! its counts, which is a fact rather than a sentence — so nothing here depends on the wording,
//! or the language, of the line the patch form prints for the same file.
//!
//! The patch that follows is split where its hunks begin, each carrying the range its `@@` line
//! states. The split exists for two readers. A very long diff can be handed over as far as its
//! first hunks and no further: joined back in order it is byte-for-byte what version control
//! produced, and cut anywhere in that order it is still a patch rather than the middle of one.
//! And the range is how an action names one hunk, which is why every hunk here has one — text
//! before the first `@@`, and a second path's block where one appears, is carried with the
//! hunk it precedes rather than left as a rangeless segment nothing could refer to.

use soloist_core::{HunkRange, RawFileDiff, RawHunk};

/// What version control prints for each count of a path it treats as binary.
const BINARY_COUNT: &[u8] = b"-";

/// What begins one hunk of a unified diff. A line of a file's own content is always prefixed
/// with a space, a `+`, or a `-`, so this at the start of a line is never file content.
const HUNK: &[u8] = b"@@ ";

/// What begins one path's block of a unified diff.
const FILE: &[u8] = b"diff --git ";

/// What separates the two sides' ranges on a hunk's `@@` line, and marks each of them.
const OLD_SIDE: char = '-';
const NEW_SIDE: char = '+';

/// The count a side's range means when it states only a start.
const IMPLIED_COUNT: u32 = 1;

/// Reads one invocation's output: whether the path is binary, and its patch split into the
/// header every hunk has to follow, and the hunks themselves.
pub(crate) fn parse(output: &[u8]) -> RawFileDiff {
    let mut binary = false;
    let mut header: Vec<u8> = Vec::new();
    let mut hunks: Vec<(Option<HunkRange>, Vec<u8>)> = Vec::new();
    let mut in_patch = false;

    for line in output.split_inclusive(|&byte| byte == b'\n') {
        if !in_patch {
            if !line.starts_with(FILE) {
                binary |= is_binary_record(line);
                continue;
            }
            in_patch = true;
        }
        // A second path's block, where one appears, opens a segment of its own rather than
        // being swallowed into the hunk above it — so a cut between segments never leaves one
        // path's header inside another's hunk.
        let range = line.starts_with(HUNK).then(|| hunk_range(line)).flatten();
        let opens_segment = range.is_some() || (line.starts_with(FILE) && !hunks.is_empty());
        match hunks.last_mut() {
            Some((_, hunk)) if !opens_segment => hunk.extend_from_slice(line),
            _ if opens_segment => hunks.push((range, line.to_vec())),
            _ => header.extend_from_slice(line),
        }
    }

    RawFileDiff {
        binary,
        header: text(&header),
        hunks: joined(hunks),
    }
}

/// Folds each rangeless segment into the hunk that follows it, so every hunk handed over states
/// where it falls and carries the header a patch built from it has to name. Joined back in order
/// the result is unchanged: only where the boundaries sit moves.
fn joined(segments: Vec<(Option<HunkRange>, Vec<u8>)>) -> Vec<RawHunk> {
    let mut hunks: Vec<RawHunk> = Vec::new();
    let mut pending: Vec<u8> = Vec::new();
    for (range, bytes) in segments {
        pending.extend_from_slice(&bytes);
        if let Some(range) = range {
            hunks.push(RawHunk {
                range,
                text: text(&pending),
            });
            pending.clear();
        }
    }
    // A trailing block with no hunk of its own — a second path that only moved, say. It belongs
    // to the patch, so it joins the last hunk rather than being dropped. There is always one to
    // join: a segment only ever opens on a hunk, or after one.
    if let Some(hunk) = hunks.last_mut() {
        hunk.text.push_str(&text(&pending));
    }
    hunks
}

/// Where a hunk falls on each side, from its `@@ -old[,count] +new[,count] @@` line. A side
/// that states only a start covers exactly one line.
fn hunk_range(line: &[u8]) -> Option<HunkRange> {
    let line = std::str::from_utf8(line).ok()?;
    let mut sides = line.strip_prefix("@@ ")?.split(' ');
    let (old_start, old_lines) = side(sides.next()?, OLD_SIDE)?;
    let (new_start, new_lines) = side(sides.next()?, NEW_SIDE)?;
    Some(HunkRange {
        old_start,
        old_lines,
        new_start,
        new_lines,
    })
}

/// One side of a hunk's range: `-12,3`, or `+12` for a side covering a single line.
fn side(field: &str, marker: char) -> Option<(u32, u32)> {
    let mut parts = field.strip_prefix(marker)?.split(',');
    let start = parts.next()?.parse().ok()?;
    let lines = match parts.next() {
        Some(lines) => lines.parse().ok()?,
        None => IMPLIED_COUNT,
    };
    Some((start, lines))
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
