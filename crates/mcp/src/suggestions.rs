//! Contextual next-tool suggestions that decay.
//!
//! Some tool results carry a short "you just did X, consider Y next" hint — a nudge toward the
//! idiomatic follow-up (arm a timer instead of polling, claim a todo before working it). To avoid
//! spending tokens on the same hint forever, each suggestion **decays**: it is shown at most
//! [`SHOW_BUDGET`] times per session, then falls silent. One MCP connection is one session, so the
//! decay ledger lives with the server handler for the life of that connection.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

/// How many times one suggestion is shown to a session before it decays. Small on purpose: enough
/// to reinforce the idiom a couple of times, not enough to keep nagging a caller who knows it.
const SHOW_BUDGET: u32 = 2;

/// The contextual next-tool hint for a tool, or `None` when a tool has no suggestion. The single
/// source of which action points to which follow-up; keeping it one `match` means the nudges stay
/// consistent and easy to audit. Tools that share a hint share its decay budget (the hint text is
/// the ledger key).
fn hint_for(tool: &str) -> Option<&'static str> {
    Some(match tool {
        "spawn_agent" => {
            "You spawned a worker. Arm `timer_fire_when_idle_any` to be woken when it \
goes quiet, rather than polling its output in a loop."
        }
        "start_process" | "restart_process" => {
            "Use `wait_for_bound_port` to block until its port \
is up, or a fire-when-idle timer to be woken — don't poll the output waiting for it to be ready."
        }
        "send_input" => {
            "Read the effect with `get_process_output`; to wait for a prompt or for the \
process to go idle, set a timer instead of polling."
        }
        "lock_acquire" => {
            "Release it with `lock_release` when you are done — a lease also \
auto-releases when your process closes."
        }
        "todo_create" => {
            "Claim it with `todo_lock` before working it, so another agent doesn't \
duplicate the effort."
        }
        "scratchpad_write" => {
            "Others coordinate through this scratchpad: take a `lock_acquire` \
lease before editing shared state, and always write back the revision you read."
        }
        "register_agent" => {
            "Call `whoami` to confirm your scope; if more than one project is open \
you may need `select_project`."
        }
        _ => return None,
    })
}

/// A session's record of how many times each next-tool suggestion has been shown, so a suggestion
/// decays after [`SHOW_BUDGET`] appearances. Interior mutability lets the shared handler record a
/// show behind `&self`.
#[derive(Default)]
pub(crate) struct Suggestions {
    shown: Mutex<HashMap<&'static str, u32>>,
}

impl Suggestions {
    /// The next-tool hint to append to a `tool` result, or `None` when the tool has no suggestion
    /// or its suggestion has already decayed for this session. Taking the hint records the show, so
    /// each successful call advances the decay by one.
    pub(crate) fn take(&self, tool: &str) -> Option<&'static str> {
        let hint = hint_for(tool)?;
        let mut shown = self.shown.lock().unwrap_or_else(PoisonError::into_inner);
        let count = shown.entry(hint).or_insert(0);
        if *count >= SHOW_BUDGET {
            return None;
        }
        *count += 1;
        Some(hint)
    }
}

#[cfg(test)]
#[path = "suggestions_tests.rs"]
mod tests;
