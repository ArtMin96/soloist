//! What a path from outside the core is allowed to name.

use super::inside_repository;

#[test]
fn an_ordinary_relative_path_names_something_inside_the_repository() {
    assert!(inside_repository("src/main.rs"));
}

#[test]
fn a_path_that_climbs_out_of_the_repository_names_nothing_inside_it() {
    assert!(!inside_repository("../secrets.env"));
    assert!(!inside_repository("src/../../etc/passwd"));
}

#[test]
fn an_absolute_path_names_nothing_inside_the_repository() {
    assert!(!inside_repository("/etc/passwd"));
}

#[test]
fn the_repository_root_itself_is_not_something_to_read() {
    assert!(!inside_repository(""));
    assert!(!inside_repository("."));
}
