use std::fs;

use super::project_root_for;

#[test]
fn a_directory_opens_itself() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(project_root_for(dir.path()), Some(dir.path().to_path_buf()));
}

#[test]
fn a_solo_yml_file_opens_its_containing_directory() {
    let dir = tempfile::tempdir().unwrap();
    let yml = dir.path().join("solo.yml");
    fs::write(&yml, "processes: {}\n").unwrap();
    assert_eq!(project_root_for(&yml), Some(dir.path().to_path_buf()));
}

#[test]
fn a_path_that_does_not_exist_opens_nothing() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(project_root_for(&dir.path().join("absent.yml")), None);
}
