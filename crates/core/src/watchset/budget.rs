//! The app-wide watch budget: how many filesystem watches Soloist may hold, and how much of
//! that a project's share is once several are open.

/// The fraction of the system's watch limit Soloist may hold. Every other program on the
/// machine — an editor, a build tool, another dev-tools app — watches files too, so claiming
/// the whole limit for ourselves would starve them; half is generous to Soloist without being
/// greedy toward everything else running.
const BUDGET_FRACTION: usize = 2;

/// The assumed system watch limit when the OS will not say (`FileWatcher::capacity` returned
/// `None`). The Linux kernel's own default for `fs.inotify.max_user_watches`, so a host that
/// cannot be asked is treated as an unmodified one rather than as unbounded.
const ASSUMED_CAPACITY: usize = 8_192;

/// What Soloist may spend on filesystem watches, and how much of it is currently spent.
///
/// Pure bookkeeping — no I/O, no clock — so [`ProjectWatchSet`](super::ProjectWatchSet) can
/// recompute a project's share on every re-sync without touching the OS again.
pub(crate) struct Budget {
    total: usize,
    spent: usize,
}

impl Budget {
    /// A budget over the backend's reported capacity, or [`ASSUMED_CAPACITY`] when it would not
    /// say, holding [`BUDGET_FRACTION`] of it.
    pub(crate) fn new(capacity: Option<usize>) -> Self {
        Self {
            total: capacity.unwrap_or(ASSUMED_CAPACITY) / BUDGET_FRACTION,
            spent: 0,
        }
    }

    /// One project's even share of the whole budget, recomputed fresh from `open_projects` on
    /// every re-sync so opening a second project shrinks a first one's share rather than
    /// silently degrading it later. At least one project's worth, even when none is open yet.
    pub(crate) fn share(&self, open_projects: usize) -> usize {
        self.total / open_projects.max(1)
    }

    /// Records `watches` more watches as spent.
    pub(crate) fn spend(&mut self, watches: usize) {
        self.spent = self.spent.saturating_add(watches);
    }

    /// Returns `watches` watches to the budget — a directory pruned, a project closed.
    pub(crate) fn refund(&mut self, watches: usize) {
        self.spent = self.spent.saturating_sub(watches);
    }

    /// What is left to spend — what [`Registrations::register`](super::registry::Registrations)
    /// asks before it takes another watch out on the OS. Saturates at zero rather than
    /// underflowing if more was ever spent than the total holds, which a shrinking share (a new
    /// project opening) can momentarily cause until the next re-sync catches up.
    pub(crate) fn remaining(&self) -> usize {
        self.total.saturating_sub(self.spent)
    }
}

#[cfg(test)]
#[path = "budget_tests.rs"]
mod tests;
