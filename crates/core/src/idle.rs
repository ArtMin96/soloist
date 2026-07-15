//! Agent idle vocabulary: the activity state an agent process is in.
//!
//! Shared kernel, not a context. The state is what the agents context (C4) derives from terminal
//! output, what coordination (C6) arms fire-when-idle timers on, what notifications (C7) raise
//! attention from, and what [`crate::events`] carries — so, like [`crate::process`]'s
//! [`ProcStatus`](crate::process::ProcStatus), it is owned by none of them and depends on nothing.
//! The heuristics that decide it live in `crate::agents::idle`.

use serde::{Deserialize, Serialize};

/// The five activity states an agent process can be in, derived from its terminal output
/// by a per-provider heuristic. A closed enum so every consumer handles each case explicitly.
///
/// This is the observable signal the coordination layer is built on: it answers two
/// questions a human is not around to answer — *is this agent busy or available?*
/// ([`Working`](Self::Working)/[`Thinking`](Self::Thinking) vs [`Idle`](Self::Idle)) and
/// *does it need a human?* ([`Permission`](Self::Permission)/[`Error`](Self::Error)). A
/// fire-when-idle timer must treat [`Permission`](Self::Permission) as *not* idle — the
/// agent is blocked waiting on the user, not done — which is why the state is richer than a
/// quiet/active boolean.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum AgentActivity {
    /// Quiet and finished its turn — available for new work.
    Idle,
    /// Blocked on a prompt that needs the user to approve or answer. An attention state:
    /// quiet, but *not* available — distinct from [`Idle`](Self::Idle).
    Permission,
    /// Generating a response but not yet producing visible work output. Only some
    /// providers expose this distinctly (e.g. via the terminal title); others fold it into
    /// [`Working`](Self::Working).
    Thinking,
    /// Actively producing output — running tools, editing, streaming a reply.
    Working,
    /// Reported an error within its session while still running. An attention state.
    Error,
}

impl AgentActivity {
    /// Whether this state warrants pulling the user back — a blocking prompt or an error.
    /// Drives the attention notification (see [`crate::notify`]).
    pub fn requires_attention(self) -> bool {
        matches!(self, AgentActivity::Permission | AgentActivity::Error)
    }
}
