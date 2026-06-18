//! Orphan reconciliation (part of context C2).
//!
//! On launch after a crash or force-quit, the runtime-state file may list process
//! groups that were running last time. Each is classified purely from two facts — is
//! its group still alive, and does it match a currently-registered command — into one
//! of three fates: **adopt** it back as a running process, **surface** it for a user
//! Kill/Leave decision, or **prune** its stale record. The classification is a pure
//! function so it is exhaustively testable; the supervisor performs the side effects.

use serde::Serialize;

use crate::ids::ProcessId;
use crate::ports::OrphanRecord;

/// A leftover process group surfaced to the user for a Kill / Kill All / Leave running
/// decision. The dialog lives in the UI; the core only reports the candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OrphanInfo {
    pub name: String,
    pub command: String,
    pub pgid: i32,
}

impl From<&OrphanRecord> for OrphanInfo {
    fn from(record: &OrphanRecord) -> Self {
        Self {
            name: record.name.clone(),
            command: record.command.clone(),
            pgid: record.pgid,
        }
    }
}

/// The outcome of reconciling the runtime-state file against live process groups.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OrphanReport {
    /// Live groups matched to a registered command and adopted as running.
    pub adopted: Vec<ProcessId>,
    /// Live groups with no match, surfaced for a user decision.
    pub surfaced: Vec<OrphanInfo>,
    /// Dead groups whose stale records were pruned.
    pub pruned: usize,
}

/// What to do with one recorded process group.
pub(crate) enum OrphanFate {
    /// Alive and matched to a resting registered command — re-attach it.
    Adopt {
        record: OrphanRecord,
        target: ProcessId,
    },
    /// Alive but unmatched — report it for a user decision.
    Surface(OrphanRecord),
    /// No longer alive — drop the stale record.
    Prune(OrphanRecord),
}

/// Classifies each recorded group: dead → prune; alive and matched → adopt; alive and
/// unmatched → surface. `matcher` returns the resting registered process a record
/// should re-attach to, if any.
pub(crate) fn classify(
    records: Vec<OrphanRecord>,
    is_alive: impl Fn(i32) -> bool,
    matcher: impl Fn(&OrphanRecord) -> Option<ProcessId>,
) -> Vec<OrphanFate> {
    records
        .into_iter()
        .map(|record| {
            if !is_alive(record.pgid) {
                OrphanFate::Prune(record)
            } else if let Some(target) = matcher(&record) {
                OrphanFate::Adopt { record, target }
            } else {
                OrphanFate::Surface(record)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn record(name: &str, pgid: i32) -> OrphanRecord {
        OrphanRecord {
            project_root: PathBuf::from("/p"),
            name: name.into(),
            command: format!("run {name}"),
            pgid,
        }
    }

    #[test]
    fn classifies_each_record_by_liveness_and_match() {
        let dead = record("dead", 1);
        let matched = record("web", 2);
        let unmatched = record("stray", 3);
        let target = ProcessId::from_raw(42);

        // Only pgid 1 is dead; only "web" matches a registered command.
        let fates = classify(
            vec![dead, matched, unmatched],
            |pgid| pgid != 1,
            |rec| (rec.name == "web").then_some(target),
        );

        assert!(matches!(fates[0], OrphanFate::Prune(_)));
        assert!(matches!(
            &fates[1],
            OrphanFate::Adopt { target: t, .. } if *t == target
        ));
        assert!(matches!(&fates[2], OrphanFate::Surface(rec) if rec.name == "stray"));
    }
}
