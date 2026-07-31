use super::*;
use crate::agents::AgentTool;

/// A tool of `kind` whose command is the provider's conventional binary, with optional default
/// args and prompt convention — enough to exercise how each composes its opening turn.
fn tool(
    kind: AgentKind,
    command: &str,
    default_args: &[&str],
    prompt_mode: PromptMode,
) -> AgentTool {
    AgentTool {
        name: format!("{kind:?}"),
        command: command.to_string(),
        default_args: default_args.iter().map(|s| s.to_string()).collect(),
        kind,
        prompt_mode,
    }
}

#[test]
fn a_provider_with_a_documented_prompt_argument_opens_on_it() {
    for (kind, command, expected) in [
        (AgentKind::Claude, "claude", "claude 'do the thing'"),
        (AgentKind::Codex, "codex", "codex 'do the thing'"),
    ] {
        assert_eq!(
            tool(kind, command, &[], PromptMode::AppendedArg)
                .launch_command_line_with_prompt("do the thing", &[])
                .as_deref(),
            Some(expected),
        );
    }
}

#[test]
fn a_provider_without_a_documented_prompt_argument_takes_none() {
    // No invented flag: the launch falls back to the plain command line, which still starts.
    for (kind, command) in [
        (AgentKind::Gemini, "gemini"),
        (AgentKind::Amp, "amp"),
        (AgentKind::OpenCode, "opencode"),
        (AgentKind::Copilot, "copilot"),
        (AgentKind::Kimi, "kimi"),
    ] {
        assert_eq!(
            tool(kind, command, &[], PromptMode::AppendedArg)
                .launch_command_line_with_prompt("do the thing", &[]),
            None,
            "{kind:?} has no documented interactive prompt argument",
        );
    }
}

#[test]
fn the_prompt_goes_last_after_every_flag() {
    // Default args and per-launch flags come first, so the prompt is never consumed as the value
    // of the flag before it.
    assert_eq!(
        tool(
            AgentKind::Claude,
            "claude",
            &["--verbose"],
            PromptMode::AppendedArg
        )
        .launch_command_line_with_prompt("go", &["--model".to_string(), "opus".to_string()])
        .as_deref(),
        Some("claude --verbose --model opus go"),
    );
}

#[test]
fn a_multiline_prompt_survives_as_one_shell_argument() {
    // The preamble is several paragraphs with punctuation the shell would otherwise act on; it
    // has to reach the CLI as a single argument, not be word-split into flags.
    let line = tool(AgentKind::Claude, "claude", &[], PromptMode::AppendedArg)
        .launch_command_line_with_prompt("first line\nsecond $HOME `line`", &[])
        .expect("Claude takes a prompt");
    assert_eq!(line, "claude 'first line\nsecond $HOME `line`'");
}

#[test]
fn a_generic_tool_follows_the_convention_its_user_declared() {
    // A user-configured CLI has no convention of its own, so the tool's prompt mode decides:
    // an appended argument carries the turn, and stdin — which a PTY launch cannot supply —
    // carries none.
    assert_eq!(
        tool(AgentKind::Generic, "mycli", &[], PromptMode::AppendedArg)
            .launch_command_line_with_prompt("go", &[])
            .as_deref(),
        Some("mycli go"),
    );
    assert_eq!(
        tool(AgentKind::Generic, "mycli", &[], PromptMode::Stdin)
            .launch_command_line_with_prompt("go", &[]),
        None,
    );
}
