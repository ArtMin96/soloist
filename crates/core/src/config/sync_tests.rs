//! Tests for the per-project `solo.yml` sync engine: an explicit `write` round-trips the
//! comment-preserving editor and refreshes sync state; a re-read diffs the file, flags re-trust by
//! variant, and announces a `ConfigChanged` only on a real change.

use super::*;
use crate::testing::FakeTrustRepo;
use tokio::sync::broadcast::error::TryRecvError;

fn write(path: &std::path::Path, contents: &str) {
    std::fs::write(path, contents).expect("write solo.yml");
}

/// Builds an engine and seeds a project from an initial `solo.yml`, returning
/// the engine, the trust repo, a fresh event receiver, the project id, and root.
fn setup(
    initial: &str,
) -> (
    ConfigEngine,
    Arc<FakeTrustRepo>,
    tokio::sync::broadcast::Receiver<DomainEvent>,
    ProjectId,
    tempfile::TempDir,
) {
    let dir = tempfile::tempdir().expect("temp dir");
    write(&config_path(dir.path()), initial);
    let trust = Arc::new(FakeTrustRepo::new());
    let bus = EventBus::new(16);
    let rx = bus.subscribe();
    let engine = ConfigEngine::new(trust.clone(), bus);
    let project = ProjectId::from_raw(1);
    engine
        .open(project, dir.path().to_path_buf())
        .expect("open seeds state");
    (engine, trust, rx, project, dir)
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
fn write_adds_a_command_to_the_file_and_flags_trust() {
    let (engine, _trust, mut rx, project, dir) =
        setup("processes:\n  Web:\n    command: npm run dev\n");

    let pending = engine
        .write(project, |c| {
            c.processes.insert("Api".into(), spec("cargo run"));
            Ok(())
        })
        .expect("write");

    // The new command needs trust.
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].name, "Api");

    // The file gained exactly the new entry; the existing one is preserved.
    let text = std::fs::read_to_string(config_path(dir.path())).unwrap();
    assert!(text.contains("command: npm run dev"));
    assert!(text.contains("Api:\n    command: cargo run"));

    match rx.try_recv() {
        Ok(DomainEvent::ConfigChanged { requires_trust, .. }) => assert!(requires_trust),
        other => panic!("expected ConfigChanged, got {other:?}"),
    }

    // Sync state is refreshed to our own write, so the watcher's re-read is a no-op.
    assert!(engine.sync(project).expect("sync ok").is_none());
    assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
}

#[test]
fn writing_a_no_op_change_leaves_the_file_untouched() {
    let (engine, _trust, _rx, project, dir) =
        setup("processes:\n  Web:\n    command: npm run dev  # keep this\n");
    let before = std::fs::read_to_string(config_path(dir.path())).unwrap();

    let pending = engine.write(project, |_| Ok(())).expect("write");

    assert!(pending.is_empty());
    assert_eq!(
        std::fs::read_to_string(config_path(dir.path())).unwrap(),
        before,
        "a no-op mutation writes nothing — the file is byte-unchanged"
    );
}

#[test]
fn writing_an_unknown_project_errors() {
    let (engine, ..) = setup("processes:\n  Web:\n    command: x\n");
    assert!(matches!(
        engine.write(ProjectId::from_raw(999), |_| Ok(())),
        Err(ConfigWriteError::UnknownProject)
    ));
}

#[test]
fn adding_a_command_emits_change_requiring_trust() {
    let (engine, _trust, mut rx, project, dir) =
        setup("processes:\n  Web:\n    command: npm run dev\n");
    write(
        &config_path(dir.path()),
        "processes:\n  Web:\n    command: npm run dev\n  Api:\n    command: cargo run\n",
    );

    let changes = engine.sync(project).expect("sync ok").expect("a change");
    assert_eq!(changes.added, vec!["Api"]);

    match rx.try_recv() {
        Ok(DomainEvent::ConfigChanged {
            requires_trust,
            diff,
            ..
        }) => {
            assert!(requires_trust, "a new untrusted command requires trust");
            assert_eq!(diff.added, vec!["Api"]);
        }
        other => panic!("expected ConfigChanged, got {other:?}"),
    }
}

#[test]
fn the_change_event_carries_the_untrusted_command_detail() {
    let (engine, _trust, mut rx, project, dir) =
        setup("processes:\n  Web:\n    command: npm run dev\n");
    write(
        &config_path(dir.path()),
        "processes:\n  Web:\n    command: npm run dev\n  Api:\n    command: cargo run\n    working_dir: api\n    env:\n      PORT: '4000'\n",
    );

    engine.sync(project).expect("sync ok").expect("a change");

    match rx.try_recv() {
        Ok(DomainEvent::ConfigChanged { commands, .. }) => {
            assert_eq!(
                commands.len(),
                1,
                "only the new untrusted command is pending"
            );
            let api = &commands[0];
            assert_eq!(api.name, "Api");
            assert_eq!(api.command, "cargo run");
            assert_eq!(api.working_dir.as_deref(), Some("api"));
            assert_eq!(api.env.get("PORT").map(String::as_str), Some("4000"));
        }
        other => panic!("expected ConfigChanged, got {other:?}"),
    }
}

#[test]
fn renaming_a_trusted_command_preserves_trust() {
    let (engine, trust, mut rx, project, dir) =
        setup("processes:\n  Web:\n    command: npm run dev\n");
    // Trust Web's current variant.
    let web = crate::config::parse("processes:\n  Web:\n    command: npm run dev\n").unwrap();
    trust
        .set_trusted(project, &web.processes["Web"].variant_hash())
        .unwrap();

    write(
        &config_path(dir.path()),
        "processes:\n  Frontend:\n    command: npm run dev\n",
    );
    let changes = engine.sync(project).expect("sync ok").expect("a change");
    assert_eq!(changes.renamed.len(), 1);
    assert!(changes.added.is_empty() && changes.removed.is_empty());

    match rx.try_recv() {
        Ok(DomainEvent::ConfigChanged { requires_trust, .. }) => {
            assert!(!requires_trust, "a pure rename keeps trust");
        }
        other => panic!("expected ConfigChanged, got {other:?}"),
    }
}

#[test]
fn a_rename_that_also_edits_the_variant_still_requires_trust() {
    let (engine, trust, mut rx, project, dir) =
        setup("processes:\n  Web:\n    command: npm run dev\n");
    // Trust Web's original variant (no env).
    let original = crate::config::parse("processes:\n  Web:\n    command: npm run dev\n").unwrap();
    trust
        .set_trusted(project, &original.processes["Web"].variant_hash())
        .unwrap();

    // Rename Web -> Frontend (same command, so it is detected as a rename) but
    // also add an env var — the variant changes, so trust must be re-confirmed.
    write(
        &config_path(dir.path()),
        "processes:\n  Frontend:\n    command: npm run dev\n    env:\n      PORT: '3000'\n",
    );
    let changes = engine.sync(project).expect("sync ok").expect("a change");
    assert_eq!(changes.renamed.len(), 1, "still classified as a rename");

    match rx.try_recv() {
        Ok(DomainEvent::ConfigChanged { requires_trust, .. }) => {
            assert!(
                requires_trust,
                "a rename that changes the variant needs re-trust"
            );
        }
        other => panic!("expected ConfigChanged, got {other:?}"),
    }
}

#[test]
fn touching_without_changing_bytes_is_a_no_op() {
    let (engine, _trust, mut rx, project, dir) =
        setup("processes:\n  Web:\n    command: npm run dev\n");
    // Rewrite identical bytes.
    write(
        &config_path(dir.path()),
        "processes:\n  Web:\n    command: npm run dev\n",
    );
    assert!(engine.sync(project).expect("sync ok").is_none());
    assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
}

#[test]
fn syncing_an_unknown_project_is_a_no_op() {
    let (engine, _trust, _rx, _project, _dir) =
        setup("processes:\n  Web:\n    command: npm run dev\n");
    assert!(engine
        .sync(ProjectId::from_raw(999))
        .expect("sync ok")
        .is_none());
}
