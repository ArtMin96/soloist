//! Behavioural tests for drafting a commit message with the user's own agent tool. They assemble a
//! real [`Facade`] over fakes, so what is asserted is what a Tauri command or a tool would get back
//! — including the refusals a draft produces without running anything.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use tempfile::TempDir;
use tokio::time::timeout;

use crate::agents::{AgentKind, AgentTool, OneShotError, PromptMode};
use crate::composition::CorePorts;
use crate::coordination::{TodoDoc, TodoRepo, TodoStatus};
use crate::facade::Facade;
use crate::ids::{ProcessId, ProjectId, TodoId};
use crate::ports::{ProjectRepo, TokioClock};
use crate::process::{ProcStatus, ProcessKind};
use crate::settings::Assist;
use crate::shellenv::{NoopShellEnvProbe, ShellEnvProbe};
use crate::supervisor::Registration;
use crate::testing::{
    agent_registration, file_change, git_status, raw_diff, wait_all, FakeAgentOneShot,
    FakeAgentToolRepo, FakeGitRepository, FakeProjectRepo, FakeSettingsRepo, FakeShellEnvProbe,
    FakeSpawner, FakeTodoRepo, FakeTrustRepo,
};
use crate::vcs::ChangeKind;

use super::DraftError;

const PATH: &str = "src/main.rs";

const HEADER: &str =
    "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n";

const ASSIST_TOOL: &str = "My CLI";

const DRAFTED: &str = "Record the index";

/// A directory an agent CLI is only ever on the user's interactive `PATH` from.
const VERSION_MANAGER_BIN: &str = "/home/dev/.nvm/versions/node/v20.0.0/bin";

/// Long enough for a fake spawner to report a start, short enough that a launch that never
/// resolves an environment fails in seconds instead of hanging the suite.
const SPAWN_WAIT: Duration = Duration::from_secs(10);

/// The line that introduces what a change was for, so a test can tell a prompt that claims to know
/// from one that does not.
const INTENT_MARKER: &str = "What this work set out to do";

fn assist_tool() -> AgentTool {
    AgentTool {
        name: ASSIST_TOOL.to_string(),
        command: "mycli".to_string(),
        default_args: Vec::new(),
        kind: AgentKind::Generic,
        prompt_mode: PromptMode::Stdin,
    }
}

/// A façade over a trusted project with one path staged, the given registry, and `one_shot` behind
/// the drafting port. Durable settings are real (in memory), so what a test selects is what the
/// façade reads back.
fn facade_for_drafting(
    tools: Vec<AgentTool>,
    one_shot: Arc<FakeAgentOneShot>,
) -> (Arc<Facade>, ProjectId, TempDir) {
    let (facade, project, _todos, dir) = facade_for_drafting_with_todos(tools, one_shot);
    (facade, project, dir)
}

/// The same façade with a durable todo store behind it — for the tests about what a change is said
/// to have been for, where the answer comes from which of them a running agent holds a lock on.
fn facade_for_drafting_with_todos(
    tools: Vec<AgentTool>,
    one_shot: Arc<FakeAgentOneShot>,
) -> (Arc<Facade>, ProjectId, Arc<FakeTodoRepo>, TempDir) {
    let todos = Arc::new(FakeTodoRepo::new());
    let (facade, project, dir) = facade_for_drafting_over(
        tools,
        one_shot,
        Arc::new(NoopShellEnvProbe),
        todos.clone(),
        FakeSpawner::exits_on_terminate(),
    );
    (facade, project, todos, dir)
}

/// The same façade, resolving the environment a launch and a headless run are made in through
/// `shell_env_probe` — for the test that proves both consult the one cache.
fn facade_for_drafting_capturing(
    tools: Vec<AgentTool>,
    one_shot: Arc<FakeAgentOneShot>,
    shell_env_probe: Arc<dyn ShellEnvProbe>,
) -> (Arc<Facade>, ProjectId, TempDir) {
    facade_for_drafting_over(
        tools,
        one_shot,
        shell_env_probe,
        Arc::new(FakeTodoRepo::new()),
        FakeSpawner::exits_on_terminate(),
    )
}

/// Every drafting façade, assembled once.
fn facade_for_drafting_over(
    tools: Vec<AgentTool>,
    one_shot: Arc<FakeAgentOneShot>,
    shell_env_probe: Arc<dyn ShellEnvProbe>,
    todo_repo: Arc<FakeTodoRepo>,
    spawner: FakeSpawner,
) -> (Arc<Facade>, ProjectId, TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let projects = Arc::new(FakeProjectRepo::new());
    let root = dir.path().canonicalize().expect("canonical root");
    let project = projects.upsert(&root, None, None).expect("add project").id;
    let mut status = git_status("main");
    status
        .changes
        .push(file_change(PATH, Some(ChangeKind::Modified), None));
    let ports = CorePorts::builder(
        Arc::new(spawner),
        Arc::new(TokioClock),
        Arc::new(FakeTrustRepo::new().trusting_project(project)),
        projects,
    )
    .git_repository(Arc::new(
        FakeGitRepository::reporting(status)
            .diffing(raw_diff(HEADER, &["@@ -1,1 +1,1 @@\n-old\n+new\n"])),
    ))
    .agent_tools(Arc::new(FakeAgentToolRepo::new(tools)))
    .agent_one_shot(one_shot)
    .shell_env_probe(shell_env_probe)
    .todo_repo(todo_repo)
    .settings_repo(Arc::new(FakeSettingsRepo::new()));
    (Arc::new(Facade::new(ports.build())), project, dir)
}

#[tokio::test]
async fn with_a_tool_selected_a_draft_comes_back_describing_what_is_staged() {
    let one_shot = Arc::new(FakeAgentOneShot::answering(DRAFTED));
    let (facade, project, _dir) = facade_for_drafting(vec![assist_tool()], one_shot.clone());
    facade
        .set_assist_settings(Assist {
            tool: Some(ASSIST_TOOL.to_string()),
        })
        .expect("select");

    let drafted = facade
        .git_draft_commit_message(project)
        .await
        .expect("a draft");

    assert_eq!(drafted, DRAFTED);
    let subject = one_shot.subjects().pop().expect("one run");
    assert!(
        subject.contains("+new"),
        "the tool was given the staged change to describe: {subject}",
    );
}

#[tokio::test]
async fn a_draft_records_nothing_and_commits_nothing() {
    // The whole safety property of the feature: what comes back is text for a person to read and
    // change. Nothing about asking for it touches the index or the history.
    let one_shot = Arc::new(FakeAgentOneShot::answering(DRAFTED));
    let (facade, project, _dir) = facade_for_drafting(vec![assist_tool()], one_shot);
    facade
        .set_assist_settings(Assist {
            tool: Some(ASSIST_TOOL.to_string()),
        })
        .expect("select");

    facade
        .git_draft_commit_message(project)
        .await
        .expect("a draft");

    let status = facade
        .git_status(project)
        .expect("read")
        .expect("a repository");
    assert_eq!(
        status.changes.len(),
        1,
        "the working tree is exactly as it was",
    );
}

#[tokio::test]
async fn with_no_tool_selected_nothing_is_run_at_all() {
    // The opt-in half of the feature, asserted where it matters: not merely that the call is
    // refused, but that the refusal happens before anything is run — neither the agent, nor the
    // shell whose environment a run would be made in. Resolving that environment before knowing
    // there is anything to run would cost a person's machine a shell startup for an action nobody
    // can take.
    let one_shot = Arc::new(FakeAgentOneShot::answering(DRAFTED));
    let probe = Arc::new(FakeShellEnvProbe::returning(BTreeMap::new()));
    let (facade, project, _dir) =
        facade_for_drafting_capturing(vec![assist_tool()], one_shot.clone(), probe.clone());

    let refusal = facade.git_draft_commit_message(project).await.unwrap_err();

    assert!(matches!(refusal, DraftError::NoAssistTool), "{refusal:?}",);
    assert!(one_shot.runs().is_empty());
    assert_eq!(probe.calls(), 0, "and no shell was started either");
}

#[tokio::test]
async fn a_selected_tool_that_is_no_longer_configured_is_named_rather_than_guessed_at() {
    // A tool can be renamed or removed after it was picked. Falling back to another one would run
    // something the user never chose.
    let one_shot = Arc::new(FakeAgentOneShot::answering(DRAFTED));
    let (facade, project, _dir) = facade_for_drafting(vec![assist_tool()], one_shot.clone());
    facade
        .set_assist_settings(Assist {
            tool: Some("Some Other CLI".to_string()),
        })
        .expect("select");

    let refusal = facade.git_draft_commit_message(project).await.unwrap_err();

    assert!(matches!(refusal, DraftError::UnknownTool), "{refusal:?}",);
    assert!(one_shot.runs().is_empty());
}

#[tokio::test]
async fn a_tool_that_never_answers_surfaces_the_timeout_it_was_stopped_at() {
    let one_shot = Arc::new(FakeAgentOneShot::refusing(OneShotError::Timeout));
    let (facade, project, _dir) = facade_for_drafting(vec![assist_tool()], one_shot);
    facade
        .set_assist_settings(Assist {
            tool: Some(ASSIST_TOOL.to_string()),
        })
        .expect("select");

    let refusal = facade.git_draft_commit_message(project).await.unwrap_err();

    assert!(
        matches!(refusal, DraftError::OneShot(OneShotError::Timeout)),
        "a surface can only say what went wrong if the reason survives the trip: {refusal:?}",
    );
}

#[tokio::test]
async fn a_draft_asked_for_after_a_launch_runs_no_second_shell() {
    // The one resolver, asserted where it is observable rather than assumed from how the façade is
    // assembled. A launch resolves the environment it starts a process in; a draft asked for
    // straight afterwards must find that answer already there. Two resolvers would each capture,
    // which on a shell that takes seconds to start is a person waiting twice for one commit
    // message.
    let probe = Arc::new(FakeShellEnvProbe::returning(BTreeMap::from([(
        "PATH".to_string(),
        VERSION_MANAGER_BIN.to_string(),
    )])));
    let one_shot = Arc::new(FakeAgentOneShot::answering(DRAFTED));
    let (facade, project, _dir) =
        facade_for_drafting_capturing(vec![assist_tool()], one_shot.clone(), probe.clone());
    facade
        .set_assist_settings(Assist {
            tool: Some(ASSIST_TOOL.to_string()),
        })
        .expect("select");
    let mut events = facade.subscribe();

    let launched = facade
        .launch_agent(project, ASSIST_TOOL, Vec::new())
        .expect("launch");
    timeout(
        SPAWN_WAIT,
        wait_all(&mut events, &[launched], ProcStatus::Running),
    )
    .await
    .expect("the launched process reached Running");
    assert_eq!(probe.calls(), 1, "the launch captured the shell once");

    facade
        .git_draft_commit_message(project)
        .await
        .expect("a draft");

    assert_eq!(
        probe.calls(),
        1,
        "the draft read the environment the launch had already resolved",
    );
    assert_eq!(
        one_shot.environments().pop().expect("one run").get("PATH"),
        Some(&VERSION_MANAGER_BIN.to_string()),
        "and it is that captured environment the tool was run in",
    );
}

/// Registers `registration` and waits for it to be running — a process that has been asked to start
/// is not one that is running, and what a draft asks about is what is running now. The wait is
/// bounded, so a process that never starts fails in seconds rather than hanging the suite.
async fn started(facade: &Arc<Facade>, registration: Registration) -> ProcessId {
    let mut events = facade.subscribe();
    let id = facade.supervisor().register(registration);
    facade.supervisor().start(id).expect("start the process");
    timeout(
        SPAWN_WAIT,
        wait_all(&mut events, &[id], ProcStatus::Running),
    )
    .await
    .expect("the process reached Running");
    id
}

/// A task of `project` in the state an agent that picked it up would leave it.
fn task(facade: &Arc<Facade>, project: ProjectId, title: &str, body: &str) -> TodoId {
    facade
        .todo_create_in(
            project,
            TodoDoc {
                title: title.to_string(),
                body: body.to_string(),
                status: TodoStatus::InProgress,
            },
            None,
        )
        .expect("create a task")
        .id
}

/// Selects the assist tool, so a draft is asked for rather than refused.
fn drafting_with(facade: &Arc<Facade>) {
    facade
        .set_assist_settings(Assist {
            tool: Some(ASSIST_TOOL.to_string()),
        })
        .expect("select");
}

#[tokio::test]
async fn the_task_a_running_agent_holds_is_what_the_change_is_said_to_be_for() {
    // The feature: a diff records what moved and never why, and Soloist is the only thing in the
    // room that knows what the agent making it was asked to do.
    let one_shot = Arc::new(FakeAgentOneShot::answering(DRAFTED));
    let (facade, project, todos, _dir) =
        facade_for_drafting_with_todos(vec![assist_tool()], one_shot.clone());
    drafting_with(&facade);
    let agent = started(&facade, agent_registration(project, "worker")).await;
    let picked_up = task(
        &facade,
        project,
        "Honour the repository's own commit template",
        "Git consults a template only when it would open an editor.",
    );
    todos
        .lock(project, picked_up, agent)
        .expect("lock the task for the agent");

    facade
        .git_draft_commit_message(project)
        .await
        .expect("a draft");

    let subject = one_shot.subjects().pop().expect("one run");
    assert!(
        subject.contains("Honour the repository's own commit template"),
        "the tool was told what the work was for: {subject}",
    );
    assert!(
        subject.contains("Git consults a template only when it would open an editor."),
        "including what the task says to do: {subject}",
    );
}

#[tokio::test]
async fn with_no_task_held_the_change_is_described_on_its_own() {
    // Nobody is required to use the tracker, so this is the ordinary case rather than a degraded
    // one: a running agent with nothing locked leaves the draft exactly as it was before intent
    // existed at all.
    let one_shot = Arc::new(FakeAgentOneShot::answering(DRAFTED));
    let (facade, project, _todos, _dir) =
        facade_for_drafting_with_todos(vec![assist_tool()], one_shot.clone());
    drafting_with(&facade);
    started(&facade, agent_registration(project, "worker")).await;
    task(
        &facade,
        project,
        "Something nobody picked up",
        "not started",
    );

    facade
        .git_draft_commit_message(project)
        .await
        .expect("a draft");

    let subject = one_shot.subjects().pop().expect("one run");
    assert!(
        !subject.contains(INTENT_MARKER),
        "nothing claims to know why: {subject}",
    );
    assert!(
        !subject.contains("Something nobody picked up"),
        "a task nobody is working on is not the intent behind this change: {subject}",
    );
    assert!(subject.contains("+new"), "the change is still described");
}

#[tokio::test]
async fn with_several_tasks_held_none_of_them_is_guessed_at() {
    // The refusal to guess, and the load-bearing half of the rule. Naming one of two would attribute
    // a change to work it may have nothing to do with — and state it to a model as fact.
    let one_shot = Arc::new(FakeAgentOneShot::answering(DRAFTED));
    let (facade, project, todos, _dir) =
        facade_for_drafting_with_todos(vec![assist_tool()], one_shot.clone());
    drafting_with(&facade);
    let one = started(&facade, agent_registration(project, "one")).await;
    let other = started(&facade, agent_registration(project, "other")).await;
    let first = task(&facade, project, "Read a repository's status", "");
    let second = task(&facade, project, "Write a repository's history", "");
    todos.lock(project, first, one).expect("lock the first");
    todos.lock(project, second, other).expect("lock the second");

    facade
        .git_draft_commit_message(project)
        .await
        .expect("a draft");

    let subject = one_shot.subjects().pop().expect("one run");
    assert!(
        !subject.contains(INTENT_MARKER),
        "with two in flight there is no answer, so none is invented: {subject}",
    );
    for guess in ["Read a repository's status", "Write a repository's history"] {
        assert!(
            !subject.contains(guess),
            "{guess:?} was guessed at: {subject}"
        );
    }
}

#[tokio::test]
async fn a_task_an_agent_left_behind_when_it_stopped_is_not_what_this_change_was_for() {
    // Why the question is asked of a lock rather than of a status somebody declared: a lock is
    // owned by a process, so it speaks for work in flight. One left behind speaks for nothing, and
    // a change made hours later must not be attributed to it.
    let one_shot = Arc::new(FakeAgentOneShot::answering(DRAFTED));
    let (facade, project, todos, _dir) =
        facade_for_drafting_with_todos(vec![assist_tool()], one_shot.clone());
    drafting_with(&facade);
    let agent = started(&facade, agent_registration(project, "worker")).await;
    let abandoned = task(&facade, project, "Something from this morning", "");
    todos.lock(project, abandoned, agent).expect("lock it");
    let mut events = facade.subscribe();
    assert!(
        facade.supervisor().stop(agent),
        "the agent was asked to stop"
    );
    timeout(
        SPAWN_WAIT,
        wait_all(&mut events, &[agent], ProcStatus::Stopped),
    )
    .await
    .expect("the agent stopped");

    facade
        .git_draft_commit_message(project)
        .await
        .expect("a draft");

    let subject = one_shot.subjects().pop().expect("one run");
    assert!(
        !subject.contains("Something from this morning"),
        "a task whose owner is gone is not the intent behind this change: {subject}",
    );
}

#[tokio::test]
async fn a_task_held_by_a_process_that_is_not_an_agent_is_not_what_this_change_was_for() {
    // A terminal carries the same process identity an agent does, so it can hold a lock. What it
    // cannot do is be the agent whose work a drafted message is about.
    let one_shot = Arc::new(FakeAgentOneShot::answering(DRAFTED));
    let (facade, project, todos, _dir) =
        facade_for_drafting_with_todos(vec![assist_tool()], one_shot.clone());
    drafting_with(&facade);
    started(&facade, agent_registration(project, "worker")).await;
    let shell = started(
        &facade,
        Registration::launched(
            project,
            ProcessKind::Terminal,
            "shell",
            crate::ports::SpawnSpec {
                command: "sh".into(),
                working_dir: ".".into(),
                env: Default::default(),
                size: Default::default(),
            },
        ),
    )
    .await;
    let held = task(&facade, project, "Something a shell picked up", "");
    todos.lock(project, held, shell).expect("lock it");

    facade
        .git_draft_commit_message(project)
        .await
        .expect("a draft");

    let subject = one_shot.subjects().pop().expect("one run");
    assert!(
        !subject.contains("Something a shell picked up"),
        "only a running agent's work is taken as the intent: {subject}",
    );
}
