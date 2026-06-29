use super::*;
use crate::agents::{AgentTool, PromptMode};

/// A tool of `kind` whose command is the provider's conventional binary, with optional
/// default args — enough to exercise how each provider composes its resume line.
fn tool(kind: AgentKind, command: &str, default_args: &[&str]) -> AgentTool {
    AgentTool {
        name: format!("{kind:?}"),
        command: command.to_string(),
        default_args: default_args.iter().map(|s| s.to_string()).collect(),
        kind,
        prompt_mode: PromptMode::AppendedArg,
    }
}

#[test]
fn each_supported_provider_resumes_the_most_recent_session() {
    // The documented resume-last invocation for every provider that has one: a flag for most,
    // a subcommand for Codex.
    let cases = [
        (AgentKind::Claude, "claude", "claude --continue"),
        (AgentKind::Codex, "codex", "codex resume --last"),
        (AgentKind::Gemini, "gemini", "gemini --resume"),
        (AgentKind::OpenCode, "opencode", "opencode --continue"),
        (AgentKind::Copilot, "copilot", "copilot --continue"),
        (AgentKind::Kimi, "kimi", "kimi --continue"),
    ];
    for (kind, command, expected) in cases {
        assert_eq!(
            tool(kind, command, &[]).resume_command_line(&[]).as_deref(),
            Some(expected),
            "{kind:?} resumes its most recent session"
        );
    }
}

#[test]
fn providers_without_a_documented_resume_offer_none() {
    // Amp resumes only by a thread id Soloist does not capture; Generic is user-configured.
    // Neither fabricates a flag — they offer no resume command.
    assert_eq!(
        tool(AgentKind::Amp, "amp", &[]).resume_command_line(&[]),
        None
    );
    assert_eq!(
        tool(AgentKind::Generic, "my-agent", &[]).resume_command_line(&[]),
        None
    );
}

#[test]
fn resume_inserts_its_prefix_before_the_default_and_extra_args() {
    // The resume verb/flag comes right after the command; the tool's own default args and the
    // per-launch extra args follow, in the same order a fresh launch uses them.
    let claude = tool(AgentKind::Claude, "claude", &["--permission-mode", "plan"]);
    assert_eq!(
        claude
            .resume_command_line(&["--model".to_string(), "sonnet".to_string()])
            .as_deref(),
        Some("claude --continue --permission-mode plan --model sonnet")
    );
}

#[test]
fn codex_resume_subcommand_precedes_the_args() {
    // Codex's resume is a subcommand, not a flag, but composes through the same path: the
    // `resume --last` tokens lead, then the tool's args.
    let codex = tool(AgentKind::Codex, "codex", &["--full-auto"]);
    assert_eq!(
        codex.resume_command_line(&[]).as_deref(),
        Some("codex resume --last --full-auto")
    );
}

#[test]
fn resume_quotes_arguments_like_a_fresh_launch() {
    // Resume reuses the launch composition, so an arg with spaces is still one shell token.
    let line = tool(AgentKind::Claude, "claude", &[]).resume_command_line(&[
        "--append-system-prompt".to_string(),
        "be concise".to_string(),
    ]);
    assert_eq!(
        line.as_deref(),
        Some("claude --continue --append-system-prompt 'be concise'")
    );
}
