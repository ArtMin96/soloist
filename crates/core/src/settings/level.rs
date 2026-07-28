//! How much a scope notifies: the three-level gate a project and a single command each carry.

use serde::{Deserialize, Serialize};

use crate::attention::Severity;

/// How much a scope notifies. Set per project and per command; the two combine to the more
/// restrictive of the pair ([`most_restrictive`](Self::most_restrictive)), so a command can quieten
/// itself below its project but never make itself louder than it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationLevel {
    /// Everything: terminal alerts as well as the important ones.
    All,
    /// Only what stops or blocks work — crashes, exhausted auto-restart, an agent waiting on the
    /// user. Terminal alerts are dropped.
    Important,
    /// Nothing at all.
    None,
}

impl NotificationLevel {
    /// Whether this level lets a signal of that severity through.
    pub fn admits(self, severity: Severity) -> bool {
        match self {
            Self::All => true,
            Self::Important => matches!(severity, Severity::Important),
            Self::None => false,
        }
    }

    /// The tighter of two levels. Combining is commutative, so which side holds the project and
    /// which the command override cannot change the answer.
    pub fn most_restrictive(self, other: Self) -> Self {
        match (self, other) {
            (Self::None, _) | (_, Self::None) => Self::None,
            (Self::Important, _) | (_, Self::Important) => Self::Important,
            (Self::All, Self::All) => Self::All,
        }
    }
}

#[cfg(test)]
#[path = "level_tests.rs"]
mod tests;
