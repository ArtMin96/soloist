//! The per-provider headless summarizer invocation (the Strategy pattern).
//!
//! When summarization is enabled, a compact rendered-text snapshot of an idle agent is sent to the
//! user's *own* configured summarizer CLI, run headless. Each provider exposes a different
//! non-interactive one-shot — a print flag, an `exec`/`run` subcommand, a model flag — so one
//! [`SummaryStrategy`] per provider, selected by [`summary_strategy_for`], owns the single
//! decision of how (or whether) that provider summarizes. Adding a provider is one arm here,
//! exactly as [`resume`](crate::agents) adds a resume invocation.
//!
//! Each supported invocation is grounded in that provider's own published CLI reference (cited per
//! `static`). A provider with no documented headless one-shot is [`NoSummary`] — honest rather
//! than a guessed flag — and simply yields no summary (graceful degradation).

use crate::agents::tool::shell_command_line;
use crate::agents::{AgentKind, AgentTool, PromptMode};

use super::runner::SummaryInvocation;

/// How one provider is invoked headless to produce a summary, or that it cannot.
pub(super) trait SummaryStrategy: Sync {
    /// The invocation that runs `tool` headless over `prompt`, selecting `model` when set — or
    /// `None` when the provider has no documented headless one-shot.
    fn invocation(
        &self,
        tool: &AgentTool,
        model: Option<&str>,
        prompt: &str,
    ) -> Option<SummaryInvocation>;
}

/// Providers whose headless one-shot takes the prompt as a single positional argument, after an
/// optional subcommand, an optional model flag (inserted only when a model is configured), and an
/// optional print flag. The tool's *launch* default args are deliberately not appended — a
/// headless summary is a fresh minimal call, not a re-run of the interactive launch flags.
struct ArgPrompt {
    /// Tokens inserted right after the command (a subcommand like `exec`/`run`, or none).
    subcommand: &'static [&'static str],
    /// The flag that selects the model, used only when a model is configured.
    model_flag: &'static str,
    /// Tokens placed immediately before the prompt argument (a print flag like `-p`, or none).
    print_flag: &'static [&'static str],
}

impl SummaryStrategy for ArgPrompt {
    fn invocation(
        &self,
        tool: &AgentTool,
        model: Option<&str>,
        prompt: &str,
    ) -> Option<SummaryInvocation> {
        let mut tokens = vec![tool.command.clone()];
        tokens.extend(self.subcommand.iter().map(|token| token.to_string()));
        if let Some(model) = model {
            tokens.push(self.model_flag.to_string());
            tokens.push(model.to_string());
        }
        tokens.extend(self.print_flag.iter().map(|token| token.to_string()));
        tokens.push(prompt.to_string());
        Some(SummaryInvocation {
            command_line: shell_command_line(tokens.iter().map(String::as_str)),
            stdin: None,
        })
    }
}

/// A user-configured [`AgentKind::Generic`] tool: no provider convention, so it follows the tool's
/// declared [`PromptMode`] — the prompt is appended as a final argument, or piped to stdin. The
/// user's own default args carry any model or flags, so none is inserted here.
struct GenericPrompt;

impl SummaryStrategy for GenericPrompt {
    fn invocation(
        &self,
        tool: &AgentTool,
        _model: Option<&str>,
        prompt: &str,
    ) -> Option<SummaryInvocation> {
        let mut tokens = vec![tool.command.clone()];
        tokens.extend(tool.default_args.iter().cloned());
        match tool.prompt_mode {
            PromptMode::AppendedArg => {
                tokens.push(prompt.to_string());
                Some(SummaryInvocation {
                    command_line: shell_command_line(tokens.iter().map(String::as_str)),
                    stdin: None,
                })
            }
            PromptMode::Stdin => Some(SummaryInvocation {
                command_line: shell_command_line(tokens.iter().map(String::as_str)),
                stdin: Some(prompt.to_string()),
            }),
        }
    }
}

/// A provider with no documented headless one-shot summary invocation. No summary is produced
/// rather than a fabricated flag; idle detection stays heuristic-only for it.
struct NoSummary;

impl SummaryStrategy for NoSummary {
    fn invocation(
        &self,
        _tool: &AgentTool,
        _model: Option<&str>,
        _prompt: &str,
    ) -> Option<SummaryInvocation> {
        None
    }
}

/// `claude -p "<prompt>"`: "Print response without interactive mode"; the prompt is a positional
/// argument and `--model <alias|name>` selects the model. — code.claude.com/docs/en/cli-reference
static CLAUDE: ArgPrompt = ArgPrompt {
    subcommand: &[],
    model_flag: "--model",
    print_flag: &["-p"],
};
/// `codex exec [-m <model>] "<prompt>"`: the non-interactive subcommand for scripted runs; the
/// prompt is a positional argument, the model set via `-m`/`--model`. —
/// developers.openai.com/codex/cli/reference
static CODEX: ArgPrompt = ArgPrompt {
    subcommand: &["exec"],
    model_flag: "-m",
    print_flag: &[],
};
/// `gemini -p "<prompt>" [-m <model>]`: non-interactive prompt via `-p`/`--prompt`, model via
/// `-m`/`--model`. — github.com/google-gemini/gemini-cli docs/cli
static GEMINI: ArgPrompt = ArgPrompt {
    subcommand: &[],
    model_flag: "-m",
    print_flag: &["-p"],
};
/// `opencode run [-m <provider/model>] "<message>"`: the non-interactive run subcommand; the
/// message is a positional argument. — opencode.ai/docs/cli
static OPENCODE: ArgPrompt = ArgPrompt {
    subcommand: &["run"],
    model_flag: "-m",
    print_flag: &[],
};
static GENERIC: GenericPrompt = GenericPrompt;
static NO_SUMMARY: NoSummary = NoSummary;

/// The headless summarizer invocation for a provider — the single place that knows how each one
/// summarizes. Amp resumes/summarizes only by a thread id Soloist does not capture, and Copilot
/// and Kimi document no id-less headless one-shot we can ground, so all three resolve to
/// [`NoSummary`]: no summary is produced for them rather than a guessed invocation.
pub(super) fn summary_strategy_for(kind: AgentKind) -> &'static dyn SummaryStrategy {
    use AgentKind::*;
    match kind {
        Claude => &CLAUDE,
        Codex => &CODEX,
        Gemini => &GEMINI,
        OpenCode => &OPENCODE,
        Generic => &GENERIC,
        Amp | Copilot | Kimi => &NO_SUMMARY,
    }
}

#[cfg(test)]
#[path = "strategy_tests.rs"]
mod tests;
