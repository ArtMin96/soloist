//! The per-provider initial-prompt invocation (the Strategy pattern).
//!
//! A worker Soloist spawns is handed its orchestration context as a first turn, and each provider
//! takes one differently — a trailing positional argument for those that take one at all. So one
//! [`InitialPromptStrategy`] per provider, selected by [`initial_prompt_strategy_for`], owns the
//! single decision of how, or whether, a launch can carry a first turn. Adding a future provider
//! is one arm there, exactly as [`resume`](super::resume) adds a resume invocation.
//!
//! Each supported invocation is grounded in that provider's own published reference (cited per
//! arm). A provider whose interactive prompt argument is not documented resolves to
//! [`NoInitialPrompt`] and launches exactly as it did before: an unverified argument is not a
//! degraded first turn but a CLI that rejects it and never starts, so silence is the safe answer.

use super::tool::{AgentTool, PromptMode};
use super::AgentKind;

/// How one provider receives a first-turn prompt at launch, or that it cannot. The launch path
/// asks [`initial_prompt_strategy_for`] for a tool's strategy and calls this once; the result, if
/// any, is the command line that starts the CLI with that prompt already submitted.
pub(super) trait InitialPromptStrategy: Sync {
    /// The command line that launches `tool` with `prompt` as its first turn, composed with the
    /// same `extra_args` as any other launch — or `None` when this provider has no documented way
    /// to receive one, in which case the plain launch command line is used unchanged.
    fn launch_command_line(
        &self,
        tool: &AgentTool,
        prompt: &str,
        extra_args: &[String],
    ) -> Option<String>;
}

/// Providers that take the prompt as a trailing positional argument and stay interactive with it
/// — the CLI opens on that first turn instead of an empty one. It goes last, after the tool's own
/// default and per-launch args, because those are flags the prompt must not be read as a value of.
struct PositionalPrompt;

impl InitialPromptStrategy for PositionalPrompt {
    fn launch_command_line(
        &self,
        tool: &AgentTool,
        prompt: &str,
        extra_args: &[String],
    ) -> Option<String> {
        Some(tool.command_line_with_trailing(extra_args, prompt))
    }
}

/// A [`AgentKind::Generic`] tool, which has no fixed command of its own: the user declares its
/// convention as the tool's [`PromptMode`]. [`PromptMode::AppendedArg`] takes the prompt as a
/// trailing argument, as the built-in providers that accept one do. [`PromptMode::Stdin`] takes it
/// on standard input, which a launch cannot supply — Soloist runs every agent interactively on a
/// PTY and pipes nothing into it — so such a tool has no launch-time hook for a first turn and is
/// launched without one.
struct ByPromptMode;

impl InitialPromptStrategy for ByPromptMode {
    fn launch_command_line(
        &self,
        tool: &AgentTool,
        prompt: &str,
        extra_args: &[String],
    ) -> Option<String> {
        match tool.prompt_mode {
            PromptMode::AppendedArg => Some(tool.command_line_with_trailing(extra_args, prompt)),
            PromptMode::Stdin => None,
        }
    }
}

/// A provider with no documented interactive prompt argument. Its launch carries no first turn
/// rather than a guessed flag that could stop the CLI from starting at all.
struct NoInitialPrompt;

impl InitialPromptStrategy for NoInitialPrompt {
    fn launch_command_line(
        &self,
        _tool: &AgentTool,
        _prompt: &str,
        _extra_args: &[String],
    ) -> Option<String> {
        None
    }
}

static POSITIONAL: PositionalPrompt = PositionalPrompt;
static BY_PROMPT_MODE: ByPromptMode = ByPromptMode;
static NO_INITIAL_PROMPT: NoInitialPrompt = NoInitialPrompt;

/// The initial-prompt invocation for a provider — the single place that knows how each one takes
/// a first turn at launch.
pub(super) fn initial_prompt_strategy_for(kind: AgentKind) -> &'static dyn InitialPromptStrategy {
    use AgentKind::*;
    match kind {
        // `claude "query"`: "Start interactive session with initial prompt" — a positional
        // argument, distinct from `-p`/`--print`, which runs headless and exits. —
        // code.claude.com/docs/en/cli-reference
        Claude => &POSITIONAL,
        // `codex "prompt"`: the bare command "Launch[es] the terminal UI. Accepts the global flags
        // above plus an optional prompt", omitted "to launch the TUI without a pre-filled
        // message"; `codex exec` is the non-interactive form. —
        // learn.chatgpt.com/docs/developer-commands
        Codex => &POSITIONAL,
        // A user-configured CLI, so its convention is the user's to declare.
        Generic => &BY_PROMPT_MODE,
        // No published interactive prompt argument for these, so none is invented. Amp
        // (`amp -x`) and OpenCode (`opencode run`) document prompt arguments only for their
        // non-interactive forms, which would exit instead of opening the session Soloist runs.
        Amp | Gemini | OpenCode | Copilot | Kimi => &NO_INITIAL_PROMPT,
    }
}

#[cfg(test)]
#[path = "initial_prompt_tests.rs"]
mod tests;
