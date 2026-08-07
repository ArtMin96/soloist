//! Behavioural tests for the façade's version-control read — the door every adapter comes
//! through. They assemble a real [`Facade`] over fakes, so what is asserted is what a Tauri
//! command, an HTTP route, or a tool would get back.

use std::sync::Arc;

use tempfile::TempDir;

use crate::agents::{AgentKind, AgentTool, OneShotError, PromptMode};
use crate::composition::CorePorts;
use crate::facade::Facade;
use crate::git::DiffExtent;
use crate::ids::ProjectId;
use crate::ports::{ProjectRepo, TokioClock};
use crate::settings::Assist;
use crate::testing::{
    file_change, git_status, raw_diff, FakeAgentOneShot, FakeAgentToolRepo, FakeGitRepository,
    FakeProjectRepo, FakeSettingsRepo, FakeSpawner, FakeTrustRepo,
};
use crate::vcs::{ChangeKind, DiffTarget, FileContent};

use super::{DraftMessageError, GitReadError};

const PATH: &str = "src/main.rs";

const HEADER: &str =
    "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n";

/// A façade over `repository` with one project open, returning it and the project's id.
fn facade_with_project(repository: Option<FakeGitRepository>) -> (Facade, ProjectId, TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let projects = Arc::new(FakeProjectRepo::new());
    let root = dir.path().canonicalize().expect("canonical root");
    let project = projects.upsert(&root, None, None).expect("add project").id;
    let mut ports = CorePorts::builder(
        Arc::new(FakeSpawner::exits_on_terminate()),
        Arc::new(TokioClock),
        Arc::new(FakeTrustRepo::new()),
        projects,
    );
    if let Some(repository) = repository {
        ports = ports.git_repository(Arc::new(repository));
    }
    (Facade::new(ports.build()), project, dir)
}

#[test]
fn an_open_project_reports_the_status_its_repository_holds() {
    let (facade, project, _dir) =
        facade_with_project(Some(FakeGitRepository::reporting(git_status("main"))));

    let status = facade
        .git_status(project)
        .expect("read")
        .expect("a repository");

    assert_eq!(status.branch.name.as_deref(), Some("main"));
}

#[test]
fn a_project_that_is_not_open_is_named_as_such_rather_than_read() {
    let repository = FakeGitRepository::reporting(git_status("main"));
    let (facade, _project, _dir) = facade_with_project(Some(repository.clone()));

    assert!(matches!(
        facade.git_status(ProjectId::from_raw(4_242)),
        Err(GitReadError::UnknownProject),
    ));
    assert_eq!(repository.reads(), 0, "an unknown project is never read");
}

#[test]
fn an_open_project_reports_the_diff_of_one_of_its_paths() {
    let mut status = git_status("main");
    status
        .changes
        .push(file_change(PATH, None, Some(ChangeKind::Modified)));
    let repository = FakeGitRepository::reporting(status)
        .diffing(raw_diff(HEADER, &["@@ -1,1 +1,1 @@\n-old\n+new\n"]));
    let (facade, project, _dir) = facade_with_project(Some(repository));

    let diff = facade
        .git_diff(project, PATH, DiffTarget::Unstaged, DiffExtent::Capped)
        .expect("read")
        .expect("a repository");

    assert_eq!(diff.path, PATH);
    assert!(diff.patch.contains("+new"));
}

#[test]
fn an_open_project_reports_the_contents_of_one_of_its_files() {
    let repository = FakeGitRepository::reporting(git_status("main")).holding(FileContent {
        text: Some("fn main() {}\n".to_string()),
        truncated: false,
    });
    let (facade, project, _dir) = facade_with_project(Some(repository));

    let content = facade
        .git_file(project, PATH)
        .expect("read")
        .expect("a file");

    assert_eq!(content.text.as_deref(), Some("fn main() {}\n"));
    assert!(!content.truncated);
}

#[test]
fn without_the_git_adapter_a_project_simply_has_no_version_control() {
    let (facade, project, _dir) = facade_with_project(None);

    assert_eq!(
        facade.git_status(project).expect("no error"),
        None,
        "the default port degrades silently, as every optional driven port does",
    );
}

const ASSIST_TOOL: &str = "My CLI";

const DRAFTED: &str = "Record the index";

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
) -> (Facade, ProjectId, TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let projects = Arc::new(FakeProjectRepo::new());
    let root = dir.path().canonicalize().expect("canonical root");
    let project = projects.upsert(&root, None, None).expect("add project").id;
    let mut status = git_status("main");
    status
        .changes
        .push(file_change(PATH, Some(ChangeKind::Modified), None));
    let ports = CorePorts::builder(
        Arc::new(FakeSpawner::exits_on_terminate()),
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
    .settings_repo(Arc::new(FakeSettingsRepo::new()));
    (Facade::new(ports.build()), project, dir)
}

#[test]
fn with_a_tool_selected_a_draft_comes_back_describing_what_is_staged() {
    let one_shot = Arc::new(FakeAgentOneShot::answering(DRAFTED));
    let (facade, project, _dir) = facade_for_drafting(vec![assist_tool()], one_shot.clone());
    facade
        .set_assist_settings(Assist {
            tool: Some(ASSIST_TOOL.to_string()),
        })
        .expect("select");

    let drafted = facade.git_draft_commit_message(project).expect("a draft");

    assert_eq!(drafted, DRAFTED);
    let subject = one_shot.subjects().pop().expect("one run");
    assert!(
        subject.contains("+new"),
        "the tool was given the staged change to describe: {subject}",
    );
}

#[test]
fn a_draft_records_nothing_and_commits_nothing() {
    // The whole safety property of the feature: what comes back is text for a person to read and
    // change. Nothing about asking for it touches the index or the history.
    let one_shot = Arc::new(FakeAgentOneShot::answering(DRAFTED));
    let (facade, project, _dir) = facade_for_drafting(vec![assist_tool()], one_shot);
    facade
        .set_assist_settings(Assist {
            tool: Some(ASSIST_TOOL.to_string()),
        })
        .expect("select");

    facade.git_draft_commit_message(project).expect("a draft");

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

#[test]
fn with_no_tool_selected_nothing_is_run_at_all() {
    // The opt-in half of the feature, asserted where it matters: not merely that the call is
    // refused, but that the refusal happens before an agent is reached.
    let one_shot = Arc::new(FakeAgentOneShot::answering(DRAFTED));
    let (facade, project, _dir) = facade_for_drafting(vec![assist_tool()], one_shot.clone());

    let refusal = facade.git_draft_commit_message(project).unwrap_err();

    assert!(
        matches!(refusal, DraftMessageError::NoAssistTool),
        "{refusal:?}",
    );
    assert!(one_shot.runs().is_empty());
}

#[test]
fn a_selected_tool_that_is_no_longer_configured_is_named_rather_than_guessed_at() {
    // A tool can be renamed or removed after it was picked. Falling back to another one would run
    // something the user never chose.
    let one_shot = Arc::new(FakeAgentOneShot::answering(DRAFTED));
    let (facade, project, _dir) = facade_for_drafting(vec![assist_tool()], one_shot.clone());
    facade
        .set_assist_settings(Assist {
            tool: Some("Some Other CLI".to_string()),
        })
        .expect("select");

    let refusal = facade.git_draft_commit_message(project).unwrap_err();

    assert!(
        matches!(refusal, DraftMessageError::UnknownTool),
        "{refusal:?}",
    );
    assert!(one_shot.runs().is_empty());
}

#[test]
fn a_tool_that_never_answers_surfaces_the_timeout_it_was_stopped_at() {
    let one_shot = Arc::new(FakeAgentOneShot::refusing(OneShotError::Timeout));
    let (facade, project, _dir) = facade_for_drafting(vec![assist_tool()], one_shot);
    facade
        .set_assist_settings(Assist {
            tool: Some(ASSIST_TOOL.to_string()),
        })
        .expect("select");

    let refusal = facade.git_draft_commit_message(project).unwrap_err();

    assert!(
        matches!(refusal, DraftMessageError::OneShot(OneShotError::Timeout)),
        "a surface can only say what went wrong if the reason survives the trip: {refusal:?}",
    );
}
