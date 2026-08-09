//! Where a path leads, against a real filesystem — the guard the pure core cannot make, so it is
//! asserted against real files and real links rather than against a description of them.

use std::os::unix::fs::symlink;

use super::contained;

/// A project holding `readme.md`, and a secret kept well outside it.
fn a_project_and_a_secret() -> (tempfile::TempDir, tempfile::TempDir) {
    let project = tempfile::tempdir().expect("temp dir");
    std::fs::write(project.path().join("readme.md"), "the file").expect("write");
    let elsewhere = tempfile::tempdir().expect("temp dir");
    std::fs::write(elsewhere.path().join("id_rsa"), "a private key").expect("write");
    (project, elsewhere)
}

#[test]
fn a_file_the_project_holds_is_where_it_says_it_is() {
    let (project, _) = a_project_and_a_secret();

    let led = contained(project.path(), "readme.md").expect("a path inside the project");

    assert!(led.ends_with("readme.md"), "{led:?}");
    assert!(led.starts_with(project.path().canonicalize().expect("canonical root")));
}

#[test]
fn a_link_inside_the_project_that_leads_out_of_it_opens_nothing() {
    // The case the core's own guard cannot catch: every component of "key" is an ordinary name,
    // so nothing about the path says it leaves the repository. Following it would hand a program
    // a file the project never held.
    let (project, elsewhere) = a_project_and_a_secret();
    symlink(elsewhere.path().join("id_rsa"), project.path().join("key")).expect("link");

    assert_eq!(contained(project.path(), "key"), None);
}

#[test]
fn a_link_inside_the_project_that_stays_inside_it_is_still_opened() {
    // Refusing every link would refuse an ordinary way of organising a repository, so what is
    // refused is leaving — not being a link.
    let (project, _) = a_project_and_a_secret();
    symlink(
        project.path().join("readme.md"),
        project.path().join("docs.md"),
    )
    .expect("link");

    let led = contained(project.path(), "docs.md").expect("a link that stays inside");

    assert!(led.ends_with("readme.md"), "{led:?}");
}

#[test]
fn a_path_that_climbs_out_of_the_project_opens_nothing() {
    let (project, elsewhere) = a_project_and_a_secret();
    let climbing = format!(
        "../{}/id_rsa",
        elsewhere
            .path()
            .file_name()
            .expect("a folder name")
            .to_string_lossy()
    );

    assert_eq!(contained(project.path(), &climbing), None);
}

#[test]
fn a_file_that_is_no_longer_there_opens_nothing() {
    // A listing is a moment old by the time somebody clicks it. There is nothing to open, and no
    // way to know where the name would have led.
    let (project, _) = a_project_and_a_secret();

    assert_eq!(contained(project.path(), "gone.md"), None);
}
