//! What the diff seam reads out of one invocation's output. The fixtures are real `git diff`
//! output, trimmed to the shape under test.

use super::parse;

const PATCH: &str = "\
diff --git a/src/main.rs b/src/main.rs
index 83db48f..bf269f4 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,3 @@
 fn main() {
-    println!(\"old\");
+    println!(\"new\");
@@ -10,2 +10,3 @@
 }
+
";

#[test]
fn a_patch_is_split_into_the_header_and_its_hunks_and_joins_back_unchanged() {
    let output = format!("1\t1\tsrc/main.rs\n\n{PATCH}");

    let diff = parse(output.as_bytes());

    assert!(!diff.binary);
    assert_eq!(diff.hunks.len(), 2);
    assert!(diff.header.starts_with("diff --git "));
    assert_eq!(
        format!("{}{}", diff.header, diff.hunks.concat()),
        PATCH,
        "joined back in order it is what version control produced, byte for byte",
    );
}

#[test]
fn each_hunk_carries_its_own_lines_rather_than_the_ones_after_it() {
    let diff = parse(format!("1\t1\tsrc/main.rs\n\n{PATCH}").as_bytes());

    assert!(diff.hunks[0].starts_with("@@ -1,3 +1,3 @@\n"));
    assert!(
        diff.hunks[0].contains("+    println!(\"new\");\n"),
        "a hunk holds the lines under its own header",
    );
    assert!(
        !diff.hunks[0].contains("@@ -10,2"),
        "and stops where the next one starts",
    );
}

#[test]
fn a_path_reported_with_no_counts_at_all_is_binary() {
    let output = "-\t-\tassets/icon.png\n\ndiff --git a/assets/icon.png b/assets/icon.png\n\
                  index c98bb4c..7103e29 100644\nBinary files a/assets/icon.png and b/assets/icon.png differ\n";

    assert!(
        parse(output.as_bytes()).binary,
        "the counted form states it as data, so nothing depends on the sentence beneath it",
    );
}

#[test]
fn a_counted_path_with_real_numbers_is_not_binary() {
    let output = "0\t0\tsrc/main.rs\n\ndiff --git a/src/main.rs b/src/main.rs\n";

    assert!(!parse(output.as_bytes()).binary);
}

#[test]
fn a_path_that_does_not_differ_produces_no_patch_at_all() {
    let diff = parse(b"");

    assert_eq!(diff.header, "");
    assert!(diff.hunks.is_empty());
    assert!(!diff.binary);
}

#[test]
fn a_second_paths_block_stays_whole_rather_than_joining_the_hunk_above_it() {
    let output = "1\t1\ta.txt\n1\t1\tb.txt\n\n\
                  diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-a\n+A\n\
                  diff --git a/b.txt b/b.txt\n--- a/b.txt\n+++ b/b.txt\n@@ -1 +1 @@\n-b\n+B\n";

    let diff = parse(output.as_bytes());

    assert_eq!(
        diff.hunks.len(),
        3,
        "two hunks and the second path's header"
    );
    assert!(diff.hunks[1].starts_with("diff --git a/b.txt"));
    assert!(
        !diff.hunks[0].contains("diff --git a/b.txt"),
        "a cut between segments never leaves one path's header inside another's hunk",
    );
}
