//! What a drafting run gives back, over the shared [`FakeAgentOneShot`] rather than a real agent.
//! What is asserted is what a caller receives: the text, or the refusal that leaves it with none.

use std::path::Path;
use std::sync::Arc;

use crate::agents::{
    AgentKind, AgentOneShot, AgentTool, Agents, NoopAgentOneShot, NoopVersionProbe, OneShotError,
    PromptMode,
};
use crate::testing::{FakeAgentOneShot, FakeAgentToolRepo, MockClock};

/// The fake ignores both — a run is addressed by neither here.
const WORKING_DIR: &str = "/project";
const PROMPT: &str = "Describe this change.";

fn generic_tool() -> AgentTool {
    AgentTool {
        name: "My CLI".to_string(),
        command: "mycli".to_string(),
        default_args: Vec::new(),
        kind: AgentKind::Generic,
        prompt_mode: PromptMode::Stdin,
    }
}

fn agents_over(one_shot: Arc<dyn AgentOneShot>) -> Agents {
    Agents::new(
        Arc::new(FakeAgentToolRepo::new(vec![generic_tool()])),
        Arc::new(NoopVersionProbe),
        one_shot,
        Arc::new(MockClock::new()),
    )
}

fn draft(one_shot: Arc<dyn AgentOneShot>) -> Result<String, OneShotError> {
    agents_over(one_shot).draft(&generic_tool(), Path::new(WORKING_DIR), PROMPT)
}

#[test]
fn what_a_tool_wrote_comes_back_without_the_blank_space_around_it() {
    // An agent CLI ends its answer with a newline, and often opens with one. A commit message box
    // is about to receive this, so the surrounding blank space is not part of the answer.
    let drafted = draft(Arc::new(FakeAgentOneShot::answering(
        "\n  Record the index\n\n",
    )))
    .expect("drafted");

    assert_eq!(drafted, "Record the index");
}

#[test]
fn a_tool_that_answers_with_nothing_is_a_refusal_rather_than_an_empty_draft() {
    // A caller asked for text. Handing back an empty string would put an empty draft in front of
    // the user as though it were an answer, which is worse than saying there was none.
    let refusal = draft(Arc::new(FakeAgentOneShot::answering(" \n\t "))).unwrap_err();

    assert_eq!(refusal, OneShotError::Empty);
}

#[test]
fn a_core_without_the_adapter_reports_the_tool_as_absent() {
    // The optional-subsystem default: nothing is run, and the refusal says why rather than
    // pretending a draft was made.
    let refusal = draft(Arc::new(NoopAgentOneShot)).unwrap_err();

    assert_eq!(refusal, OneShotError::Missing);
}
