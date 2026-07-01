//! Agent-tool domain types: the provider taxonomy, the prompt convention, and the tool
//! definition, plus the built-in providers Soloist seeds into the registry.

use serde::{Deserialize, Serialize};

use super::resume::resume_strategy_for;

/// The agent CLI providers Soloist knows out of the box, plus [`AgentKind::Generic`] for any
/// other CLI the user configures. A closed enum so auto-detection — and the per-provider idle
/// heuristics that build on it — handle every provider through an exhaustive `match` rather
/// than a stringly-typed comparison.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum AgentKind {
    Claude,
    Codex,
    Amp,
    Gemini,
    OpenCode,
    Copilot,
    Kimi,
    /// Any other agent CLI the user configures; has no fixed command, so it is not
    /// auto-detected and its prompt convention is set explicitly via [`PromptMode`].
    Generic,
}

impl AgentKind {
    /// Whether Soloist auto-detects this provider by probing its CLI's `--version`. True for
    /// the five providers whose `--version` probe is part of the auto-detect set (Claude,
    /// Codex, Amp, Gemini, OpenCode); false for Copilot and Kimi — built-in tool types that
    /// are configurable and launchable but outside that probe set — and for
    /// [`AgentKind::Generic`], which is user-configured with no fixed command to probe.
    pub fn auto_detectable(self) -> bool {
        use AgentKind::*;
        matches!(self, Claude | Codex | Amp | Gemini | OpenCode)
    }
}

/// How a [`AgentKind::Generic`] tool receives its prompt: piped to the process's stdin, or
/// appended as a command-line argument. Built-in providers follow their own conventions, so
/// this field is ignored for them.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum PromptMode {
    Stdin,
    AppendedArg,
}

/// A configured agent tool: a launchable CLI with the arguments appended on every launch and
/// its prompt convention. Built-in providers are seeded by [`AgentTool::builtin_defaults`];
/// the user can add or edit tools. `name` is the unique registry key and display label.
///
/// This is persisted as its own JSON (the store's `agent_tools.definition` column), so the
/// durable encoding is exactly this type. A field added in a later build must therefore carry
/// `#[serde(default)]` (or a migration must backfill it) so rows written by an earlier build
/// still deserialize.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentTool {
    /// The unique display name and registry key (e.g. `"Claude"`).
    pub name: String,
    /// The shell command that launches the CLI (e.g. `"claude"`); also what `--version`
    /// auto-detection probes.
    pub command: String,
    /// Arguments appended on every launch, before any per-launch extra flags.
    pub default_args: Vec<String>,
    /// Which provider's conventions this tool follows (drives auto-detection and, later, the
    /// idle heuristics).
    pub kind: AgentKind,
    /// How a generic tool receives its prompt; ignored for built-in providers.
    pub prompt_mode: PromptMode,
}

impl AgentTool {
    /// The shell command line that launches this tool with `extra_args` for one launch:
    /// the command, then the default args (appended on every launch), then the per-launch
    /// extra args, in that order. The supervisor runs the returned line verbatim through the
    /// login shell, so PATH and version managers still resolve.
    pub fn launch_command_line(&self, extra_args: &[String]) -> String {
        self.command_line_with_prefix(&[], extra_args)
    }

    /// The command line that relaunches this tool resuming its most recent session in the
    /// working directory, composed with the same `extra_args` as the original launch — or
    /// `None` when the provider has no documented id-less resume (Amp resumes only by a thread
    /// id Soloist does not capture; Generic is user-configured). The per-provider invocation
    /// is owned by [`resume_strategy_for`], the single place that knows how each provider
    /// resumes; this just composes it onto the tool's command and args.
    pub fn resume_command_line(&self, extra_args: &[String]) -> Option<String> {
        resume_strategy_for(self.kind).resume_command_line(self, extra_args)
    }

    /// Composes the tool's command line with `prefix` tokens inserted immediately after the
    /// command and before the args: command, prefix, default args (appended every launch),
    /// then per-launch extra args. Each token is POSIX-quoted so it survives `$SHELL -lc
    /// <line>` as exactly one argument — an arg with spaces or shell metacharacters stays a
    /// single argument rather than being word-split. The single source for how this tool's
    /// command line is built: a fresh launch passes an empty prefix, a resume passes the
    /// provider's resume verb or flag.
    pub(super) fn command_line_with_prefix(
        &self,
        prefix: &[&str],
        extra_args: &[String],
    ) -> String {
        shell_command_line(
            std::iter::once(self.command.as_str())
                .chain(prefix.iter().copied())
                .chain(self.default_args.iter().map(String::as_str))
                .chain(extra_args.iter().map(String::as_str)),
        )
    }

    /// The built-in agent providers Soloist seeds into the registry on first run. Each
    /// `command` is the provider's conventional CLI name, and default args are empty until
    /// the user adds flags. The first five are the providers Solo documents auto-detecting
    /// (their command is also the binary `--version` probes); Copilot and Kimi are additional
    /// built-in tool types — configurable and launchable, but outside the auto-detect set.
    pub fn builtin_defaults() -> Vec<AgentTool> {
        [
            ("Claude", "claude", AgentKind::Claude),
            ("Codex", "codex", AgentKind::Codex),
            ("Amp", "amp", AgentKind::Amp),
            ("Gemini", "gemini", AgentKind::Gemini),
            ("OpenCode", "opencode", AgentKind::OpenCode),
            ("Copilot", "copilot", AgentKind::Copilot),
            ("Kimi", "kimi", AgentKind::Kimi),
        ]
        .into_iter()
        .map(|(name, command, kind)| AgentTool {
            name: name.to_string(),
            command: command.to_string(),
            default_args: Vec::new(),
            kind,
            prompt_mode: PromptMode::AppendedArg,
        })
        .collect()
    }
}

/// Composes a POSIX shell command line from `tokens`: each is [`shell_quote`]d so it survives
/// `$SHELL -lc <line>` as exactly one argument, then joined by spaces. The single source for
/// turning a token list into a runnable command line — the launch path
/// ([`AgentTool::command_line_with_prefix`]) and the headless summarizer invocation
/// ([`crate::agents::summarize`]) both build their lines through it.
pub(crate) fn shell_command_line<'a>(tokens: impl Iterator<Item = &'a str>) -> String {
    tokens.map(shell_quote).collect::<Vec<_>>().join(" ")
}

/// Quotes one token for a POSIX shell so it is passed through `$SHELL -lc` as exactly one
/// argument. A token of only shell-safe characters is returned bare (readable command lines
/// for ordinary flags); anything else is wrapped in single quotes, with any embedded single
/// quote rendered as `'\''` (close quote, escaped quote, reopen) — the standard safe form.
fn shell_quote(token: &str) -> String {
    /// Characters safe to leave unquoted in a POSIX shell word.
    fn is_safe(ch: char) -> bool {
        ch.is_ascii_alphanumeric()
            || matches!(
                ch,
                '%' | '+' | ',' | '-' | '.' | '/' | ':' | '=' | '@' | '_'
            )
    }

    if !token.is_empty() && token.chars().all(is_safe) {
        return token.to_string();
    }
    let mut quoted = String::with_capacity(token.len() + 2);
    quoted.push('\'');
    for ch in token.chars() {
        if ch == '\'' {
            quoted.push_str("'\\''");
        } else {
            quoted.push(ch);
        }
    }
    quoted.push('\'');
    quoted
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
