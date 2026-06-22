//! Agent-tool domain types: the provider taxonomy, the prompt convention, and the tool
//! definition, plus the built-in providers Soloist seeds into the registry.

use serde::{Deserialize, Serialize};

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

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
