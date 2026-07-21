//! Acceptance tests for opening a terminal: it registers as an ungated `Terminal` and starts,
//! and its label is unique within the project so several open terminals stay tellable apart.

use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::composition::CorePorts;
use crate::config::ProcessSpec;
use crate::events::DomainEvent;
use crate::ids::ProjectId;
use crate::ports::TokioClock;
use crate::process::{ProcStatus, ProcessView};
use crate::testing::{wait_all, FakeProjectRepo, FakeSpawner, FakeTrustRepo};

use tokio::sync::broadcast;
use tokio::time::timeout;

/// How long a started terminal is given to reach `Running` against the fake spawner, which
/// needs none of it — generous enough never to flake under load, short enough that a process
/// that never starts reports a failure instead of hanging.
const START_GRACE: Duration = Duration::from_secs(5);

/// A façade over fakes with a long-lived child, so a started terminal stays `Running` for the
/// assertions instead of racing its own exit.
fn facade() -> Facade {
    Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .build(),
    )
}

/// The façade with one project opened on a temp directory. The directory is returned so it
/// outlives the test body — dropping it would delete the project root mid-run.
fn facade_with_project() -> (Facade, ProjectId, tempfile::TempDir) {
    let facade = facade();
    let dir = tempfile::tempdir().expect("temp dir");
    let project = facade.load_project(dir.path()).expect("load project");
    (facade, project.id, dir)
}

fn view_of(facade: &Facade, id: ProcessId) -> ProcessView {
    facade
        .snapshot()
        .into_iter()
        .find(|view| view.id == id)
        .expect("the created terminal is in the snapshot")
}

fn label_of(facade: &Facade, id: ProcessId) -> String {
    view_of(facade, id).label
}

#[tokio::test]
async fn a_new_terminal_is_an_ungated_terminal_kind_process_that_starts() {
    let (facade, project, _dir) = facade_with_project();
    let mut events = facade.subscribe();

    let id = facade.create_terminal(project).expect("create terminal");

    let view = view_of(&facade, id);
    assert_eq!(view.kind, ProcessKind::Terminal);
    assert_eq!(view.label, "Terminal");
    assert!(
        !view.requires_trust,
        "a terminal is the user's own shell, so it is never trust-gated"
    );
    assert!(
        !view.resumable,
        "a shell has no agent session to resume, so it offers only Start"
    );
    // Creating starts it: the caller lands on a live shell, not a stopped row. Bounded so a
    // terminal that is registered but never started fails here rather than hanging the suite.
    assert!(
        timeout(
            START_GRACE,
            wait_all(&mut events, &[id], ProcStatus::Running)
        )
        .await
        .is_ok(),
        "the created terminal reached Running"
    );
}

#[tokio::test]
async fn terminals_are_numbered_so_several_stay_tellable_apart() {
    let (facade, project, _dir) = facade_with_project();

    let first = facade.create_terminal(project).expect("first");
    let second = facade.create_terminal(project).expect("second");
    let third = facade.create_terminal(project).expect("third");

    assert_eq!(label_of(&facade, first), "Terminal");
    assert_eq!(label_of(&facade, second), "Terminal 2");
    assert_eq!(label_of(&facade, third), "Terminal 3");
}

#[tokio::test]
async fn a_closed_terminals_number_is_reused_rather_than_left_a_gap() {
    // Numbering reads the labels actually in use, not a running counter, so closing the middle
    // terminal frees its name for the next one instead of climbing forever.
    let (facade, project, _dir) = facade_with_project();
    let first = facade.create_terminal(project).expect("first");
    let second = facade.create_terminal(project).expect("second");
    let third = facade.create_terminal(project).expect("third");

    facade.supervisor().close(second).await.expect("close");
    let fourth = facade.create_terminal(project).expect("fourth");

    assert_eq!(label_of(&facade, fourth), "Terminal 2");
    assert_eq!(label_of(&facade, first), "Terminal");
    assert_eq!(label_of(&facade, third), "Terminal 3");
}

#[tokio::test]
async fn a_terminal_steps_over_a_command_that_already_holds_its_name() {
    // Uniqueness spans every kind: a `solo.yml` command named "Terminal" is a different process
    // in the same sidebar, so the new terminal must not render under the same label.
    let (facade, project, dir) = facade_with_project();
    let spec = ProcessSpec {
        command: "sleep 60".into(),
        working_dir: None,
        auto_start: false,
        auto_restart: false,
        restart_when_changed: Vec::new(),
        env: Default::default(),
    };
    facade.supervisor().register(Registration::command(
        project,
        dir.path(),
        "Terminal",
        &spec,
    ));

    let id = facade.create_terminal(project).expect("create terminal");

    assert_eq!(label_of(&facade, id), "Terminal 2");
}

#[tokio::test]
async fn each_project_numbers_its_own_terminals() {
    // Labels only have to be unique within the project they render under, so a second project
    // starts at "Terminal" rather than continuing the first project's count.
    let facade = facade();
    let first_dir = tempfile::tempdir().expect("temp dir");
    let second_dir = tempfile::tempdir().expect("temp dir");
    let first = facade.load_project(first_dir.path()).expect("load first");
    let second = facade.load_project(second_dir.path()).expect("load second");

    facade.create_terminal(first.id).expect("first project");
    let other = facade.create_terminal(second.id).expect("second project");

    assert_eq!(label_of(&facade, other), "Terminal");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn terminals_created_concurrently_never_land_on_one_label() {
    // The façade runs store-touching ops on the blocking pool, so two `create_terminal` calls
    // genuinely overlap. Naming is only unique if the label is chosen under the same lock that
    // files the process: a caller that read the labels first and registered second could be
    // overtaken in between, and both would file "Terminal". The barrier releases every task at
    // once so the calls overlap where that window used to be.
    const TERMINALS: usize = 8;
    let (facade, project, _dir) = facade_with_project();
    let facade = Arc::new(facade);
    let barrier = Arc::new(std::sync::Barrier::new(TERMINALS));

    let mut handles = Vec::with_capacity(TERMINALS);
    for _ in 0..TERMINALS {
        let facade = Arc::clone(&facade);
        let barrier = Arc::clone(&barrier);
        handles.push(tokio::task::spawn_blocking(move || {
            barrier.wait();
            facade.create_terminal(project).expect("create terminal")
        }));
    }
    let mut ids = Vec::with_capacity(TERMINALS);
    for handle in handles {
        ids.push(handle.await.expect("the creating task did not panic"));
    }

    let labels: std::collections::HashSet<String> =
        ids.iter().map(|&id| label_of(&facade, id)).collect();
    assert_eq!(
        labels.len(),
        TERMINALS,
        "every concurrently created terminal took its own label, got {labels:?}"
    );
}

#[tokio::test]
async fn creating_a_terminal_in_an_unknown_project_is_refused() {
    let facade = facade();

    assert!(matches!(
        facade.create_terminal(ProjectId::from_raw(9999)),
        Err(CreateTerminalError::UnknownProject)
    ));
}

#[tokio::test]
async fn creating_a_terminal_announces_it_on_the_bus() {
    // The sidebar row is built from `ProcessSpawned`, never from the create call's return, so a
    // terminal that registered without announcing itself would never appear.
    let (facade, project, _dir) = facade_with_project();
    let mut events = facade.subscribe();

    let id = facade.create_terminal(project).expect("create terminal");

    let spawned = next_spawned(&mut events).await;
    assert!(
        matches!(
            spawned,
            DomainEvent::ProcessSpawned { id: spawned, kind, ref label, .. }
                if spawned == id && kind == ProcessKind::Terminal && label == "Terminal"
        ),
        "the announcement carries the new terminal's identity: {spawned:?}"
    );
}

async fn next_spawned(rx: &mut broadcast::Receiver<DomainEvent>) -> DomainEvent {
    crate::testing::next_matching(rx, |event| {
        matches!(event, DomainEvent::ProcessSpawned { .. })
    })
    .await
}
