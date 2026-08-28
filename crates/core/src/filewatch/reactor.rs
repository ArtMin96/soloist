//! The file-watch restart policy: a [`Clock`]-driven reactor that turns watched-file changes
//! into debounced restarts.
//!
//! The reactor consumes changed paths from [`crate::watchset::ProjectWatchSet`]'s fan-out —
//! itself the one owner of every OS watch registration (monitoring C5) — matches each against
//! the watch-eligible commands' globs (the pure [`super::policy`], with the default ignores),
//! coalesces a burst into a single restart with the shared [`crate::debounce::Debouncer`], and
//! routes the restart through the supervisor's existing [`Supervisor::file_restart`] — so
//! file-watch reuses one restart behaviour (the trust gate and the crash-tracking reset) rather
//! than reimplementing it. It rebuilds its match rules at startup and on each
//! [`DomainEvent::ProjectOpened`], [`DomainEvent::ProjectRemoved`], and
//! [`DomainEvent::ConfigChanged`], so a project opened after launch is matched too and a
//! `solo.yml` reload that re-globs or adds a command takes effect without a re-open. It holds a
//! [`Weak`] reference to the supervisor and ends when the event bus closes (app shutdown), like
//! the crash reactor; command-only, trusted-only, and running-only all follow from the watch
//! targets and the restart gate.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::debounce::{sleep_until, Debouncer};
use crate::events::{DomainEvent, EventBus};
use crate::ids::ProcessId;
use crate::ports::Clock;
use crate::supervisor::Supervisor;

use super::policy::{compile, WatchRule};

/// The quiet window a burst of changes is coalesced into before a restart fires. Long enough
/// to absorb an editor writing several files for one save, short enough to feel immediate.
const QUIET: Duration = Duration::from_millis(300);

/// Turns watched-file changes into debounced command restarts. Built once by the composition
/// root (via [`crate::facade::Facade::file_watch_loop`]) and spawned on the runtime.
pub struct WatchReactor {
    clock: Arc<dyn Clock>,
    events: broadcast::Receiver<DomainEvent>,
    changes: broadcast::Receiver<PathBuf>,
    supervisor: Weak<Supervisor>,
}

impl WatchReactor {
    /// Builds a reactor over the clock, watching the given supervisor weakly (so it never keeps
    /// the app alive), subscribing to the bus for the shutdown signal, and consuming changed
    /// paths from the watch set's fan-out.
    pub(crate) fn new(
        clock: Arc<dyn Clock>,
        changes: broadcast::Receiver<PathBuf>,
        bus: &EventBus,
        supervisor: Weak<Supervisor>,
    ) -> Self {
        Self {
            clock,
            events: bus.subscribe(),
            changes,
            supervisor,
        }
    }

    /// Runs the reactor until the bus closes (app shutdown) or the supervisor is dropped.
    /// Rebuilds the match rules at startup and whenever the watch-eligible command set may have
    /// changed, then debounces matching changes into restarts. Returned as a future for the
    /// composition root to spawn once.
    pub async fn run(mut self) {
        let Some(supervisor) = self.supervisor.upgrade() else {
            return;
        };
        let mut rules: Vec<WatchRule> = Vec::new();
        self.resync(&supervisor, &mut rules);
        drop(supervisor);

        let mut debouncers: HashMap<ProcessId, Debouncer> = HashMap::new();
        loop {
            let next_due = debouncers.values().filter_map(Debouncer::due_at).min();
            tokio::select! {
                // The event bus drives one thing: a closed bus means the facade dropped, so
                // stop; a project opening or being removed, a `solo.yml` reload (which can add,
                // remove, or re-glob a watch-eligible command), or a lag that may have hidden
                // any of them means the watch-eligible command set or its globs changed, so
                // rebuild the match rules. Changes themselves arrive on `self.changes`, not here.
                result = self.events.recv() => {
                    match result {
                        Err(RecvError::Closed) => break,
                        Ok(DomainEvent::ProjectOpened { .. }
                            | DomainEvent::ProjectRemoved { .. }
                            | DomainEvent::ConfigChanged { .. })
                        | Err(RecvError::Lagged(_)) => {
                            let Some(supervisor) = self.supervisor.upgrade() else {
                                break;
                            };
                            self.resync(&supervisor, &mut rules);
                        }
                        Ok(_) => {}
                    }
                }
                // A changed path from the watch set: arm the debounce for every command whose
                // globs it matches. A lagged receiver is left alone rather than armed wholesale —
                // a missed restart is a missed convenience, where arming every rule would restart
                // running dev servers that never actually changed.
                changed = self.changes.recv() => {
                    match changed {
                        Ok(path) => {
                            let now = self.clock.now();
                            for id in rules.iter().filter(|rule| rule.matches(&path)).map(|rule| rule.id) {
                                debouncers
                                    .entry(id)
                                    .or_insert_with(|| Debouncer::new(QUIET))
                                    .trigger(now);
                            }
                        }
                        Err(RecvError::Lagged(_)) => {}
                        Err(RecvError::Closed) => break,
                    }
                }
                // The quiet window elapsed for at least one command: restart the due ones.
                () = sleep_until(&self.clock, next_due) => {
                    let now = self.clock.now();
                    let Some(supervisor) = self.supervisor.upgrade() else {
                        break;
                    };
                    let mut due: Vec<ProcessId> = Vec::new();
                    debouncers.retain(|&id, debouncer| {
                        if debouncer.take_if_due(now) {
                            due.push(id);
                        }
                        debouncer.due_at().is_some()
                    });
                    for id in due {
                        supervisor.file_restart(id);
                    }
                }
            }
        }
    }

    /// Rebuilds the match rules from the current watch-eligible commands, wholesale, so a
    /// command that is gone simply drops out of matching. Which directories are actually
    /// watched, and what a refusal or a degradation means, is the watch set's concern
    /// entirely — this reactor only decides which changed paths, once reported, are worth a
    /// restart.
    fn resync(&self, supervisor: &Supervisor, rules: &mut Vec<WatchRule>) {
        rules.clear();
        for target in supervisor.watch_targets() {
            let Some(set) = compile(&target.globs) else {
                continue;
            };
            rules.push(WatchRule::new(target.id, target.project_root, set));
        }
    }
}

#[cfg(test)]
#[path = "reactor_tests.rs"]
mod tests;
