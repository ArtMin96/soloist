use soloist_core::GitLineCounts;

use super::parse;

#[test]
fn numstat_counts_additions_and_deletions_without_a_third_modified_total() {
    let counts = parse(b"3\t2\tsrc/lib.rs\0");

    assert_eq!(
        counts,
        GitLineCounts {
            additions: 3,
            deletions: 2,
            complete: true,
        },
    );
}

#[test]
fn numstat_ignores_binary_entries_and_reads_rename_records_without_parsing_paths() {
    let counts = parse(
        b"-\t-\tassets/icon.bin\0\
          1\t4\t\0old name.rs\0new name.rs\0\
          2\t0\tname with a tab\tinside.rs\0",
    );

    assert_eq!(
        counts,
        GitLineCounts {
            additions: 3,
            deletions: 4,
            complete: true,
        },
    );
}
