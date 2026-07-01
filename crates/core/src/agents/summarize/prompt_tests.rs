use super::{build_prompt, INSTRUCTION, MAX_SNAPSHOT_BYTES};

#[test]
fn prompt_carries_the_instruction_and_the_snapshot() {
    let prompt = build_prompt(&[
        "opened src/main.rs".to_string(),
        "running cargo test".to_string(),
    ]);
    assert!(prompt.starts_with(INSTRUCTION));
    assert!(prompt.contains("opened src/main.rs\nrunning cargo test"));
}

#[test]
fn an_empty_snapshot_still_yields_the_instruction() {
    let prompt = build_prompt(&[]);
    assert_eq!(prompt, format!("{INSTRUCTION}\n\n"));
}

#[test]
fn an_oversized_snapshot_is_bounded_to_its_most_recent_bytes() {
    // A single line larger than the cap, ending in a marker: the bound must keep the tail.
    let line = format!("{}TAIL", "x".repeat(MAX_SNAPSHOT_BYTES + 500));
    let prompt = build_prompt(&[line]);

    let body = prompt
        .strip_prefix(&format!("{INSTRUCTION}\n\n"))
        .expect("prompt begins with the instruction");
    assert_eq!(
        body.len(),
        MAX_SNAPSHOT_BYTES,
        "snapshot trimmed to the cap"
    );
    assert!(body.ends_with("TAIL"), "the most recent bytes are kept");
}
