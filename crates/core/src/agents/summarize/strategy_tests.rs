use super::summary_strategy_for;
use crate::agents::{AgentKind, AgentTool, PromptMode};

/// A tool of `kind` named/commanded for the test, with no default args unless overridden.
fn tool(command: &str, kind: AgentKind) -> AgentTool {
    AgentTool {
        name: command.to_string(),
        command: command.to_string(),
        default_args: Vec::new(),
        kind,
        prompt_mode: PromptMode::AppendedArg,
    }
}

/// The composed command line for a supported provider (unwrapping the `Some`).
fn command_line(kind: AgentKind, command: &str, model: Option<&str>, prompt: &str) -> String {
    summary_strategy_for(kind)
        .invocation(&tool(command, kind), model, prompt)
        .expect("supported provider yields an invocation")
        .command_line
}

#[test]
fn claude_uses_print_flag_and_model_flag() {
    // `claude -p` is print mode; `--model` selects the model; the multi-word prompt is quoted.
    let line = command_line(
        AgentKind::Claude,
        "claude",
        Some("sonnet"),
        "summarize this",
    );
    assert_eq!(line, "claude --model sonnet -p 'summarize this'");
}

#[test]
fn claude_without_a_model_omits_the_model_flag() {
    let line = command_line(AgentKind::Claude, "claude", None, "hello");
    assert_eq!(line, "claude -p hello");
}

#[test]
fn codex_uses_the_exec_subcommand_and_short_model_flag() {
    let line = command_line(
        AgentKind::Codex,
        "codex",
        Some("gpt-5-codex"),
        "summarize this",
    );
    assert_eq!(line, "codex exec -m gpt-5-codex 'summarize this'");
}

#[test]
fn gemini_uses_the_prompt_flag() {
    let line = command_line(
        AgentKind::Gemini,
        "gemini",
        Some("flash-lite"),
        "summarize this",
    );
    assert_eq!(line, "gemini -m flash-lite -p 'summarize this'");
}

#[test]
fn opencode_uses_the_run_subcommand() {
    let line = command_line(
        AgentKind::OpenCode,
        "opencode",
        Some("anthropic/claude"),
        "summarize this",
    );
    assert_eq!(line, "opencode run -m anthropic/claude 'summarize this'");
}

#[test]
fn generic_appends_the_prompt_as_an_argument_after_default_args() {
    let mut generic = tool("mytool", AgentKind::Generic);
    generic.default_args = vec!["--quiet".to_string()];
    generic.prompt_mode = PromptMode::AppendedArg;
    let invocation = summary_strategy_for(AgentKind::Generic)
        .invocation(&generic, Some("ignored"), "summarize this")
        .expect("generic yields an invocation");
    // No model flag is inserted for a generic tool — the user's own default args carry any flags.
    assert_eq!(invocation.command_line, "mytool --quiet 'summarize this'");
    assert_eq!(invocation.stdin, None);
}

#[test]
fn generic_in_stdin_mode_pipes_the_prompt() {
    let mut generic = tool("mytool", AgentKind::Generic);
    generic.prompt_mode = PromptMode::Stdin;
    let invocation = summary_strategy_for(AgentKind::Generic)
        .invocation(&generic, None, "summarize this")
        .expect("generic yields an invocation");
    assert_eq!(invocation.command_line, "mytool");
    assert_eq!(invocation.stdin.as_deref(), Some("summarize this"));
}

#[test]
fn providers_without_a_headless_one_shot_yield_no_invocation() {
    for kind in [AgentKind::Amp, AgentKind::Copilot, AgentKind::Kimi] {
        let invocation = summary_strategy_for(kind).invocation(
            &tool("cli", kind),
            Some("model"),
            "summarize this",
        );
        assert!(
            invocation.is_none(),
            "{kind:?} has no documented headless one-shot and must yield None"
        );
    }
}
