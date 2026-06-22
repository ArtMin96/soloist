use super::*;

#[test]
fn builtin_defaults_seed_the_known_providers_in_order() {
    let kinds: Vec<AgentKind> = AgentTool::builtin_defaults()
        .iter()
        .map(|tool| tool.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            AgentKind::Claude,
            AgentKind::Codex,
            AgentKind::Amp,
            AgentKind::Gemini,
            AgentKind::OpenCode,
            AgentKind::Copilot,
            AgentKind::Kimi,
        ],
        "the seeded built-ins are the known providers, in their canonical order"
    );
}

#[test]
fn every_builtin_command_is_a_bare_binary_with_no_seed_args() {
    // The command is run directly (and, for the auto-detected providers, probed with
    // `--version`), so a built-in's command must be the bare binary name with no arguments
    // baked in.
    for tool in AgentTool::builtin_defaults() {
        assert!(
            !tool.command.contains(char::is_whitespace),
            "{} command should be a bare binary, got {:?}",
            tool.name,
            tool.command
        );
        assert!(tool.default_args.is_empty(), "{} seeds no args", tool.name);
    }
}

#[test]
fn auto_detection_covers_exactly_the_documented_probe_set() {
    // Solo documents probing `--version` for these five providers.
    for kind in [
        AgentKind::Claude,
        AgentKind::Codex,
        AgentKind::Amp,
        AgentKind::Gemini,
        AgentKind::OpenCode,
    ] {
        assert!(
            kind.auto_detectable(),
            "{kind:?} is in the documented probe set"
        );
    }
    // Copilot and Kimi are built-in tool types but outside that set; Generic is
    // user-configured. None are auto-detected.
    for kind in [AgentKind::Copilot, AgentKind::Kimi, AgentKind::Generic] {
        assert!(!kind.auto_detectable(), "{kind:?} is not auto-detected");
    }
}

fn tool(command: &str, default_args: &[&str]) -> AgentTool {
    AgentTool {
        name: "Test".to_string(),
        command: command.to_string(),
        default_args: default_args.iter().map(|s| s.to_string()).collect(),
        kind: AgentKind::Generic,
        prompt_mode: PromptMode::AppendedArg,
    }
}

#[test]
fn launch_command_line_appends_default_then_extra_args_in_order() {
    let claude = tool("claude", &["--permission-mode", "plan"]);
    assert_eq!(
        claude.launch_command_line(&["--model".to_string(), "sonnet".to_string()]),
        "claude --permission-mode plan --model sonnet",
        "the command, then default args, then per-launch extra args, in that order"
    );
}

#[test]
fn launch_command_line_with_no_extra_args_is_command_and_defaults() {
    let claude = tool("claude", &["--resume"]);
    assert_eq!(claude.launch_command_line(&[]), "claude --resume");
}

#[test]
fn launch_command_line_quotes_an_argument_with_spaces_as_one_token() {
    // A flag value containing spaces must reach the agent as a single argument, not be
    // word-split by the login shell.
    let line = tool("claude", &[]).launch_command_line(&[
        "--append-system-prompt".to_string(),
        "be concise".to_string(),
    ]);
    assert_eq!(line, "claude --append-system-prompt 'be concise'");
}

#[test]
fn launch_command_line_escapes_embedded_single_quotes() {
    // The standard `'\''` form: close the quote, an escaped literal quote, reopen.
    let line = tool("claude", &[]).launch_command_line(&["it's".to_string()]);
    assert_eq!(line, r#"claude 'it'\''s'"#);
}
