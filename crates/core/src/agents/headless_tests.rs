//! What each provider is actually asked, and which providers cannot be asked at all. The
//! invocation is the whole observable output of a strategy, so that is what is asserted.

use super::*;

const PROMPT: &str = "Describe this change.";

fn tool(kind: AgentKind) -> AgentTool {
    AgentTool {
        name: "Tool".to_string(),
        command: "tool".to_string(),
        default_args: Vec::new(),
        kind,
        prompt_mode: PromptMode::AppendedArg,
    }
}

fn line(kind: AgentKind) -> Option<String> {
    tool(kind)
        .one_shot_invocation(PROMPT)
        .map(|invocation| invocation.command_line)
}

#[test]
fn each_supported_provider_is_asked_the_way_its_own_reference_documents() {
    // A provider run without its one-shot form opens its interactive interface instead, and a run
    // with no terminal attached never answers — so the exact token matters, per provider.
    assert_eq!(
        line(AgentKind::Claude).as_deref(),
        Some("tool --print 'Describe this change.'"),
    );
    assert_eq!(
        line(AgentKind::Codex).as_deref(),
        Some("tool exec 'Describe this change.'"),
    );
    assert_eq!(
        line(AgentKind::Amp).as_deref(),
        Some("tool --execute 'Describe this change.'"),
    );
    assert_eq!(
        line(AgentKind::Gemini).as_deref(),
        Some("tool --prompt 'Describe this change.'"),
    );
    assert_eq!(
        line(AgentKind::OpenCode).as_deref(),
        Some("tool run 'Describe this change.'"),
    );
    assert_eq!(
        line(AgentKind::Kimi).as_deref(),
        Some("tool --print --prompt 'Describe this change.'"),
    );
}

#[test]
fn a_provider_with_no_documented_one_shot_form_is_not_asked_at_all() {
    // Copilot's reference states no single-shot print-and-exit invocation. Guessing one would run
    // the interactive CLI with no terminal, which answers nothing until it is killed.
    assert_eq!(tool(AgentKind::Copilot).one_shot_invocation(PROMPT), None);
}

#[test]
fn the_prompt_comes_last_so_a_tools_own_flags_still_precede_it() {
    let mut configured = tool(AgentKind::Claude);
    configured.default_args = vec!["--model".to_string(), "opus".to_string()];

    let invocation = configured.one_shot_invocation(PROMPT).expect("an ask");

    assert_eq!(
        invocation.command_line,
        "tool --print --model opus 'Describe this change.'",
    );
}

#[test]
fn a_configured_tool_is_asked_the_way_it_says_it_takes_a_prompt() {
    let mut appended = tool(AgentKind::Generic);
    appended.default_args = vec!["--headless".to_string()];
    let appended = appended.one_shot_invocation(PROMPT).expect("an ask");
    assert_eq!(
        appended.command_line,
        "tool --headless 'Describe this change.'"
    );
    assert_eq!(appended.input, None);

    let mut piped = tool(AgentKind::Generic);
    piped.prompt_mode = PromptMode::Stdin;
    let piped = piped.one_shot_invocation(PROMPT).expect("an ask");
    assert_eq!(
        piped.command_line, "tool",
        "a tool that reads its prompt does not also get it in the line",
    );
    assert_eq!(piped.input.as_deref(), Some(PROMPT));
}
