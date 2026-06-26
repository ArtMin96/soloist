//! Acceptance tests for project command editing: a shared add appends exactly one `processes:`
//! entry and re-trusts; a local command runs in app state and leaves `solo.yml` byte-unchanged; and
//! the shared⇄local move transfers a command between the stores without duplicating or corrupting it.

use std::sync::Arc;

use super::*;
use crate::config::{config_path, ConfigWriteError, ProcessSpec};
use crate::ids::ProjectId;
use crate::ports::{CorePorts, TokioClock};
use crate::testing::{FakeProjectRepo, FakeSettingsRepo, FakeSpawner, FakeTrustRepo};

/// A façade over fakes, plus a temp project directory seeded with `solo.yml`, opened in the config
/// engine so its write path has sync state. Returns the façade, the project id, and the temp dir.
fn project_with_yaml(initial: &str) -> (Facade, ProjectId, tempfile::TempDir) {
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .project_settings_repo(Arc::new(FakeSettingsRepo::new()))
        .build(),
    );
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(config_path(dir.path()), initial).expect("seed solo.yml");
    let project = ProjectId::from_raw(1);
    facade
        .config()
        .open(project, dir.path().to_path_buf())
        .expect("open seeds config state");
    (facade, project, dir)
}

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

#[test]
fn a_shared_add_appends_one_entry_and_requires_trust() {
    let (facade, project, dir) =
        project_with_yaml("processes:\n  Web:\n    command: npm run dev\n");

    let pending = facade
        .add_shared_command(project, "Queue", spec("php queue"))
        .expect("shared add");

    // The new command needs trust before it can start.
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].name, "Queue");

    // The file gained exactly one entry; the existing one is preserved.
    let text = std::fs::read_to_string(config_path(dir.path())).unwrap();
    assert!(text.contains("command: npm run dev"));
    assert!(text.contains("Queue:\n    command: php queue"));
    let parsed = crate::config::parse(&text).unwrap();
    assert_eq!(parsed.processes.len(), 2, "exactly one entry was appended");
}

#[test]
fn a_local_command_leaves_solo_yml_byte_unchanged() {
    let original = "# my stack\nprocesses:\n  Web:\n    command: npm run dev  # vite\n";
    let (facade, project, dir) = project_with_yaml(original);
    let before = std::fs::read_to_string(config_path(dir.path())).unwrap();

    let settings = facade
        .add_local_command(project, "Logs", spec("tail -f log"))
        .expect("local add");

    // The local command is stored in app state…
    assert!(settings.local_commands.contains_key("Logs"));
    assert_eq!(
        facade
            .project_settings(project)
            .unwrap()
            .local_commands
            .len(),
        1
    );
    // …and the shared file is byte-for-byte untouched.
    assert_eq!(
        std::fs::read_to_string(config_path(dir.path())).unwrap(),
        before,
        "a local command is never written to solo.yml"
    );
}

#[test]
fn making_a_command_local_moves_it_without_duplication() {
    let (facade, project, dir) = project_with_yaml(
        "processes:\n  Web:\n    command: npm run dev\n  Api:\n    command: cargo run\n",
    );

    let settings = facade
        .make_command_local(project, "Api")
        .expect("make local");

    // It is now local…
    assert!(settings.local_commands.contains_key("Api"));
    assert_eq!(settings.local_commands["Api"].command, "cargo run");
    // …and gone from solo.yml (no duplicate across stores).
    let parsed =
        crate::config::parse(&std::fs::read_to_string(config_path(dir.path())).unwrap()).unwrap();
    assert!(parsed.processes.contains_key("Web"));
    assert!(
        !parsed.processes.contains_key("Api"),
        "the moved command left solo.yml"
    );
}

#[test]
fn saving_a_local_command_to_yaml_moves_it_back_and_requires_trust() {
    let (facade, project, dir) =
        project_with_yaml("processes:\n  Web:\n    command: npm run dev\n");
    facade
        .add_local_command(project, "Api", spec("cargo run"))
        .expect("local add");

    let pending = facade
        .save_command_to_yaml(project, "Api")
        .expect("save to yaml");

    // The shared write re-trusts the now-shared command.
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].name, "Api");
    // It is in solo.yml…
    let parsed =
        crate::config::parse(&std::fs::read_to_string(config_path(dir.path())).unwrap()).unwrap();
    assert!(parsed.processes.contains_key("Api"));
    // …and no longer local (moved, not copied).
    assert!(
        !facade
            .project_settings(project)
            .unwrap()
            .local_commands
            .contains_key("Api"),
        "the command moved out of the local overlay"
    );
}

#[test]
fn a_round_trip_through_local_and_back_preserves_the_command() {
    let original = "processes:\n  Api:\n    command: cargo run\n    auto_restart: true\n";
    let (facade, project, dir) = project_with_yaml(original);

    facade
        .make_command_local(project, "Api")
        .expect("make local");
    facade
        .save_command_to_yaml(project, "Api")
        .expect("save back");

    // Back in solo.yml with its fields intact, exactly once, and no longer local.
    let parsed =
        crate::config::parse(&std::fs::read_to_string(config_path(dir.path())).unwrap()).unwrap();
    assert_eq!(parsed.processes.len(), 1);
    assert!(
        parsed.processes["Api"].auto_restart,
        "the spec survived the round trip"
    );
    assert!(facade
        .project_settings(project)
        .unwrap()
        .local_commands
        .is_empty());
}

#[test]
fn a_duplicate_shared_add_is_refused_and_does_not_touch_the_file() {
    let (facade, project, dir) =
        project_with_yaml("processes:\n  Web:\n    command: npm run dev\n");
    let before = std::fs::read_to_string(config_path(dir.path())).unwrap();

    let err = facade
        .add_shared_command(project, "Web", spec("other"))
        .unwrap_err();

    assert!(matches!(err, ConfigWriteError::DuplicateCommand(name) if name == "Web"));
    assert_eq!(
        std::fs::read_to_string(config_path(dir.path())).unwrap(),
        before,
        "a refused add leaves solo.yml untouched"
    );
}

#[test]
fn an_svg_icon_is_rejected_and_leaves_the_file_untouched() {
    let (facade, project, dir) =
        project_with_yaml("processes:\n  Web:\n    command: npm run dev\n");
    let before = std::fs::read_to_string(config_path(dir.path())).unwrap();

    let err = facade
        .set_project_icon(project, Some("public/favicon.svg".into()))
        .unwrap_err();

    assert!(matches!(err, ConfigWriteError::UnsupportedIcon(p) if p.ends_with(".svg")));
    assert_eq!(
        std::fs::read_to_string(config_path(dir.path())).unwrap(),
        before,
        "a rejected icon leaves solo.yml untouched"
    );
}

#[test]
fn a_supported_icon_is_written_to_solo_yml() {
    let (facade, project, dir) =
        project_with_yaml("processes:\n  Web:\n    command: npm run dev\n");

    facade
        .set_project_icon(project, Some("assets/icon.png".into()))
        .expect("set icon");

    let parsed =
        crate::config::parse(&std::fs::read_to_string(config_path(dir.path())).unwrap()).unwrap();
    assert_eq!(
        parsed.icon.as_deref(),
        Some(std::path::Path::new("assets/icon.png"))
    );
}
