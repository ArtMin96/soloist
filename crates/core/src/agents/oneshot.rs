//! Running a configured agent tool once, headless, for a piece of text.
//!
//! Every other way Soloist runs an agent attaches it to a pseudo-terminal and leaves it there: an
//! agent CLI is a terminal application, its session belongs to a process the user can see and type
//! into, and it is **never** started with a headless prompt flag. That invariant is not relaxed
//! here — this is a separate path end to end. Nothing is attached to a run, no session is bound to
//! it, none of its output reaches a terminal, and it either answers within a bounded time or is
//! stopped. Anything that wants an agent to *work* on something wants the interactive path; this
//! one asks a question and takes the answer.
//!
//! Optional, like every other driven subsystem: [`NoopAgentOneShot`] is the default, and nothing
//! reaches the port at all unless the user has picked a tool to serve it.

use std::path::Path;

use super::tool::AgentTool;
use super::Agents;

/// The most prompt one run may carry.
///
/// A prompt is composed to fit this rather than trimmed to it: whatever a caller wants described,
/// it decides what to leave out while it still knows what the parts mean. Generous enough for a
/// substantial change, bounded because handing an unbounded string to a subprocess is how one
/// pathological input becomes an out-of-memory.
pub const ONE_SHOT_PROMPT_LIMIT: usize = 48 * 1024;

/// The most of a run's answer that is carried back. A tool that will not stop writing is cut here
/// rather than followed, and what arrives is a piece of text a person is about to edit — not a
/// document.
pub const ONE_SHOT_REPLY_LIMIT: usize = 32 * 1024;

/// How one headless run is made, resolved in the core so the adapter decides nothing.
///
/// Which invocation a provider answers to, and whether the prompt travels in the line or on the
/// process's standard input, are both provider knowledge — so they are settled by
/// [`AgentTool::one_shot_invocation`](crate::agents::AgentTool::one_shot_invocation) before the port
/// is reached. What is left for an adapter is running the line and reading the answer.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OneShotInvocation {
    /// The command line to run, composed for this provider's one-shot form. Run verbatim through
    /// the user's login shell, so their `PATH` and version managers still resolve.
    pub command_line: String,
    /// The prompt, when the tool reads it from standard input rather than from the line. `None`
    /// means the line already carries it, and the run is given nothing to read.
    pub input: Option<String>,
}

impl OneShotInvocation {
    /// A run whose prompt is already part of `command_line`.
    pub fn in_line(command_line: String) -> Self {
        Self {
            command_line,
            input: None,
        }
    }

    /// A run that is written `prompt` on its standard input.
    pub fn on_input(command_line: String, prompt: &str) -> Self {
        Self {
            command_line,
            input: Some(prompt.to_string()),
        }
    }
}

/// Runs one configured agent tool headless, for text.
///
/// An implementation is **blocking**: it runs an external program, so callers reach it from the
/// blocking pool ([`crate::facade::Facade::blocking`]) rather than a runtime worker. It must return
/// within a bounded time — a run that cannot answer is [`OneShotError::Timeout`], never a wait
/// without end — must carry back at most [`ONE_SHOT_REPLY_LIMIT`] of what was written, and must
/// leave no process behind.
pub trait AgentOneShot: Send + Sync {
    /// Makes `invocation` in `working_dir`, and returns what it wrote to standard output.
    fn run(
        &self,
        invocation: &OneShotInvocation,
        working_dir: &Path,
    ) -> Result<String, OneShotError>;
}

/// Why a headless run produced no text.
///
/// Machine data only — an exit status at most. What the tool printed about itself does not cross
/// the port, so no behaviour here can come to depend on the wording of a program Soloist does not
/// own.
#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
pub enum OneShotError {
    /// Soloist knows no documented way to ask this provider a single question, so nothing was run.
    /// Produced by the context before the port is reached — such a tool is not offered for drafting
    /// in the first place, and this is what refuses one asked for anyway.
    #[error("Soloist cannot run that agent tool headless")]
    NoHeadlessForm,
    /// The tool's command is not installed, so there was nothing to run.
    #[error("that agent tool is not installed")]
    Missing,
    /// It did not answer within its time limit and was stopped.
    #[error("the agent tool did not answer within its time limit")]
    Timeout,
    /// It ran and failed. `status` is the exit status where it produced one.
    #[error("the agent tool failed")]
    Failed { status: Option<i32> },
    /// It ran, and had nothing to say. An answer of only blank space is this rather than a
    /// success, because a caller asked for text and there is none to hand on.
    #[error("the agent tool answered with nothing")]
    Empty,
}

/// An [`AgentOneShot`] that runs nothing — the default until the real adapter is wired (headless
/// tools, tests that do not exercise an agent).
///
/// Unlike the read-only driven ports it does not degrade silently, because there is nothing quieter
/// to degrade to: text that cannot be drafted has no stand-in. It reports the tool as absent, which
/// is precisely what a core without the adapter is. Nothing calls it unless the user picked a tool,
/// so a build without the adapter behaves as one where nobody asked for a draft.
#[derive(Clone, Copy, Default)]
pub struct NoopAgentOneShot;

impl AgentOneShot for NoopAgentOneShot {
    fn run(
        &self,
        _invocation: &OneShotInvocation,
        _working_dir: &Path,
    ) -> Result<String, OneShotError> {
        Err(OneShotError::Missing)
    }
}

impl Agents {
    /// Runs `tool` once in `working_dir` to draft text from `prompt`, and returns what it wrote,
    /// trimmed.
    ///
    /// Whatever comes back is a draft: it is handed to whoever asked for it to read and change,
    /// and nothing here or above acts on it. An answer of only blank space is [`OneShotError::Empty`]
    /// rather than an empty success, because a caller asked for text and there is none.
    ///
    /// Runs an external program, so callers reach it through
    /// [`Facade::blocking`](crate::facade::Facade::blocking) rather than a runtime worker.
    pub fn draft(
        &self,
        tool: &AgentTool,
        working_dir: &Path,
        prompt: &str,
    ) -> Result<String, OneShotError> {
        let invocation = tool
            .one_shot_invocation(prompt)
            .ok_or(OneShotError::NoHeadlessForm)?;
        let written = self.one_shot.run(&invocation, working_dir)?;
        let drafted = written.trim();
        if drafted.is_empty() {
            return Err(OneShotError::Empty);
        }
        Ok(drafted.to_string())
    }
}

#[cfg(test)]
#[path = "oneshot_tests.rs"]
mod tests;
