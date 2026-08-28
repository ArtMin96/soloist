//! What the OS is currently limiting — refusing outright, or degrading — and the one place that
//! says so.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use crate::events::{DomainEvent, EventBus};
use crate::ids::ProjectId;
use crate::sync::lock;
use crate::watch::{WatchLimit, WatchOutcome, WatchPurpose};

/// A project's standing limits: what each [`WatchPurpose`] met, ordered so two equal sets compare
/// equal however they were arrived at.
type Limits = BTreeMap<WatchPurpose, WatchLimit>;

/// Which of each project's watches the OS is limiting, and the single voice that tells the
/// surfaces.
///
/// Both watch reactors record their answers here rather than announcing their own, for two
/// reasons. A project can be limited on one watch and granted the other, and only the aggregate
/// says which consequences actually follow — so it is the aggregate a surface renders, without
/// knowing which reactor asked. And both ask again for a limited root on every re-sync —
/// deliberately, so a limit that has since cleared is not permanent — which means announcing per
/// attempt would repeat one sentence for as long as the condition lasted. Announced on the
/// transition instead: limited, limited differently, or watched again in full.
pub(crate) struct WatchStatus {
    bus: EventBus,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// Each project's standing limits. An entry exists only while at least one purpose is
    /// limited for it.
    limited: HashMap<ProjectId, Limits>,
    /// What the surfaces were last told about each project, so a repeat says nothing.
    announced: HashMap<ProjectId, Limits>,
}

impl WatchStatus {
    pub(crate) fn new(bus: EventBus) -> Self {
        Self {
            bus,
            state: Mutex::new(State::default()),
        }
    }

    /// Replaces `purpose`'s answers with `outcomes` — one per project it has just tried to watch —
    /// and announces every project whose resulting set of limits differs from what the surfaces
    /// were last told.
    ///
    /// A project absent from `outcomes` is one this purpose no longer watches at all (its commands
    /// stopped declaring globs, or the project is gone), so its limit is withdrawn rather than
    /// left standing: nothing is being limited for a watch nobody is asking for.
    pub(crate) fn resynced(&self, purpose: WatchPurpose, outcomes: &[WatchOutcome]) {
        let announcements = {
            let mut state = lock(&self.state);
            let touched: Vec<ProjectId> = state
                .limited
                .iter()
                .filter(|(_, limits)| limits.contains_key(&purpose))
                .map(|(project, _)| *project)
                .chain(outcomes.iter().map(|outcome| outcome.project))
                .collect();
            state.limited.retain(|_, limits| {
                limits.remove(&purpose);
                !limits.is_empty()
            });
            for outcome in outcomes {
                if let Some(limit) = outcome.limit {
                    state
                        .limited
                        .entry(outcome.project)
                        .or_default()
                        .insert(purpose, limit);
                }
            }
            touched
                .into_iter()
                .filter_map(|project| state.settle(project).map(|limits| (project, limits)))
                .collect::<Vec<_>>()
        };
        for (project, limits) in announcements {
            self.bus
                .publish(DomainEvent::WatchLimitChanged { project, limits });
        }
    }
}

impl State {
    /// The announcement `project` now owes the surfaces, or `None` when they already hold its
    /// state. The set itself is the announcement: empty means every watch it had limited is
    /// established again in full.
    ///
    /// The whole set is compared rather than a limit picked out of it, so a project limited on two
    /// watches for two reasons settles on one answer instead of whichever the last look happened to
    /// reach — which would announce again on a condition that had not moved.
    fn settle(&mut self, project: ProjectId) -> Option<Limits> {
        let current = self.limited.get(&project).cloned().unwrap_or_default();
        let previous = if current.is_empty() {
            self.announced.remove(&project)
        } else {
            self.announced.insert(project, current.clone())
        };
        (previous.unwrap_or_default() != current).then_some(current)
    }
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
