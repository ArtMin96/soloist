//! Tests for reading back what a create printed. The tool answers with one thing — where what it
//! made can be found — and everything else it may have said comes before it.

use super::address;
use soloist_core::ForgeError;

const URL: &str = "https://github.example/owner/repo/pull/12";

#[test]
fn the_address_is_what_a_create_answers_with() {
    assert_eq!(address(URL.as_bytes()).expect("read"), URL);
}

#[test]
fn anything_the_tool_said_on_the_way_comes_before_the_address_and_is_not_it() {
    let chatty = format!("Creating pull request for feature into main\n\n{URL}\n");

    assert_eq!(address(chatty.as_bytes()).expect("read"), URL);
}

#[test]
fn a_create_that_printed_no_address_is_a_failure_rather_than_an_empty_answer() {
    assert!(matches!(
        address(b"   \n\n"),
        Err(ForgeError::Op { status: None }),
    ));
}
