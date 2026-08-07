//! What a drafting run gives back, over the shared [`FakeAgentOneShot`] rather than a real agent.
//! What is asserted is what a caller receives: the text, or the refusal that leaves it with none.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use crate::agents::test_support::drafting;
use crate::agents::{
    AgentKind, AgentOneShot, AgentTool, NoopAgentOneShot, OneShotError, PromptMode,
};
use crate::shellenv::{NoopShellEnvProbe, ShellEnvProbe};
use crate::testing::{FakeAgentOneShot, FakeShellEnvProbe};

/// The fake ignores both — a run is addressed by neither here.
const WORKING_DIR: &str = "/project";
const PROMPT: &str = "Describe this change.";

/// A directory an agent CLI is only ever on the user's interactive `PATH` from.
const VERSION_MANAGER_BIN: &str = "/home/dev/.nvm/versions/node/v20.0.0/bin";

fn generic_tool() -> AgentTool {
    AgentTool {
        name: "My CLI".to_string(),
        command: "mycli".to_string(),
        default_args: Vec::new(),
        kind: AgentKind::Generic,
        prompt_mode: PromptMode::Stdin,
    }
}

async fn draft(one_shot: Arc<dyn AgentOneShot>) -> Result<String, OneShotError> {
    draft_capturing(one_shot, Arc::new(NoopShellEnvProbe)).await
}

async fn draft_capturing(
    one_shot: Arc<dyn AgentOneShot>,
    shell_env_probe: Arc<dyn ShellEnvProbe>,
) -> Result<String, OneShotError> {
    drafting(vec![generic_tool()], one_shot, shell_env_probe)
        .draft(&generic_tool(), Path::new(WORKING_DIR), PROMPT)
        .await
}

#[tokio::test]
async fn what_a_tool_wrote_comes_back_without_the_blank_space_around_it() {
    // An agent CLI ends its answer with a newline, and often opens with one. A commit message box
    // is about to receive this, so the surrounding blank space is not part of the answer.
    let drafted = draft(Arc::new(FakeAgentOneShot::answering(
        "\n  Record the index\n\n",
    )))
    .await
    .expect("drafted");

    assert_eq!(drafted, "Record the index");
}

#[tokio::test]
async fn a_tool_that_answers_with_nothing_is_a_refusal_rather_than_an_empty_draft() {
    // A caller asked for text. Handing back an empty string would put an empty draft in front of
    // the user as though it were an answer, which is worse than saying there was none.
    let refusal = draft(Arc::new(FakeAgentOneShot::answering(" \n\t ")))
        .await
        .unwrap_err();

    assert_eq!(refusal, OneShotError::Empty);
}

#[tokio::test]
async fn a_core_without_the_adapter_reports_the_tool_as_absent() {
    // The optional-subsystem default: nothing is run, and the refusal says why rather than
    // pretending a draft was made.
    let refusal = draft(Arc::new(NoopAgentOneShot)).await.unwrap_err();

    assert_eq!(refusal, OneShotError::Missing);
}

#[tokio::test]
async fn a_run_is_made_in_the_environment_the_user_s_own_shell_would_have() {
    // Why the environment crosses the port at all: an agent CLI a version manager installed is on
    // no `PATH` the app itself inherited. Resolving it here is what lets the adapter run the tool
    // without starting a shell that would read the user's startup files over the answer.
    let one_shot = Arc::new(FakeAgentOneShot::answering("Record the index"));
    let captured = BTreeMap::from([("PATH".to_string(), VERSION_MANAGER_BIN.to_string())]);

    draft_capturing(
        one_shot.clone(),
        Arc::new(FakeShellEnvProbe::returning(captured.clone())),
    )
    .await
    .expect("drafted");

    assert_eq!(one_shot.environments(), vec![captured]);
}

#[tokio::test]
async fn a_shell_that_could_not_be_read_still_yields_a_draft() {
    // The capture is best-effort, as it is for a spawn: what it degrades to is a `PATH` holding the
    // usual places a user installs to, never a refusal — a draft the user asked for must not fail
    // because their shell was slow to start.
    let one_shot = Arc::new(FakeAgentOneShot::answering("Record the index"));

    let drafted = draft_capturing(one_shot.clone(), Arc::new(FakeShellEnvProbe::failing()))
        .await
        .expect("a draft even so");

    assert_eq!(drafted, "Record the index");
    let path = one_shot
        .environments()
        .pop()
        .expect("one run")
        .remove("PATH")
        .expect("a fallback search path");
    assert!(path.contains("/usr/local/bin"), "{path}");
}

#[tokio::test]
async fn with_no_capture_wired_a_run_is_made_in_the_app_s_own_environment() {
    // The optional-subsystem default, from the run's side: no overrides at all, so the tool
    // inherits exactly what the app has — the same degradation a spawn makes.
    let one_shot = Arc::new(FakeAgentOneShot::answering("Record the index"));

    draft_capturing(one_shot.clone(), Arc::new(NoopShellEnvProbe))
        .await
        .expect("drafted");

    assert_eq!(one_shot.environments(), vec![BTreeMap::new()]);
}
