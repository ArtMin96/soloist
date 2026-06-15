//! Process domain types: the kind taxonomy, the status state machine, and the
//! read-model projection adapters render.

use serde::{Deserialize, Serialize};

use crate::ids::{ProcessId, ProjectId};

/// The three process subtypes Soloist supervises. A closed enum so every consumer
/// must handle each case via an exhaustive `match`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ProcessKind {
    /// A named shell command (e.g. a dev server or worker).
    Command,
    /// An AI CLI running in an interactive terminal.
    Agent,
    /// A plain interactive shell.
    Terminal,
}

/// The lifecycle state of a managed process. Closed enum; transitions are only ever
/// made through [`ProcStatus::transition`], never by ad-hoc field assignment.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ProcStatus {
    Stopped,
    Starting,
    Running,
    Crashed,
    Restarting,
    Stopping,
    RestartExhausted,
}

/// Returned when a caller attempts a transition the state machine forbids.
#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
#[error("illegal process status transition: {from:?} -> {to:?}")]
pub struct IllegalTransition {
    pub from: ProcStatus,
    pub to: ProcStatus,
}

impl ProcStatus {
    /// Validates a transition, returning the new state or [`IllegalTransition`].
    ///
    /// The FSM is the contract: callers route every state change through here so an
    /// impossible edge (e.g. `Stopped -> Running` without `Starting`) is rejected
    /// rather than silently applied. Reaching [`ProcStatus::Crashed`] from any live
    /// state is allowed because a supervised panic or unexpected exit can occur at
    /// any point in the lifecycle.
    pub fn transition(self, to: ProcStatus) -> Result<ProcStatus, IllegalTransition> {
        use ProcStatus::*;
        let allowed = matches!(
            (self, to),
            (Stopped, Starting)
                | (Starting, Running)
                | (Starting, Stopping)
                | (Running, Stopping)
                | (Running, Stopped)
                | (Running, Restarting)
                | (Restarting, Starting)
                | (Stopping, Stopped)
                | (Crashed, Starting)
                | (RestartExhausted, Starting)
        ) || (matches!(self, Starting | Running | Stopping | Restarting)
            && to == Crashed);

        if allowed {
            Ok(to)
        } else {
            Err(IllegalTransition { from: self, to })
        }
    }
}

/// A cheap, cloneable snapshot of one process for adapters to render. Holds no
/// behaviour — the authoritative state lives in the owning actor and registry. The
/// `project` scopes it; `exit_code` is the most recent terminal exit code (`None`
/// while running, or when terminated by a signal).
#[derive(Clone, Debug, Serialize)]
pub struct ProcessView {
    pub id: ProcessId,
    pub project: ProjectId,
    pub kind: ProcessKind,
    pub label: String,
    pub status: ProcStatus,
    pub exit_code: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_transitions_are_accepted() {
        assert_eq!(
            ProcStatus::Stopped.transition(ProcStatus::Starting),
            Ok(ProcStatus::Starting)
        );
        assert_eq!(
            ProcStatus::Starting.transition(ProcStatus::Running),
            Ok(ProcStatus::Running)
        );
        assert_eq!(
            ProcStatus::Running.transition(ProcStatus::Stopping),
            Ok(ProcStatus::Stopping)
        );
        assert_eq!(
            ProcStatus::Stopping.transition(ProcStatus::Stopped),
            Ok(ProcStatus::Stopped)
        );
    }

    #[test]
    fn skipping_starting_is_illegal() {
        assert_eq!(
            ProcStatus::Stopped.transition(ProcStatus::Running),
            Err(IllegalTransition {
                from: ProcStatus::Stopped,
                to: ProcStatus::Running,
            })
        );
    }

    #[test]
    fn crashed_is_reachable_from_any_live_state() {
        for live in [
            ProcStatus::Starting,
            ProcStatus::Running,
            ProcStatus::Stopping,
            ProcStatus::Restarting,
        ] {
            assert_eq!(
                live.transition(ProcStatus::Crashed),
                Ok(ProcStatus::Crashed)
            );
        }
        // ...but not from a terminal/resting state.
        assert!(ProcStatus::Stopped.transition(ProcStatus::Crashed).is_err());
    }

    #[test]
    fn a_terminal_process_can_be_restarted() {
        // Starting a process again from any resting state is legal — a user can
        // restart a stopped, crashed, or restart-exhausted command.
        for resting in [
            ProcStatus::Stopped,
            ProcStatus::Crashed,
            ProcStatus::RestartExhausted,
        ] {
            assert_eq!(
                resting.transition(ProcStatus::Starting),
                Ok(ProcStatus::Starting),
                "{resting:?} should be restartable",
            );
        }
    }
}
