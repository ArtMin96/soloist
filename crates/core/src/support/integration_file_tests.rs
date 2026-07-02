use super::*;

fn read(dir: &tempfile::TempDir, file: IntegrationFile) -> String {
    std::fs::read_to_string(dir.path().join(file.file_name())).expect("read the written file")
}

#[test]
fn a_missing_file_is_created_with_just_the_section() {
    let dir = tempfile::tempdir().expect("temp dir");

    let write = write_integration_guide(dir.path(), IntegrationFile::AgentsMd)
        .expect("write into an empty project");

    assert!(write.created);
    assert_eq!(write.path, dir.path().join("AGENTS.md"));
    let contents = read(&dir, IntegrationFile::AgentsMd);
    assert!(contents.starts_with(SECTION_BEGIN));
    assert!(contents.trim_end().ends_with(SECTION_END));
    assert!(contents.contains("bind_session_process"));
}

#[test]
fn an_unmarked_file_gets_the_section_appended_and_keeps_its_content() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("CLAUDE.md");
    std::fs::write(&path, "# My project\n\nHouse rules.\n").expect("seed the file");

    let write = write_integration_guide(dir.path(), IntegrationFile::ClaudeMd)
        .expect("append to an existing file");

    assert!(!write.created);
    let contents = read(&dir, IntegrationFile::ClaudeMd);
    assert!(contents.starts_with("# My project\n\nHouse rules.\n\n"));
    assert!(contents.contains(SECTION_BEGIN));
    assert!(contents.trim_end().ends_with(SECTION_END));
}

#[test]
fn rerunning_replaces_the_section_instead_of_duplicating_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("AGENTS.md"),
        "Before.\n\n<!-- soloist:integration-guide:begin -->\nstale guide\n<!-- soloist:integration-guide:end -->\n\nAfter.\n",
    )
    .expect("seed a marked file");

    write_integration_guide(dir.path(), IntegrationFile::AgentsMd).expect("first rewrite");
    let first = read(&dir, IntegrationFile::AgentsMd);
    write_integration_guide(dir.path(), IntegrationFile::AgentsMd).expect("second rewrite");
    let second = read(&dir, IntegrationFile::AgentsMd);

    assert_eq!(first, second, "re-running must be idempotent");
    assert_eq!(second.matches(SECTION_BEGIN).count(), 1);
    assert!(!second.contains("stale guide"));
    assert!(second.starts_with("Before.\n"));
    assert!(second.trim_end().ends_with("After."));
}

#[test]
fn a_stray_begin_marker_is_refused_and_the_file_left_untouched() {
    let dir = tempfile::tempdir().expect("temp dir");
    let seeded = format!("Notes quoting {SECTION_BEGIN} without its end.\n\nHouse rules.\n");
    std::fs::write(dir.path().join("AGENTS.md"), &seeded).expect("seed the file");

    let err = write_integration_guide(dir.path(), IntegrationFile::AgentsMd)
        .expect_err("an unpaired marker must refuse the write");

    assert!(matches!(
        err,
        IntegrationWriteError::UnmatchedMarkers { .. }
    ));
    assert_eq!(read(&dir, IntegrationFile::AgentsMd), seeded);
}

#[test]
fn an_end_marker_before_the_begin_is_refused_and_the_file_left_untouched() {
    let dir = tempfile::tempdir().expect("temp dir");
    let seeded = format!("{SECTION_END}\nOut of order.\n{SECTION_BEGIN}\n");
    std::fs::write(dir.path().join("CLAUDE.md"), &seeded).expect("seed the file");

    let err = write_integration_guide(dir.path(), IntegrationFile::ClaudeMd)
        .expect_err("an out-of-order pair must refuse the write");

    assert!(matches!(
        err,
        IntegrationWriteError::UnmatchedMarkers { .. }
    ));
    assert_eq!(read(&dir, IntegrationFile::ClaudeMd), seeded);
}

#[test]
fn a_duplicated_section_is_refused_and_the_file_left_untouched() {
    let dir = tempfile::tempdir().expect("temp dir");
    let seeded =
        format!("{SECTION_BEGIN}\none\n{SECTION_END}\n\n{SECTION_BEGIN}\ntwo\n{SECTION_END}\n");
    std::fs::write(dir.path().join("AGENTS.md"), &seeded).expect("seed the file");

    let err = write_integration_guide(dir.path(), IntegrationFile::AgentsMd)
        .expect_err("two managed sections must refuse the write");

    assert!(matches!(
        err,
        IntegrationWriteError::UnmatchedMarkers { .. }
    ));
    assert_eq!(read(&dir, IntegrationFile::AgentsMd), seeded);
}

#[test]
fn an_unwritable_root_surfaces_the_io_error() {
    let missing = std::path::Path::new("/nonexistent-soloist-test-root");

    let err = write_integration_guide(missing, IntegrationFile::AgentsMd)
        .expect_err("a missing directory cannot be written into");

    assert!(matches!(err, IntegrationWriteError::Io { .. }));
}
