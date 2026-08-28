//! What is physically held: one refcounted map from a watched path to the projects that want
//! it, and the app-wide [`Budget`] that map spends against.
//!
//! The two are one unit: every registration or release touches the map and the budget in
//! lockstep, and no watch is taken out on the OS that the budget has not first paid for.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::filewatch::WatchSession;
use crate::ids::ProjectId;
use crate::watch::WatchError;

use super::budget::Budget;

/// One watched path, and which open projects currently want it.
///
/// Refcounted because projects nest — a repository opened inside another project's tree shares
/// its files (`crate::git::routing` documents the same shape) — so releasing an inner project
/// must not unwatch a path an outer one still holds.
struct Registration {
    owners: HashSet<ProjectId>,
}

/// The paths this run currently holds, and the budget spending them against.
///
/// Per-run state: rebuilt from scratch on every [`super::ProjectWatchSet`] restart, alongside the
/// [`WatchSession`] it registers through. A restart is how the set recovers from a wedged
/// backend, so carrying either across one would leave it believing it holds watches that the
/// dropped session already released.
pub(super) struct Registrations {
    budget: Budget,
    held: HashMap<PathBuf, Registration>,
}

impl Registrations {
    pub(super) fn new(capacity: Option<usize>) -> Self {
        Self {
            budget: Budget::new(capacity),
            held: HashMap::new(),
        }
    }

    /// One project's even share of the whole budget — see [`Budget::share`].
    pub(super) fn share(&self, open_projects: usize) -> usize {
        self.budget.share(open_projects)
    }

    /// Every path `project` currently holds — counted for what is left of its share, and diffed
    /// against its plan on every re-sync so a path the plan no longer wants is released rather
    /// than left spending budget nothing asks for.
    pub(super) fn held_by(&self, project: ProjectId) -> impl Iterator<Item = &PathBuf> {
        self.held
            .iter()
            .filter(move |(_, registration)| registration.owners.contains(&project))
            .map(|(path, _)| path)
    }

    /// Whether `project` already holds `path`.
    pub(super) fn is_held(&self, path: &Path, project: ProjectId) -> bool {
        self.held
            .get(path)
            .is_some_and(|registration| registration.owners.contains(&project))
    }

    /// Every open project holding `path`, or `None` if nobody does — how [`super::set`] finds
    /// the owner(s) of the parent directory a change was reported under.
    pub(super) fn owners_of(&self, path: &Path) -> Option<&HashSet<ProjectId>> {
        self.held.get(path).map(|registration| &registration.owners)
    }

    /// Registers `path` for `project` as `tree` (a bounded subtree) or a single directory.
    ///
    /// A path another project already holds costs nothing new: `project` simply joins as an
    /// additional owner, with no second call to `session` and no second unit spent — the same
    /// physical OS watch already covers it. A path nobody holds yet costs a unit, so it is
    /// registered only if the budget still has one: an exhausted budget is refused here rather
    /// than asked of the OS, which shares its own limit with every other program on the machine.
    /// On refusal, from either, nothing changes and the error is returned for the caller to
    /// report.
    pub(super) fn register(
        &mut self,
        path: &Path,
        project: ProjectId,
        tree: bool,
        session: &dyn WatchSession,
    ) -> Result<(), WatchError> {
        if let Some(existing) = self.held.get_mut(path) {
            existing.owners.insert(project);
            return Ok(());
        }
        if self.budget.remaining() == 0 {
            return Err(WatchError::BudgetExhausted);
        }
        if tree {
            session.watch_tree(path)?;
        } else {
            session.watch_dir(path)?;
        }
        let mut owners = HashSet::new();
        owners.insert(project);
        self.held
            .insert(path.to_path_buf(), Registration { owners });
        self.budget.spend(1);
        Ok(())
    }

    /// Removes `project`'s ownership of `path`. Unwatches through `session` and refunds the
    /// budget only once no open project owns it any longer.
    pub(super) fn release(&mut self, path: &Path, project: ProjectId, session: &dyn WatchSession) {
        let Some(registration) = self.held.get_mut(path) else {
            return;
        };
        registration.owners.remove(&project);
        if registration.owners.is_empty() {
            self.held.remove(path);
            session.unwatch(path);
            self.budget.refund(1);
        }
    }

    /// Every path currently held by any project — the OS-facing set a test or a status read
    /// compares against.
    pub(super) fn paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.held.keys()
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
