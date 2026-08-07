//! The per-provider one-shot invocation (the Strategy pattern).
//!
//! Asking a provider one question and reading the answer off standard output is not the same
//! invocation as launching it: run without the right form, an agent CLI opens its interactive
//! interface instead, and a run with no terminal attached never answers at all. Each provider
//! spells the one-shot form differently — a print flag for most, a subcommand for two — so one
//! [`HeadlessStrategy`] per provider, selected by [`headless_strategy_for`], owns the single
//! decision of how, or whether, a provider is asked. Adding a future provider is one arm there,
//! exactly as [`resume`](super::resume) adds a resume invocation.
//!
//! Each supported invocation is grounded in that provider's own published reference (cited per
//! arm). A provider whose reference does not state a single-shot print-and-exit form has none, and
//! is not offered for drafting — honest rather than a guessed flag.

use super::oneshot::OneShotInvocation;
use super::tool::AgentTool;
use super::{AgentKind, PromptMode};

/// How one provider is asked a single question, or that it cannot be. The drafting path asks
/// [`headless_strategy_for`] for a tool's strategy and calls this once.
pub(super) trait HeadlessStrategy: Sync {
    /// The invocation that runs `tool` once for `prompt`, or `None` when the provider has no
    /// documented one-shot form.
    fn one_shot(&self, tool: &AgentTool, prompt: &str) -> Option<OneShotInvocation>;
}

/// Providers asked by inserting a fixed token right after the command — a print flag for most, a
/// subcommand for two — with the prompt as the last argument. It follows the tool's own default
/// args, which is where every supported CLI accepts its message.
struct PrefixedPrompt {
    prefix: &'static [&'static str],
}

impl HeadlessStrategy for PrefixedPrompt {
    fn one_shot(&self, tool: &AgentTool, prompt: &str) -> Option<OneShotInvocation> {
        Some(OneShotInvocation::in_line(tool.command_line_with_prefix(
            self.prefix,
            &[prompt.to_string()],
        )))
    }
}

/// A tool the user configured themselves, which follows no provider's convention. Its own
/// [`PromptMode`] says whether the prompt is appended to the line or written to standard input, and
/// its own default args carry whatever flag it needs to answer and exit — which is exactly what that
/// field is for.
struct ConfiguredPrompt;

impl HeadlessStrategy for ConfiguredPrompt {
    fn one_shot(&self, tool: &AgentTool, prompt: &str) -> Option<OneShotInvocation> {
        Some(match tool.prompt_mode {
            PromptMode::AppendedArg => {
                OneShotInvocation::in_line(tool.launch_command_line(&[prompt.to_string()]))
            }
            PromptMode::Stdin => OneShotInvocation::on_input(tool.launch_command_line(&[]), prompt),
        })
    }
}

/// A provider whose published reference does not state a single-shot print-and-exit invocation. It
/// is not offered for drafting, rather than run with a fabricated flag and left to hang.
struct NoOneShot;

impl HeadlessStrategy for NoOneShot {
    fn one_shot(&self, _tool: &AgentTool, _prompt: &str) -> Option<OneShotInvocation> {
        None
    }
}

/// `-p` / `--print`: the flag shared by every provider whose documented non-interactive form is to
/// print the response and exit.
static PRINT: PrefixedPrompt = PrefixedPrompt {
    prefix: &["--print"],
};
/// `-x` / `--execute`: Amp's one-shot form.
static AMP_EXECUTE: PrefixedPrompt = PrefixedPrompt {
    prefix: &["--execute"],
};
/// `exec`: Codex's non-interactive subcommand.
static CODEX_EXEC: PrefixedPrompt = PrefixedPrompt { prefix: &["exec"] };
/// `run`: opencode's non-interactive subcommand.
static OPENCODE_RUN: PrefixedPrompt = PrefixedPrompt { prefix: &["run"] };
/// `--print --prompt`: Kimi's print mode, given its prompt. Print mode is the non-interactive one;
/// the prompt flag is what carries the question into it.
static KIMI_PRINT: PrefixedPrompt = PrefixedPrompt {
    prefix: &["--print", "--prompt"],
};
/// `--prompt`: Gemini's headless flag. Spelled out rather than sharing [`PRINT`], because Gemini's
/// long form is `--prompt` where the others' is `--print` — one letter apart, and not a thing to
/// leave to a shared constant.
static GEMINI_PROMPT: PrefixedPrompt = PrefixedPrompt {
    prefix: &["--prompt"],
};
static CONFIGURED: ConfiguredPrompt = ConfiguredPrompt;
static NO_ONE_SHOT: NoOneShot = NoOneShot;

/// The one-shot invocation for a provider — the single place that knows how each one answers a
/// question and exits. Unsupported providers resolve to [`NoOneShot`], so no draft is offered for
/// them.
pub(super) fn headless_strategy_for(kind: AgentKind) -> &'static dyn HeadlessStrategy {
    use AgentKind::*;
    match kind {
        // `claude --print` / `-p`: "Print response without interactive mode."
        // — code.claude.com/docs/en/cli-reference
        Claude => &PRINT,
        // `codex exec <PROMPT>`: "Run Codex non-interactively… Stream results to stdout"; `exec` is
        // a subcommand. — learn.chatgpt.com/docs/developer-commands (Codex CLI reference)
        Codex => &CODEX_EXEC,
        // `amp --execute` / `-x`: "sends the message provided to `-x` to the agent, waits until the
        // agent ended its turn, prints its final message, and exits." — ampcode.com/manual
        Amp => &AMP_EXECUTE,
        // `gemini --prompt` / `-p`: "Headless mode is triggered … when providing a query with the
        // `-p` (or `--prompt`) flag." — github.com/google-gemini/gemini-cli docs/cli/headless.md
        Gemini => &GEMINI_PROMPT,
        // `opencode run <prompt>`: "Run opencode in non-interactive mode by passing a prompt
        // directly." — opencode.ai/docs/cli
        OpenCode => &OPENCODE_RUN,
        // `kimi --print --prompt <text>`: `--print` is "Run in print mode (non-interactive)" and
        // `--prompt`/`-p` "Pass user prompt, doesn't enter interactive mode".
        // — github.com/MoonshotAI/kimi-cli docs/en/reference/kimi-command.md
        Kimi => &KIMI_PRINT,
        // Copilot's published reference states no single-shot print-and-exit invocation — its docs
        // mention `-p` only in passing, as starting a "programmatic session", which is not the same
        // claim. No draft is offered for it until that form is documented.
        Copilot => &NO_ONE_SHOT,
        // User-configured, so its own prompt convention and args decide.
        Generic => &CONFIGURED,
    }
}

#[cfg(test)]
#[path = "headless_tests.rs"]
mod tests;
