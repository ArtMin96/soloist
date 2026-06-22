//! A conservative heuristic for spotting that an agent is blocked on a permission prompt.
//!
//! Isolated here because it is the fuzziest part of idle detection and the most likely to
//! need tuning: the exact prompt strings each agent CLI prints are **undocumented** (a
//! clean-room gap), so this matches only strong, model-agnostic approval-prompt idioms and
//! prefers a false negative to a false positive. A wrong `Permission` is worse than a missed
//! one: it would tell a fire-when-idle workflow the agent is busy when it is actually free,
//! or vice versa. This is our own approximation, not a copy of Solo's behaviour.

/// How many trailing non-empty lines to scan: a live prompt sits at the very tail, so a
/// small window avoids matching a cue buried in earlier scrollback.
const SCAN_LINES: usize = 3;

/// Strong, model-agnostic cues that a CLI is asking the user to approve or answer. Lowercase;
/// matched as substrings. Deliberately omits the bare word "permission" so an ordinary
/// "permission denied" error line is not mistaken for a prompt.
const CUES: &[&str] = &[
    "(y/n)",
    "[y/n]",
    "(yes/no)",
    "do you want to proceed",
    "do you want to continue",
    "do you want to allow",
    "allow this action",
    "grant access",
    "approve this",
];

/// Whether the rendered `tail` looks like the agent is blocked on a permission/approval
/// prompt. Scans the last few non-empty lines for an approval cue.
pub(super) fn looks_like_permission_prompt(tail: &[String]) -> bool {
    tail.iter()
        .rev()
        .filter(|line| !line.trim().is_empty())
        .take(SCAN_LINES)
        .any(|line| {
            let lower = line.to_ascii_lowercase();
            CUES.iter().any(|cue| lower.contains(cue))
        })
}

#[cfg(test)]
#[path = "permission_tests.rs"]
mod tests;
