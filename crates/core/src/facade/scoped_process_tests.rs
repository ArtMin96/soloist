//! What `spawn_process` admits, what it refuses, and — the load-bearing half — what a refusal
//! leaves behind. The gate runs before the process is registered, so every refusal here also
//! asserts that nothing was created.

use super::*;

use std::collections::BTreeMap;
use std::path::Path;

use crate::config::{InvalidCommand, ProcessSpec};
use crate::facade::Facade;
use crate::ids::{ProjectId, SessionId};
use crate::process::{ProcStatus, ProcessKind};
use crate::testing::{agent_registration, bound_session, facade_with_agent_tool, TEST_PEER_PGID};
use crate::PeerCredentials;
use tokio::sync::broadcast::error::TryRecvError;

/// The command every spawn below runs. Self-contained and never really executed — the
/// [`FakeSpawner`](crate::testing::FakeSpawner) stands in for the child.
const COMMAND: &str = "sleep 60";

/// The spec a spawn of `command` at the project root produces, for granting the trust its gate
/// looks for. Mirrors [`request`] field for field — including the **raw** `working_dir` — because
/// that is exactly what the variant hash digests.
fn spec(command: &str) -> ProcessSpec {
    ProcessSpec {
        command: command.into(),
        working_dir: None,
        auto_start: false,
        auto_restart: false,
        restart_when_changed: Vec::new(),
        env: BTreeMap::new(),
    }
}

fn request(command: &str) -> SpawnProcessRequest {
    SpawnProcessRequest {
        command: command.into(),
        working_dir: None,
        env: BTreeMap::new(),
        label: None,
    }
}

/// A façade with one project and one agent tool, with [`COMMAND`]'s variant already trusted in
/// that project — the state a user reaches by approving the command in the app.
fn facade_trusting_the_command() -> (Facade, ProjectId) {
    let (facade, project) = facade_with_agent_tool();
    facade
        .trust()
        .trust(project, &spec(COMMAND))
        .expect("trust the command variant in the project");
    (facade, project)
}

/// A caller that has proven nothing about itself beyond the group it connects from. Its scope
/// resolves to the sole open project, as an external agent's does.
fn unbound_session(facade: &Facade) -> SessionId {
    facade.open_session(PeerCredentials::in_group(TEST_PEER_PGID))
}

/// A lead: a registered agent with a session bound to it, the caller a real spawn comes from.
fn lead_session(facade: &Facade, project: ProjectId) -> (ProcessId, SessionId) {
    let lead = facade
        .supervisor()
        .register(agent_registration(project, "lead"));
    let session = bound_session(facade, lead, TEST_PEER_PGID);
    (lead, session)
}

/// Writes a `solo.yml` with one command named "Web" and returns the directory holding it.
fn project_dir_with_web_command() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary project directory");
    std::fs::write(
        dir.path().join("solo.yml"),
        "processes:\n  Web:\n    command: npm run dev\n",
    )
    .expect("write the solo.yml");
    dir
}

#[tokio::test]
async fn a_trusted_command_spawns_and_starts_in_the_session_project() {
    let (facade, project) = facade_trusting_the_command();
    let session = unbound_session(&facade);

    let id = facade
        .scoped(session)
        .spawn_process(request(COMMAND))
        .expect("a trusted command spawns");

    let view = facade
        .process_view(id)
        .expect("the spawned process is registered");
    assert_eq!(
        view.project, project,
        "a spawn lands in the session's scope"
    );
    assert_eq!(
        view.kind,
        ProcessKind::Command,
        "a spawn is a trust-gated Command, never an ungated Terminal",
    );
    assert!(
        !view.requires_trust,
        "the variant was trusted before it was registered",
    );
    assert_ne!(
        view.status,
        ProcStatus::Stopped,
        "spawning starts the process, it does not merely register it",
    );
    assert_eq!(
        view.label, "sleep",
        "an unnamed spawn is filed under the command's first word",
    );
}

#[tokio::test]
async fn an_untrusted_command_is_refused_and_registers_nothing() {
    // No trust granted: the variant is exactly the one the caller asks to run, untrusted.
    let (facade, _project) = facade_with_agent_tool();
    let session = unbound_session(&facade);
    let mut events = facade.subscribe();
    let registered_before = facade.snapshot().len();

    assert!(matches!(
        facade.scoped(session).spawn_process(request(COMMAND)),
        Err(SpawnProcessError::Untrusted)
    ));
    // The refusal alone proves nothing about ordering: registering first and letting `start`
    // refuse produces the same error while leaving a permanent row in the sidebar. These two
    // assertions are what pin the gate ahead of the registration.
    assert_eq!(
        facade.snapshot().len(),
        registered_before,
        "a refused spawn registers nothing",
    );
    assert!(
        matches!(events.try_recv(), Err(TryRecvError::Empty)),
        "a refused spawn announces no process",
    );
}

#[tokio::test]
async fn a_variant_trusted_in_another_project_is_refused() {
    let (facade, project) = facade_with_agent_tool();
    let elsewhere = facade
        .projects()
        .add(Path::new("/tmp"), None, None)
        .expect("open a second project")
        .id;
    facade
        .trust()
        .trust(elsewhere, &spec(COMMAND))
        .expect("trust the variant in the other project");
    // Two projects are open, so the caller's scope comes from the process it is bound to.
    let (_lead, session) = lead_session(&facade, project);
    let registered_before = facade.snapshot().len();

    assert!(matches!(
        facade.scoped(session).spawn_process(request(COMMAND)),
        Err(SpawnProcessError::Untrusted)
    ));
    assert_eq!(
        facade.snapshot().len(),
        registered_before,
        "a refused spawn registers nothing",
    );
}

#[tokio::test]
async fn spawning_without_a_project_in_scope_is_refused() {
    let (facade, _project) = facade_trusting_the_command();
    // A second open project leaves an unbound caller nothing to resolve its scope to; the
    // command itself is trusted, so only the missing scope can refuse this.
    facade
        .projects()
        .add(Path::new("/tmp"), None, None)
        .expect("open a second project");
    let session = unbound_session(&facade);
    let registered_before = facade.snapshot().len();

    assert!(matches!(
        facade.scoped(session).spawn_process(request(COMMAND)),
        Err(SpawnProcessError::NoProjectScope)
    ));
    assert_eq!(
        facade.snapshot().len(),
        registered_before,
        "a refused spawn registers nothing",
    );
}

#[tokio::test]
async fn a_spawned_worker_may_not_spawn_a_process() {
    let (facade, project) = facade_trusting_the_command();
    let (_lead, lead_session) = lead_session(&facade, project);
    let worker = facade
        .scoped(lead_session)
        .spawn_agent("worker", Vec::new())
        .expect("a lead spawns a worker");
    let worker_session = bound_session(&facade, worker, TEST_PEER_PGID + 1);
    let registered_before = facade.snapshot().len();

    assert!(matches!(
        facade
            .scoped(worker_session)
            .spawn_process(request(COMMAND)),
        Err(SpawnProcessError::WorkerMayNotSpawn)
    ));
    assert_eq!(
        facade.snapshot().len(),
        registered_before,
        "a refused spawn registers nothing",
    );
}

#[tokio::test]
async fn a_process_spawned_by_spawn_process_may_not_itself_spawn() {
    let (facade, project) = facade_trusting_the_command();
    let (_lead, lead_session) = lead_session(&facade, project);
    let spawned = facade
        .scoped(lead_session)
        .spawn_process(request(COMMAND))
        .expect("a lead spawns a process");

    // The delegation rule closes over the new surface because the spawn recorded its lineage.
    let spawned_session = bound_session(&facade, spawned, TEST_PEER_PGID + 1);
    assert!(matches!(
        facade
            .scoped(spawned_session)
            .spawn_process(request(COMMAND)),
        Err(SpawnProcessError::WorkerMayNotSpawn)
    ));
}

#[tokio::test]
async fn the_trust_variant_matches_a_configured_commands_variant() {
    let dir = project_dir_with_web_command();
    let (facade, _seed) = facade_with_agent_tool();
    let project = facade
        .load_project(dir.path())
        .expect("open the project from disk")
        .id;
    // Trust granted the one way the app grants it: by name, against the loaded `solo.yml`.
    facade
        .trust_command(project, "Web")
        .expect("trust the configured command");
    let (_lead, session) = lead_session(&facade, project);

    // The configured command's `working_dir` is absent, and so is the caller's. Hashing the
    // caller's value resolved against the project root instead of as written would produce a
    // digest that no trusted variant can equal, and this spawn would be refused.
    facade
        .scoped(session)
        .spawn_process(SpawnProcessRequest {
            command: "npm run dev".into(),
            working_dir: None,
            env: BTreeMap::new(),
            label: None,
        })
        .expect("a variant the user already trusted for the configured command spawns");
}

#[tokio::test]
async fn a_process_spawned_by_a_lead_nests_under_it() {
    let (facade, project) = facade_trusting_the_command();
    let (lead, session) = lead_session(&facade, project);

    let id = facade
        .scoped(session)
        .spawn_process(request(COMMAND))
        .expect("a lead spawns a process");

    let snapshot = facade
        .orchestration_snapshot(project)
        .expect("the orchestration read model");
    let node = snapshot
        .agents
        .iter()
        .find(|node| node.id == id)
        .expect("the spawned process is a node in the tree");
    assert_eq!(node.parent, Some(lead), "a spawn nests under its lead");
    assert_eq!(node.kind, ProcessKind::Command);
}

/// Orphan adoption matches a leftover process group on project root, label, and command line
/// together (`Registry::find_resting_match`). A spawn uses the real project root, like a
/// configured command, so the label is what has to differ — and numbering at registration time
/// guarantees it does, whatever name the caller asked for.
#[tokio::test]
async fn a_spawned_process_cannot_take_a_configured_commands_label() {
    let dir = project_dir_with_web_command();
    let (facade, _seed) = facade_with_agent_tool();
    let project = facade
        .load_project(dir.path())
        .expect("open the project from disk")
        .id;
    facade
        .trust_command(project, "Web")
        .expect("trust the configured command");
    let (_lead, session) = lead_session(&facade, project);

    let id = facade
        .scoped(session)
        .spawn_process(SpawnProcessRequest {
            command: "npm run dev".into(),
            working_dir: None,
            env: BTreeMap::new(),
            label: Some("Web".into()),
        })
        .expect("the spawn is admitted; only its name is contested");

    assert_eq!(
        facade.process_view(id).expect("registered").label,
        "Web 2",
        "the configured command already holds \"Web\", so the spawn is numbered",
    );
    assert!(
        facade
            .snapshot()
            .iter()
            .any(|view| view.id != id && view.project == project && view.label == "Web"),
        "the configured command keeps its own name and its adoption identity with it",
    );
}

#[tokio::test]
async fn a_blank_command_is_refused() {
    let (facade, _project) = facade_with_agent_tool();
    let session = unbound_session(&facade);
    let registered_before = facade.snapshot().len();

    assert!(matches!(
        facade.scoped(session).spawn_process(request("   ")),
        Err(SpawnProcessError::InvalidCommand(
            InvalidCommand::BlankCommand
        ))
    ));
    assert_eq!(
        facade.snapshot().len(),
        registered_before,
        "a refused spawn registers nothing",
    );
}

#[tokio::test]
async fn a_blank_label_is_refused() {
    let (facade, _project) = facade_trusting_the_command();
    let session = unbound_session(&facade);
    let registered_before = facade.snapshot().len();

    assert!(matches!(
        facade.scoped(session).spawn_process(SpawnProcessRequest {
            command: COMMAND.into(),
            working_dir: None,
            env: BTreeMap::new(),
            label: Some("  ".into()),
        }),
        Err(SpawnProcessError::InvalidCommand(InvalidCommand::BlankName))
    ));
    assert_eq!(
        facade.snapshot().len(),
        registered_before,
        "a refused spawn registers nothing",
    );
}

/// A spawned command holds no mailbox — it never appears in `agent_roster` and is never
/// idle-tracked — so a briefing queued for it could never be retrieved or acknowledged. The
/// spawn therefore queues none, and this is the test that keeps it that way.
#[tokio::test]
async fn a_spawned_process_is_queued_no_onboarding_briefing() {
    let (facade, project) = facade_trusting_the_command();
    let (_lead, session) = lead_session(&facade, project);

    let id = facade
        .scoped(session)
        .spawn_process(request(COMMAND))
        .expect("a lead spawns a process");

    assert!(
        !facade.mailbox.wake(id, facade.supervisor()),
        "a spawned command has nothing queued to wake it with",
    );
}

/// The delegation gate and the lineage record are the only things `spawn_agent` and
/// `spawn_process` share; this pins that sharing the gate did not change what a lead may do.
#[tokio::test]
async fn a_lead_may_still_spawn_after_the_gate_is_shared() {
    let (facade, project) = facade_trusting_the_command();
    let (_lead, session) = lead_session(&facade, project);

    facade
        .scoped(session)
        .spawn_agent("worker", Vec::new())
        .expect("a lead still spawns agents");
    facade
        .scoped(session)
        .spawn_process(request(COMMAND))
        .expect("and processes");
}
