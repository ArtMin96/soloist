//! The per-provider resume invocation (the Strategy pattern).
//!
//! "Resume last session" relaunches a stopped agent so its CLI reopens the most recent
//! conversation in the project directory. Each provider exposes that differently — a flag for
//! most, a subcommand for one, a thread id (which Soloist does not capture) for another — so
//! one [`ResumeStrategy`] per provider, selected by [`resume_strategy_for`], owns the single
//! decision of how, or whether, to resume. Adding a future provider is one arm there, exactly
//! as the idle [`strategy`](super::idle) adds a heuristic.
//!
//! Each supported invocation is grounded in that provider's own published reference (cited per
//! arm); a provider with no documented id-less resume is [`NoResume`] — honest rather than a
//! guessed flag.

use super::tool::AgentTool;
use super::AgentKind;

/// How one provider resumes its most recent session, or that it cannot. The launch path asks
/// [`resume_strategy_for`] for a tool's strategy and calls this once; the result, if any, is
/// the command line that reopens the last conversation.
pub(super) trait ResumeStrategy: Sync {
    /// The command line that relaunches `tool` resuming its most recent session in the working
    /// directory, composed with the same `extra_args` as the original launch — or `None` when
    /// the provider has no documented id-less resume.
    fn resume_command_line(&self, tool: &AgentTool, extra_args: &[String]) -> Option<String>;
}

/// Providers that resume by inserting a fixed token prefix right after the command — a flag
/// for most (`--continue`/`--resume`) or a subcommand for Codex (`resume --last`). The prefix
/// precedes the tool's own default and per-launch args, which every supported CLI accepts
/// after its resume verb or flag.
struct PrefixResume {
    prefix: &'static [&'static str],
}

impl ResumeStrategy for PrefixResume {
    fn resume_command_line(&self, tool: &AgentTool, extra_args: &[String]) -> Option<String> {
        Some(tool.command_line_with_prefix(self.prefix, extra_args))
    }
}

/// A provider with no documented id-less resume: Amp resumes only by an explicit thread id
/// (`amp threads continue <id>`) Soloist does not capture, and Generic is user-configured with
/// no fixed command. No resume affordance is offered for these rather than a fabricated flag.
struct NoResume;

impl ResumeStrategy for NoResume {
    fn resume_command_line(&self, _tool: &AgentTool, _extra_args: &[String]) -> Option<String> {
        None
    }
}

/// `--continue`: load the most recent conversation in the current directory, no session id.
/// Shared by every provider whose documented resume-last flag is `--continue`.
static CONTINUE: PrefixResume = PrefixResume {
    prefix: &["--continue"],
};
/// `--resume` with no argument: Gemini loads the most recent session.
static GEMINI_RESUME: PrefixResume = PrefixResume {
    prefix: &["--resume"],
};
/// `resume --last`: Codex's resume subcommand, scoped to the most recent session in the
/// current working directory.
static CODEX_RESUME: PrefixResume = PrefixResume {
    prefix: &["resume", "--last"],
};
static NO_RESUME: NoResume = NoResume;

/// The resume invocation for a provider — the single place that knows how each one resumes.
/// Every supported arm relaunches the *most recent conversation in the working directory*, the
/// faithful meaning of "Resume last session" without Soloist tracking a session id (the agent
/// process already pins its working directory to the project root). Unsupported providers
/// resolve to [`NoResume`], so no resume is offered.
pub(super) fn resume_strategy_for(kind: AgentKind) -> &'static dyn ResumeStrategy {
    use AgentKind::*;
    match kind {
        // `claude --continue` / `-c`: "Load the most recent conversation in the current
        // directory." — code.claude.com/docs/en/cli-reference
        Claude => &CONTINUE,
        // `codex resume --last`: skip the picker and resume the most recent conversation from
        // the current working directory; `resume` is a subcommand. —
        // developers.openai.com/codex/cli/reference
        Codex => &CODEX_RESUME,
        // `gemini --resume` / `-r` with no argument: "immediately loads the most recent
        // session." — github.com/google-gemini/gemini-cli docs/cli/session-management.md
        Gemini => &GEMINI_RESUME,
        // `opencode --continue` / `-c`: continue the last session. — opencode.ai/docs/cli
        OpenCode => &CONTINUE,
        // `copilot --continue`: resume the session from the current directory, else the most
        // recently active one. — docs.github.com/copilot
        Copilot => &CONTINUE,
        // `kimi --continue`: resume the most recent session for the current working directory.
        // — moonshotai.github.io/kimi-cli
        Kimi => &CONTINUE,
        // Amp resumes only by an explicit thread id (`amp threads continue <id>`, per
        // ampcode.com/manual) — which Soloist does not capture — and Generic is
        // user-configured, so neither has an id-less resume to offer.
        Amp | Generic => &NO_RESUME,
    }
}

#[cfg(test)]
#[path = "resume_tests.rs"]
mod tests;
