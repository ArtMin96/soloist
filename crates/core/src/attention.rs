//! Attention vocabulary: what a signal is asking of the user, and how loudly it asks.
//!
//! Shared kernel, not a context. The kind is what the notifications context (C7) classifies an
//! event into and what [`crate::events`] carries; the severity is what the per-project
//! [`NotificationLevel`](crate::settings::NotificationLevel) gates on. Like
//! [`crate::idle`]'s [`AgentActivity`](crate::idle::AgentActivity), it is owned by none of them and
//! depends on nothing. The logic that decides which event is which kind lives in `crate::notify`.

use serde::{Deserialize, Serialize};

/// How loudly a signal asks for the user. A closed pair, because it is the axis the notification
/// level cuts along: `Important` survives every level but `None`, `Terminal` only survives `All`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// A state the user has to come back to: work has stopped or is blocked until they act.
    Important,
    /// A signal the terminal raised. Worth surfacing, but the user is not blocking anything by
    /// missing it.
    Terminal,
}

/// The kinds of signal that warrant the user's attention, each carrying a fixed
/// [`Severity`](Self::severity). A closed enum, so adding a kind is one variant plus its arm in
/// each exhaustive match.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionKind {
    /// A process exited unexpectedly.
    Crashed,
    /// Auto-restart gave up after too many crashes.
    RestartExhausted,
    /// An agent is blocked waiting for the user to approve or answer.
    AgentPermission,
    /// An agent reported an error within its session.
    AgentError,
    /// A terminal rang the bell.
    TerminalBell,
    /// A process raised a notification of its own from its output, in its own words.
    TerminalNotification,
}

impl AttentionKind {
    /// How loudly this kind asks for the user. An agent waiting on a permission prompt is
    /// [`Important`](Severity::Important) alongside the crashes: it is a state a human must clear
    /// before anything proceeds, so it is the alert whose loss costs the most.
    pub fn severity(self) -> Severity {
        match self {
            Self::Crashed | Self::RestartExhausted | Self::AgentPermission | Self::AgentError => {
                Severity::Important
            }
            Self::TerminalBell | Self::TerminalNotification => Severity::Terminal,
        }
    }
}
