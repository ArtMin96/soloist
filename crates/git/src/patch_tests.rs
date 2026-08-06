//! What the patch builder hands to `git apply`. The fixtures are real `git diff` output, and
//! what is asserted is that the bytes come back through it unchanged — the whole reason a hunk
//! is carried rather than rebuilt.

use soloist_core::{HunkRange, RawFileDiff, RawHunk};

use super::one_hunk;

const HEADER: &str = "diff --git a/notes.md b/notes.md\nindex 83db48f..bf269f4 100644\n\
                      --- a/notes.md\n+++ b/notes.md\n";

const FIRST: &str = "@@ -1,3 +1,3 @@\n one\n-two\n+TWO\n three\n";
const LAST: &str = "@@ -20,3 +20,4 @@\n twenty\n twenty-one\n+twenty-and-a-half\n";

fn range(old_start: u32, old_lines: u32, new_start: u32, new_lines: u32) -> HunkRange {
    HunkRange {
        old_start,
        old_lines,
        new_start,
        new_lines,
    }
}

fn diff() -> RawFileDiff {
    RawFileDiff {
        binary: false,
        header: HEADER.to_string(),
        hunks: vec![
            RawHunk {
                range: range(1, 3, 1, 3),
                text: FIRST.to_string(),
            },
            RawHunk {
                range: range(20, 3, 20, 4),
                text: LAST.to_string(),
            },
        ],
    }
}

#[test]
fn one_hunk_is_carried_out_of_a_diff_with_the_header_it_has_to_follow() {
    let patch = one_hunk(&diff(), range(20, 3, 20, 4)).expect("the last hunk");

    assert_eq!(
        patch,
        format!("{HEADER}{LAST}"),
        "the hunk is version control's own bytes, unedited, under the header naming its file",
    );
}

#[test]
fn the_hunks_around_the_named_one_are_left_out() {
    let patch = one_hunk(&diff(), range(1, 3, 1, 3)).expect("the first hunk");

    assert!(!patch.contains("twenty"), "{patch}");
}

#[test]
fn a_range_the_diff_no_longer_holds_builds_no_patch_at_all() {
    assert_eq!(
        one_hunk(&diff(), range(9, 3, 9, 3)),
        None,
        "a request built against a diff the file has moved past must change nothing, rather \
         than change whatever now sits at those lines",
    );
}

#[test]
fn a_hunk_carrying_its_own_paths_header_is_not_given_a_second_one() {
    let second_path = "diff --git a/other.md b/other.md\n--- a/other.md\n+++ b/other.md\n\
                       @@ -1 +1 @@\n-a\n+A\n";
    let diff = RawFileDiff {
        binary: false,
        header: HEADER.to_string(),
        hunks: vec![RawHunk {
            range: range(1, 1, 1, 1),
            text: second_path.to_string(),
        }],
    };

    let patch = one_hunk(&diff, range(1, 1, 1, 1)).expect("the hunk");

    assert_eq!(patch, second_path, "a patch never names two files");
}
