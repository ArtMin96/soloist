//! Acceptance tests for project command editing: a shared add appends exactly one `processes:`
//! entry and re-trusts; a local command runs in app state and leaves `solo.yml` byte-unchanged; and
//! the shared⇄local move transfers a command between the stores without duplicating or corrupting it.

use std::path::PathBuf;
use std::sync::Arc;

use super::*;
use crate::config::{config_path, ConfigWriteError, ProcessSpec};
use crate::ids::ProjectId;
use crate::ports::{CorePorts, TokioClock};
use crate::projects::ProjectCommandView;
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

/// The spec the settings-page editor persists, rebuilt from a command's read-model view — the Rust
/// mirror of the UI's `specOf`/`buildSpec`. Every `ProcessSpec` field comes from the flattened view,
/// so a field the view drops is a field the edit silently wipes.
fn spec_from_view(view: &ProjectCommandView) -> ProcessSpec {
    ProcessSpec {
        command: view.command.clone(),
        working_dir: view.working_dir.clone().map(PathBuf::from),
        auto_start: view.auto_start,
        auto_restart: view.auto_restart,
        restart_when_changed: view.restart_when_changed.clone(),
        env: view.env.clone(),
    }
}

/// One command's view off the assembled settings page, by name.
fn command_view(facade: &Facade, project: ProjectId, name: &str) -> ProjectCommandView {
    facade
        .project_settings_page(project)
        .expect("settings page")
        .commands
        .into_iter()
        .find(|command| command.name == name)
        .expect("command on the settings page")
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

#[test]
fn an_extensionless_icon_is_rejected() {
    let (facade, project, _dir) =
        project_with_yaml("processes:\n  Web:\n    command: npm run dev\n");

    let err = facade
        .set_project_icon(project, Some("assets/icon".into()))
        .unwrap_err();

    assert!(matches!(err, ConfigWriteError::UnsupportedIcon(p) if p == "assets/icon"));
}

#[test]
fn setting_an_icon_refreshes_the_project_record_and_announces_it() {
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
    std::fs::write(
        config_path(dir.path()),
        "processes:\n  Web:\n    command: npm run dev\n",
    )
    .expect("seed solo.yml");
    // Register the project so it has a durable record (the displayed icon's source), then open it.
    let record = facade
        .projects()
        .add(dir.path(), None, None)
        .expect("register project");
    facade
        .config()
        .open(record.id, record.root.clone())
        .expect("open config");
    let mut events = facade.subscribe();

    facade
        .set_project_icon(record.id, Some("assets/icon.png".into()))
        .expect("set icon");

    // The durable record now carries the new icon, so project_list (the sidebar's source) shows it.
    let refreshed = facade
        .projects()
        .get(record.id)
        .expect("get")
        .expect("record");
    assert_eq!(
        refreshed.icon.as_deref(),
        Some(std::path::Path::new("assets/icon.png"))
    );
    // And the change was announced, so the project read model re-reads without a reopen.
    assert!(matches!(
        events.try_recv(),
        Ok(DomainEvent::ProjectOpened { id }) if id == record.id
    ));
}

#[test]
fn saving_to_yaml_rolls_back_the_shared_add_when_clearing_the_local_copy_fails() {
    let settings: Arc<FakeSettingsRepo<ProjectId, ProjectSettings>> =
        Arc::new(FakeSettingsRepo::new());
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .project_settings_repo(settings.clone())
        .build(),
    );
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        config_path(dir.path()),
        "processes:\n  Web:\n    command: npm run dev\n",
    )
    .expect("seed solo.yml");
    let project = ProjectId::from_raw(1);
    facade
        .config()
        .open(project, dir.path().to_path_buf())
        .expect("open config");
    facade
        .add_local_command(project, "Job", spec("php queue"))
        .expect("add local");

    // Make the local-remove step fail after the shared add has been written.
    settings.fail_saves();
    let err = facade.save_command_to_yaml(project, "Job").unwrap_err();

    assert!(matches!(err, MoveCommandError::Store(_)));
    // The shared add is rolled back, so the command is not left in solo.yml — never in both stores.
    let text = std::fs::read_to_string(config_path(dir.path())).unwrap();
    assert!(
        !text.contains("Job"),
        "the shared add is rolled back, leaving solo.yml without the command"
    );
}

#[test]
fn making_a_command_local_rolls_back_when_the_shared_remove_fails() {
    let (facade, project, dir) = project_with_yaml(
        "processes:\n  Web:\n    command: npm run dev\n  Api:\n    command: cargo run\n",
    );
    // Force the shared remove to fail: delete solo.yml so the config write reloads an empty config
    // and the remove finds no such command. The engine's in-memory snapshot still lists Api, so
    // make_command_local proceeds past the spec lookup and then hits the failing shared remove.
    std::fs::remove_file(config_path(dir.path())).expect("remove solo.yml");

    let err = facade.make_command_local(project, "Api").unwrap_err();

    assert!(matches!(err, MoveCommandError::Config(_)));
    // The local add is rolled back, so the command is never left in both stores when the shared
    // remove fails (mirrors save_command_to_yaml's rollback in the other direction).
    assert!(
        !facade
            .project_settings(project)
            .unwrap()
            .local_commands
            .contains_key("Api"),
        "the local add is rolled back when the shared remove fails"
    );
}

#[test]
fn adding_a_local_command_whose_name_is_already_shared_is_refused() {
    let (facade, project, dir) =
        project_with_yaml("processes:\n  Web:\n    command: npm run dev\n");
    let before = std::fs::read_to_string(config_path(dir.path())).unwrap();

    let err = facade
        .add_local_command(project, "Web", spec("other"))
        .unwrap_err();

    assert!(matches!(err, LocalCommandError::Duplicate(name) if name == "Web"));
    assert!(
        facade
            .project_settings(project)
            .unwrap()
            .local_commands
            .is_empty(),
        "a name already used by a shared command cannot become a local command"
    );
    assert_eq!(
        std::fs::read_to_string(config_path(dir.path())).unwrap(),
        before,
        "a refused local add leaves solo.yml untouched"
    );
}

#[test]
fn adding_a_shared_command_whose_name_is_already_local_is_refused() {
    let (facade, project, dir) =
        project_with_yaml("processes:\n  Web:\n    command: npm run dev\n");
    facade
        .add_local_command(project, "Job", spec("php queue"))
        .expect("add local");
    let before = std::fs::read_to_string(config_path(dir.path())).unwrap();

    let err = facade
        .add_shared_command(project, "Job", spec("php queue"))
        .unwrap_err();

    assert!(matches!(err, ConfigWriteError::DuplicateCommand(name) if name == "Job"));
    // The shared file is untouched and the local command is intact — no half-move into both stores.
    assert_eq!(
        std::fs::read_to_string(config_path(dir.path())).unwrap(),
        before,
        "a refused shared add leaves solo.yml untouched"
    );
    assert!(facade
        .project_settings(project)
        .unwrap()
        .local_commands
        .contains_key("Job"));
}

#[test]
fn renaming_a_local_command_onto_a_shared_name_is_refused() {
    let (facade, project, _dir) =
        project_with_yaml("processes:\n  Web:\n    command: npm run dev\n");
    facade
        .add_local_command(project, "Logs", spec("tail -f log"))
        .expect("add local");

    let err = facade
        .rename_local_command(project, "Logs", "Web")
        .unwrap_err();

    assert!(matches!(err, LocalCommandError::Duplicate(name) if name == "Web"));
    assert!(
        facade
            .project_settings(project)
            .unwrap()
            .local_commands
            .contains_key("Logs"),
        "a refused rename leaves the local command under its original name"
    );
}

#[test]
fn a_user_save_auto_trusts_the_command_when_the_setting_is_on() {
    let (facade, project, _dir) =
        project_with_yaml("processes:\n  Web:\n    command: npm run dev\n");
    facade
        .set_project_auto_trust_command_changes(project, true)
        .expect("enable auto-trust");

    let pending = facade
        .add_shared_command(project, "Queue", spec("php queue"))
        .expect("shared add");

    // With auto-trust on, the user's save trusts the command — nothing is left needing trust…
    assert!(
        pending.is_empty(),
        "an auto-trusted save leaves no command needing trust"
    );
    // …and the command's variant is trusted, so it can start without a prompt.
    assert!(
        facade
            .trust()
            .is_trusted(project, &spec("php queue"))
            .unwrap(),
        "the saved command is trusted"
    );
}

#[test]
fn a_user_save_requires_trust_when_the_setting_is_off() {
    // Off is the default — a fresh project never auto-trusts.
    let (facade, project, _dir) =
        project_with_yaml("processes:\n  Web:\n    command: npm run dev\n");

    let pending = facade
        .add_shared_command(project, "Queue", spec("php queue"))
        .expect("shared add");

    assert_eq!(pending.len(), 1, "the new command needs trust");
    assert!(
        !facade
            .trust()
            .is_trusted(project, &spec("php queue"))
            .unwrap(),
        "without auto-trust the saved command stays untrusted"
    );
}

#[test]
fn an_external_solo_yml_edit_never_auto_trusts_even_with_the_setting_on() {
    let (facade, project, dir) =
        project_with_yaml("processes:\n  Web:\n    command: npm run dev\n");
    // Auto-trust is on — but it must apply only to user saves, never to a change made to the file
    // outside Soloist (which arrives via sync, not the façade's write path).
    facade
        .set_project_auto_trust_command_changes(project, true)
        .expect("enable auto-trust");
    let mut events = facade.subscribe();

    // Simulate an external editor adding a command directly to solo.yml, then sync the change.
    std::fs::write(
        config_path(dir.path()),
        "processes:\n  Web:\n    command: npm run dev\n  Queue:\n    command: php queue\n",
    )
    .expect("external edit");
    facade
        .config()
        .sync(project)
        .expect("sync the external edit");

    // The sync flags the new command for trust and does not trust it…
    let event = events.try_recv().expect("a ConfigChanged event");
    match event {
        DomainEvent::ConfigChanged {
            requires_trust,
            commands,
            ..
        } => {
            assert!(requires_trust, "an external command change requires trust");
            assert!(commands.iter().any(|command| command.name == "Queue"));
        }
        other => panic!("expected ConfigChanged, got {other:?}"),
    }
    // …so a change made outside Soloist still requires explicit trust, even with auto-trust on.
    assert!(
        !facade
            .trust()
            .is_trusted(project, &spec("php queue"))
            .unwrap(),
        "an external edit is never auto-trusted"
    );
}

#[test]
fn renaming_a_shared_command_onto_a_local_name_is_refused() {
    let (facade, project, dir) =
        project_with_yaml("processes:\n  Web:\n    command: npm run dev\n");
    facade
        .add_local_command(project, "Logs", spec("tail -f log"))
        .expect("add local");
    let before = std::fs::read_to_string(config_path(dir.path())).unwrap();

    let err = facade
        .rename_shared_command(project, "Web", "Logs")
        .unwrap_err();

    assert!(matches!(err, ConfigWriteError::DuplicateCommand(name) if name == "Logs"));
    assert_eq!(
        std::fs::read_to_string(config_path(dir.path())).unwrap(),
        before,
        "a refused shared rename leaves solo.yml untouched"
    );
}

#[test]
fn editing_a_field_preserves_a_commands_env_block() {
    let original = "processes:\n  Web:  # web server\n    command: npm run dev\n    auto_restart: true\n    env:\n      A: '1'\n      B: '2'\n";
    let (facade, project, dir) = project_with_yaml(original);
    // The settings page needs a durable project record; the first fake registration reuses id 1.
    facade
        .projects()
        .add(dir.path(), None, None)
        .expect("register project");

    // The editor reads the command's view and persists a spec rebuilt from it. The view must carry
    // the env, or the whole-spec replace wipes the committed block on the next unrelated edit.
    let view = command_view(&facade, project, "Web");
    assert_eq!(
        view.env.get("A").map(String::as_str),
        Some("1"),
        "the read-model view carries the command's env"
    );
    let mut edited = spec_from_view(&view);
    edited.auto_start = false; // toggle one unrelated field

    facade
        .edit_shared_command(project, "Web", edited)
        .expect("edit an unrelated field");

    let text = std::fs::read_to_string(config_path(dir.path())).unwrap();
    assert!(
        text.contains("# web server"),
        "the surgical in-place edit preserved the key-line comment"
    );
    let web = &crate::config::parse(&text).unwrap().processes["Web"];
    assert_eq!(
        web.env.get("A").map(String::as_str),
        Some("1"),
        "env A survived"
    );
    assert_eq!(
        web.env.get("B").map(String::as_str),
        Some("2"),
        "env B survived"
    );
    assert!(
        web.auto_restart,
        "an unrelated field the user did not touch is unchanged"
    );
    assert!(!web.auto_start, "the field the user changed took effect");
    assert_eq!(web.command, "npm run dev", "the command is unchanged");
}

#[test]
fn editing_a_command_without_env_adds_no_spurious_env() {
    let (facade, project, dir) =
        project_with_yaml("processes:\n  Web:\n    command: npm run dev\n");
    facade
        .projects()
        .add(dir.path(), None, None)
        .expect("register project");

    let view = command_view(&facade, project, "Web");
    let mut edited = spec_from_view(&view);
    edited.auto_restart = true;
    facade
        .edit_shared_command(project, "Web", edited)
        .expect("edit");

    let text = std::fs::read_to_string(config_path(dir.path())).unwrap();
    assert!(
        !text.contains("env:"),
        "a command with no env stays env-less after an edit"
    );
}

#[test]
fn renaming_a_command_preserves_its_env_block() {
    let original = "processes:\n  Web:\n    command: npm run dev\n    env:\n      A: '1'\n";
    let (facade, project, dir) = project_with_yaml(original);

    facade
        .rename_shared_command(project, "Web", "WebApp")
        .expect("rename");

    let parsed =
        crate::config::parse(&std::fs::read_to_string(config_path(dir.path())).unwrap()).unwrap();
    assert!(parsed.processes.contains_key("WebApp"));
    assert_eq!(
        parsed.processes["WebApp"].env.get("A").map(String::as_str),
        Some("1"),
        "a rename keeps the command's env"
    );
}
