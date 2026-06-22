//! The per-provider idle heuristics (the Strategy pattern).
//!
//! There is no universal idle signal — each agent CLI surfaces its state differently — so
//! each provider gets one rule selected by [`strategy_for`]. Every rule is a pure function
//! of `(memory, signals, current)` that updates `memory` in place, so the whole heuristic is
//! unit-testable with recorded sequences and no clock, PTY, or async.

use crate::agents::AgentKind;
use crate::terminal::TerminalActivity;

use super::activity::AgentActivity;
use super::permission::looks_like_permission_prompt;

/// Consecutive unchanged samples before an agent is treated as idle. At the sampler's ~1 Hz
/// cadence this is a few seconds of quiet — long enough not to flap mid-turn, short enough to
/// notice promptly. Solo's exact quiet window is undocumented (a gap); this is our default.
const IDLE_AFTER_QUIET_SAMPLES: u32 = 3;

/// Title substrings (lowercase) that map a title-status provider's title to an activity.
/// Generic and model-agnostic: the exact title vocabulary each provider uses is undocumented
/// (a clean-room gap), so this is our approximation rather than a copy of Solo's.
const THINKING_CUES: &[&str] = &["thinking", "reasoning", "planning"];
const WORKING_CUES: &[&str] = &["working", "running", "executing", "generating", "editing"];
const ERROR_CUES: &[&str] = &["error", "failed", "failure"];

/// The rolling per-agent state a strategy carries between samples — one shape shared by every
/// strategy, each using the fields its heuristic needs. Owned by the
/// [`Classifier`](super::classifier::Classifier).
#[derive(Default)]
pub(super) struct AgentMemory {
    /// The output byte count at the previous sample (the visible-output heuristic).
    last_output_seq: u64,
    /// The title at the previous sample (the title heuristics).
    last_title: Option<String>,
    /// Consecutive samples with no change in the watched signal — how quiet the agent has
    /// been, which tips it to [`AgentActivity::Idle`] past [`IDLE_AFTER_QUIET_SAMPLES`].
    quiet_samples: u32,
}

impl AgentMemory {
    /// Records a quiet sample and reports whether the agent has now been quiet long enough
    /// to be idle. Callers fall back to `current` until it has.
    fn note_quiet(&mut self) -> bool {
        self.quiet_samples = self.quiet_samples.saturating_add(1);
        self.quiet_samples >= IDLE_AFTER_QUIET_SAMPLES
    }
}

/// A provider's idle rule. `current` is the last activity reported for the agent (so a brief
/// pause holds the previous state rather than flapping); the result is this sample's activity.
pub(super) trait IdleStrategy: Sync {
    fn classify(
        &self,
        memory: &mut AgentMemory,
        signals: &TerminalActivity,
        current: AgentActivity,
    ) -> AgentActivity;
}

/// Visible-output heuristic (Claude, OpenCode, and the default for providers with no
/// documented heuristic): output flowing means working; once output settles, a blocking
/// prompt at the tail means permission, and continued quiet means idle.
struct OutputDelta;

impl IdleStrategy for OutputDelta {
    fn classify(
        &self,
        memory: &mut AgentMemory,
        signals: &TerminalActivity,
        current: AgentActivity,
    ) -> AgentActivity {
        let produced = signals.output_seq != memory.last_output_seq;
        memory.last_output_seq = signals.output_seq;
        if produced {
            // Output is flowing, so the agent is working — even if a just-printed or
            // just-answered permission prompt still lingers in the tail. A real block is
            // recognised only once output settles (below), so resumed work is never misread
            // as a stale permission prompt.
            memory.quiet_samples = 0;
            return AgentActivity::Working;
        }
        // Output has settled. A blocking prompt at the tail now means the agent is waiting
        // on the user — quiet but *not* done — so it is checked before idle and never read
        // as available. Reset the quiet count so idle must be re-earned after it.
        if looks_like_permission_prompt(&signals.tail) {
            memory.quiet_samples = 0;
            return AgentActivity::Permission;
        }
        if memory.note_quiet() {
            AgentActivity::Idle
        } else {
            // A brief pause mid-output: hold the previous state rather than flap to idle.
            current
        }
    }
}

/// OSC-title-stability heuristic (Codex, Amp): a title that keeps changing means active work;
/// a title that holds steady for a few samples means idle. Providers that never set a title
/// read as idle — the heuristic has nothing to watch, which is honest rather than wrong.
struct TitleStability;

impl IdleStrategy for TitleStability {
    fn classify(
        &self,
        memory: &mut AgentMemory,
        signals: &TerminalActivity,
        current: AgentActivity,
    ) -> AgentActivity {
        let changed = signals.title != memory.last_title;
        memory.last_title = signals.title.clone();
        if changed {
            memory.quiet_samples = 0;
            return AgentActivity::Working;
        }
        if memory.note_quiet() {
            AgentActivity::Idle
        } else {
            current
        }
    }
}

/// OSC-title-status heuristic (Gemini): the title text itself encodes status, so map its
/// keywords to an activity; with no recognised keyword, fall back to title-change activity
/// like [`TitleStability`].
struct TitleStatus;

impl IdleStrategy for TitleStatus {
    fn classify(
        &self,
        memory: &mut AgentMemory,
        signals: &TerminalActivity,
        current: AgentActivity,
    ) -> AgentActivity {
        let changed = signals.title != memory.last_title;
        memory.last_title = signals.title.clone();
        if let Some(title) = &signals.title {
            let lower = title.to_ascii_lowercase();
            let keyword = if ERROR_CUES.iter().any(|cue| lower.contains(cue)) {
                Some(AgentActivity::Error)
            } else if THINKING_CUES.iter().any(|cue| lower.contains(cue)) {
                Some(AgentActivity::Thinking)
            } else if WORKING_CUES.iter().any(|cue| lower.contains(cue)) {
                Some(AgentActivity::Working)
            } else {
                None
            };
            if let Some(activity) = keyword {
                memory.quiet_samples = 0;
                return activity;
            }
        }
        if changed {
            memory.quiet_samples = 0;
            return AgentActivity::Working;
        }
        if memory.note_quiet() {
            AgentActivity::Idle
        } else {
            current
        }
    }
}

static OUTPUT_DELTA: OutputDelta = OutputDelta;
static TITLE_STABILITY: TitleStability = TitleStability;
static TITLE_STATUS: TitleStatus = TitleStatus;

/// The idle heuristic for a provider. Claude/OpenCode read visible output, Codex/Amp read
/// OSC-title stability, and Gemini reads the OSC-title status text — the per-runtime
/// heuristics Solo documents. Copilot/Kimi/Generic have no documented heuristic, so they
/// default to the most universal signal, visible output.
pub(super) fn strategy_for(kind: AgentKind) -> &'static dyn IdleStrategy {
    use AgentKind::*;
    match kind {
        Claude | OpenCode | Copilot | Kimi | Generic => &OUTPUT_DELTA,
        Codex | Amp => &TITLE_STABILITY,
        Gemini => &TITLE_STATUS,
    }
}

#[cfg(test)]
#[path = "strategy_tests.rs"]
mod tests;
