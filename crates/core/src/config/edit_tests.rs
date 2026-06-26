//! Tests for the comment-preserving `solo.yml` editor: each operation preserves the comments on
//! untouched entries and round-trips to exactly the intended config, and any layout the in-place
//! editor cannot handle falls back to a faithful render that still preserves the leading comments
//! and never injects Soloist's own header into a file the user wrote.

use super::*;
use crate::config::parse;

fn spec(command: &str) -> ProcessSpec {
    ProcessSpec {
        command: command.into(),
        working_dir: None,
        auto_start: true,
        auto_restart: false,
        restart_when_changed: Vec::new(),
        env: Default::default(),
    }
}

/// Every rewrite must round-trip: the new text parses back to exactly the intended config.
fn rewritten(original: &str, intended: &SoloYml) -> String {
    let current = parse(original).expect("original parses");
    let out = rewrite(original, &current, intended).expect("rewrite");
    assert_eq!(
        &parse(&out).expect("result parses"),
        intended,
        "the rewrite must round-trip to the intended config"
    );
    out
}

#[test]
fn add_appends_one_entry_and_preserves_comments() {
    let original = "# my project\nprocesses:\n  Web:\n    command: serve  # the frontend\n";
    let mut intended = parse(original).unwrap();
    intended.processes.insert("Api".into(), spec("cargo run"));

    let out = rewritten(original, &intended);

    assert!(out.contains("# my project"), "leading comment survives");
    assert!(
        out.contains("command: serve  # the frontend"),
        "the untouched entry is kept byte-for-byte, including its inline comment"
    );
    assert!(
        out.contains("  Api:\n    command: cargo run"),
        "exactly the new entry is appended"
    );
    assert_eq!(
        out.matches("command:").count(),
        2,
        "no entry was duplicated"
    );
}

#[test]
fn remove_drops_only_the_named_entry_and_keeps_other_comments() {
    let original =
        "processes:\n  Web:\n    command: serve  # keep me\n  Api:\n    command: cargo run\n";
    let mut intended = parse(original).unwrap();
    intended.processes.shift_remove("Api");

    let out = rewritten(original, &intended);

    assert!(
        out.contains("# keep me"),
        "the surviving entry keeps its comment"
    );
    assert!(!out.contains("cargo run"), "the removed entry is gone");
}

#[test]
fn a_pure_rename_swaps_the_key_and_keeps_the_body_comment() {
    let original = "processes:\n  Web:\n    command: serve  # frontend\n";
    let mut intended = parse(original).unwrap();
    intended.processes.shift_remove("Web");
    intended.processes.insert("Frontend".into(), spec("serve"));

    let out = rewritten(original, &intended);

    assert!(out.contains("Frontend:"), "the key is renamed");
    assert!(!out.contains("  Web:"), "the old key is gone");
    assert!(
        out.contains("# frontend"),
        "a pure rename preserves the entry's body comment"
    );
}

#[test]
fn an_update_replaces_the_field_and_keeps_other_entries_comments() {
    let original =
        "processes:\n  Web:\n    command: serve\n  Api:\n    command: cargo run  # backend\n";
    let mut intended = parse(original).unwrap();
    intended.processes.insert("Web".into(), spec("npm run dev"));

    let out = rewritten(original, &intended);

    assert!(out.contains("command: npm run dev"), "the field is updated");
    assert!(!out.contains("command: serve"), "the old value is gone");
    assert!(
        out.contains("# backend"),
        "the other entry's comment is preserved"
    );
}

#[test]
fn an_unparseable_layout_falls_back_to_a_correct_render() {
    // Four-space indentation is valid YAML but not the canonical layout the in-place editor edits,
    // so the rewrite falls back to a full render — which must still be correct.
    let original = "# header\nprocesses:\n    Web:\n        command: serve\n";
    let mut intended = parse(original).unwrap();
    intended.processes.insert("Api".into(), spec("cargo run"));

    let out = rewritten(original, &intended);

    assert!(
        out.contains("# header"),
        "the leading comment is preserved on fallback"
    );
    assert!(
        !out.contains("Soloist"),
        "fallback never injects Soloist's own header into a user's file"
    );
}

#[test]
fn adding_the_first_command_to_a_comment_only_file_writes_a_processes_block() {
    let original = "# just notes\n";
    let mut intended = parse(original).unwrap();
    intended.processes.insert("Web".into(), spec("serve"));

    let out = rewritten(original, &intended);

    assert!(out.contains("# just notes"), "the user's comment is kept");
    assert!(out.contains("processes:"), "the processes block is created");
}

#[test]
fn a_quoted_key_entry_is_edited_in_place() {
    // The demo's `solo.yml` uses a colon-bearing key that YAML must quote.
    let original = "processes:\n  'npm:dev':\n    command: npm run dev  # vite\n";
    let mut intended = parse(original).unwrap();
    intended.processes.insert("Queue".into(), spec("php queue"));

    let out = rewritten(original, &intended);

    assert!(
        out.contains("command: npm run dev  # vite"),
        "the quoted-key entry is kept verbatim with its comment"
    );
    assert!(out.contains("Queue:"), "the new entry is appended");
}
