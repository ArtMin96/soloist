//! What the OS is currently refusing to watch, and the one place that says so.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::events::{DomainEvent, EventBus};
use crate::ids::ProjectId;
use crate::sync::lock;
use crate::watch::WatchError;

/// What a watch on a project's root was established for.
///
/// The two reactors register their own watches over the same tree, so the OS can grant one and
/// refuse the other. Keeping their answers apart is what stops one reactor's success from erasing
/// the other's refusal — and what lets a project be reported degraded while either is refused.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum WatchPurpose {
    /// The file-watch restart policy: a `restart_when_changed` command reloading on a save.
    Restarts,
    /// The git rail: a project's status re-read when its working tree or repository state moves.
    GitStatus,
}

/// Which projects the OS is refusing watches for, and the single voice that tells the surfaces.
///
/// Both watch reactors record their answers here rather than announcing their own, for two
/// reasons. A project is degraded if *either* watch is refused, so the aggregate is the only
/// answer a surface can render without knowing which reactor asked. And both ask again for a
/// refused root on every re-sync — deliberately, so a refusal that has since cleared is not
/// permanent — which means announcing per attempt would repeat one sentence for as long as the
/// condition lasted. Announced on the transition instead: refused, refused differently, or
/// watched again.
pub struct WatchStatus {
    bus: EventBus,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// Each purpose's standing refusal per project. An entry exists only while refused.
    refused: HashMap<(ProjectId, WatchPurpose), WatchError>,
    /// What the surfaces were last told about each project, so a repeat says nothing.
    announced: HashMap<ProjectId, WatchError>,
}

impl WatchStatus {
    pub fn new(bus: EventBus) -> Self {
        Self {
            bus,
            state: Mutex::new(State::default()),
        }
    }

    /// Replaces `purpose`'s answers with `outcomes` — one entry per project it has just tried to
    /// watch, carrying the refusal it met or `None` for a watch it holds — and announces every
    /// project whose resulting state differs from what the surfaces were last told.
    ///
    /// A project absent from `outcomes` is one this purpose no longer watches at all (its commands
    /// stopped declaring globs, or the project is gone), so its refusal is withdrawn rather than
    /// left standing: nothing is being refused for a watch nobody is asking for.
    pub fn resynced(&self, purpose: WatchPurpose, outcomes: &[(ProjectId, Option<WatchError>)]) {
        let announcements = {
            let mut state = lock(&self.state);
            let touched: Vec<ProjectId> = state
                .refused
                .keys()
                .filter(|(_, held)| *held == purpose)
                .map(|(project, _)| *project)
                .chain(outcomes.iter().map(|(project, _)| *project))
                .collect();
            state.refused.retain(|(_, held), _| *held != purpose);
            for (project, refusal) in outcomes {
                if let Some(reason) = refusal {
                    state.refused.insert((*project, purpose), *reason);
                }
            }
            touched
                .into_iter()
                .filter_map(|project| state.settle(project).map(|refusal| (project, refusal)))
                .collect::<Vec<_>>()
        };
        for (project, refusal) in announcements {
            self.bus
                .publish(DomainEvent::WatchRefusalChanged { project, refusal });
        }
    }
}

impl State {
    /// The announcement `project` now owes the surfaces, or `None` when they already hold its
    /// state. The inner `Option` is the state itself: `Some(reason)` refused, `None` watched
    /// again.
    fn settle(&mut self, project: ProjectId) -> Option<Option<WatchError>> {
        let current = self
            .refused
            .iter()
            .find(|((refused, _), _)| *refused == project)
            .map(|(_, reason)| *reason);
        let previous = match current {
            Some(reason) => self.announced.insert(project, reason),
            None => self.announced.remove(&project),
        };
        (previous != current).then_some(current)
    }
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
