//! What the OS is currently refusing to watch, and the one place that says so.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use crate::events::{DomainEvent, EventBus};
use crate::ids::ProjectId;
use crate::sync::lock;
use crate::watch::{WatchError, WatchOutcome, WatchPurpose};

/// A project's standing refusals: the reason each [`WatchPurpose`] was turned down for, ordered so
/// two equal sets of refusals compare equal however they were arrived at.
type Refusals = BTreeMap<WatchPurpose, WatchError>;

/// Which of each project's watches the OS is refusing, and the single voice that tells the
/// surfaces.
///
/// Both watch reactors record their answers here rather than announcing their own, for two
/// reasons. A project can be refused one watch and granted the other, and only the aggregate says
/// which consequences actually follow — so it is the aggregate a surface renders, without knowing
/// which reactor asked. And both ask again for a refused root on every re-sync — deliberately, so
/// a refusal that has since cleared is not permanent — which means announcing per attempt would
/// repeat one sentence for as long as the condition lasted. Announced on the transition instead:
/// refused, refused differently, or watched again.
pub(crate) struct WatchStatus {
    bus: EventBus,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// Each project's standing refusals. An entry exists only while at least one purpose is
    /// refused for it.
    refused: HashMap<ProjectId, Refusals>,
    /// What the surfaces were last told about each project, so a repeat says nothing.
    announced: HashMap<ProjectId, Refusals>,
}

impl WatchStatus {
    pub(crate) fn new(bus: EventBus) -> Self {
        Self {
            bus,
            state: Mutex::new(State::default()),
        }
    }

    /// Replaces `purpose`'s answers with `outcomes` — one per project it has just tried to watch —
    /// and announces every project whose resulting set of refusals differs from what the surfaces
    /// were last told.
    ///
    /// A project absent from `outcomes` is one this purpose no longer watches at all (its commands
    /// stopped declaring globs, or the project is gone), so its refusal is withdrawn rather than
    /// left standing: nothing is being refused for a watch nobody is asking for.
    pub(crate) fn resynced(&self, purpose: WatchPurpose, outcomes: &[WatchOutcome]) {
        let announcements = {
            let mut state = lock(&self.state);
            let touched: Vec<ProjectId> = state
                .refused
                .iter()
                .filter(|(_, refusals)| refusals.contains_key(&purpose))
                .map(|(project, _)| *project)
                .chain(outcomes.iter().map(|outcome| outcome.project))
                .collect();
            state.refused.retain(|_, refusals| {
                refusals.remove(&purpose);
                !refusals.is_empty()
            });
            for outcome in outcomes {
                if let Some(reason) = outcome.refusal {
                    state
                        .refused
                        .entry(outcome.project)
                        .or_default()
                        .insert(purpose, reason);
                }
            }
            touched
                .into_iter()
                .filter_map(|project| state.settle(project).map(|refusals| (project, refusals)))
                .collect::<Vec<_>>()
        };
        for (project, refusals) in announcements {
            self.bus
                .publish(DomainEvent::WatchRefusalChanged { project, refusals });
        }
    }
}

impl State {
    /// The announcement `project` now owes the surfaces, or `None` when they already hold its
    /// state. The set itself is the announcement: empty means every watch it had refused is
    /// established again.
    ///
    /// The whole set is compared rather than a reason picked out of it, so a project refused two
    /// watches for two reasons settles on one answer instead of whichever the last look happened to
    /// reach — which would announce again on a condition that had not moved.
    fn settle(&mut self, project: ProjectId) -> Option<Refusals> {
        let current = self.refused.get(&project).cloned().unwrap_or_default();
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
