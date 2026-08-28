//! The `solo.yml` sync trigger: a [`Clock`]-driven reactor that turns external edits of an
//! open project's config file into debounced reloads.
//!
//! The reactor consumes changed paths from [`crate::watchset::ProjectWatchSet`]'s fan-out —
//! itself the one owner of every OS watch registration (monitoring C5) — matches each reported
//! path against the roots' `solo.yml` locations, coalesces an editor's save burst with the
//! shared [`crate::debounce::Debouncer`], and routes the reload through
//! [`ProjectService::reload`] — the same reconcile the HTTP `reload` endpoint drives, so an
//! external edit and an explicit reload are one behaviour. The sync engine underneath
//! hash-diffs the file (a byte-identical rewrite is a no-op), refreshes its hash on the
//! app's own writes (so a self-write never re-syncs), and announces
//! [`DomainEvent::ConfigChanged`] with the trust review the UI's dialog renders. It rebuilds
//! its `solo.yml`-path index on [`DomainEvent::ProjectOpened`] / [`DomainEvent::ProjectRemoved`],
//! so a project opened after launch is matched too. Like the file-watch reactor, it holds the
//! supervisor weakly and ends when the bus closes.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::config::{config_path, ConfigEngine};
use crate::debounce::{sleep_until, Debouncer};
use crate::events::{DomainEvent, EventBus};
use crate::ids::ProjectId;
use crate::ports::Clock;
use crate::supervisor::Supervisor;

use super::{ProjectService, Projects};

/// The quiet window a burst of config-file events is coalesced into before one reload.
/// Long enough to absorb an editor's save sequence (write + rename + metadata), short
/// enough that the trust review feels immediate.
const QUIET: Duration = Duration::from_millis(300);

/// Turns external `solo.yml` edits into debounced project reloads. Built once by the
/// composition root (via [`crate::facade::Facade::config_watch_loop`]) and spawned on the
/// runtime.
pub struct ConfigWatchReactor {
    clock: Arc<dyn Clock>,
    events: broadcast::Receiver<DomainEvent>,
    changes: broadcast::Receiver<PathBuf>,
    bus: EventBus,
    supervisor: Weak<Supervisor>,
    projects: Arc<Projects>,
    config: Arc<ConfigEngine>,
}

impl ConfigWatchReactor {
    /// Builds a reactor over the clock, sharing the contexts a reload spans, watching the
    /// supervisor weakly (so it never keeps the app alive), subscribing to the bus for project
    /// lifecycle and the shutdown signal, and consuming changed paths from the watch set's
    /// fan-out.
    pub fn new(
        clock: Arc<dyn Clock>,
        changes: broadcast::Receiver<PathBuf>,
        bus: &EventBus,
        supervisor: Weak<Supervisor>,
        projects: Arc<Projects>,
        config: Arc<ConfigEngine>,
    ) -> Self {
        Self {
            clock,
            events: bus.subscribe(),
            changes,
            bus: bus.clone(),
            supervisor,
            projects,
            config,
        }
    }

    /// Runs the reactor until the bus closes (app shutdown) or the supervisor is dropped.
    /// Rebuilds its `solo.yml`-path index at startup and whenever the open-project set may have
    /// changed, then debounces matching changes into reloads.
    pub async fn run(mut self) {
        // The config-file paths an open project's `solo.yml` can be reported at. `resync`
        // rebuilds it wholesale from the registry — once now, then again on each project open
        // or removal.
        let mut config_paths: HashMap<PathBuf, ProjectId> = HashMap::new();
        self.resync(&mut config_paths);

        let mut debouncers: HashMap<ProjectId, Debouncer> = HashMap::new();
        loop {
            let next_due = debouncers.values().filter_map(Debouncer::due_at).min();
            tokio::select! {
                // The event bus drives one thing: a closed bus means the facade dropped, so
                // stop; a project opening or being removed (or a lag that may have hidden
                // either) means the open-project set changed, so rebuild the config-path index.
                // Config-file changes themselves arrive on `self.changes`, not here.
                result = self.events.recv() => {
                    match result {
                        Err(RecvError::Closed) => break,
                        Ok(DomainEvent::ProjectOpened { .. }
                            | DomainEvent::ProjectRemoved { .. })
                        | Err(RecvError::Lagged(_)) => {
                            self.resync(&mut config_paths);
                        }
                        Ok(_) => {}
                    }
                }
                // A changed path: arm the debounce when it is an open project's `solo.yml`. A
                // lagged receiver arms every currently indexed project instead of doing
                // nothing — a reload is idempotent (the sync engine hash-diffs the file and
                // no-ops on a byte-identical read), where the alternative is an edit made
                // during the lag window going unnoticed until the file changes again.
                changed = self.changes.recv() => {
                    match changed {
                        Ok(path) => {
                            if let Some(&project) = config_paths.get(&path) {
                                debouncers
                                    .entry(project)
                                    .or_insert_with(|| Debouncer::new(QUIET))
                                    .trigger(self.clock.now());
                            }
                        }
                        Err(RecvError::Lagged(_)) => {
                            let now = self.clock.now();
                            for &project in config_paths.values() {
                                debouncers
                                    .entry(project)
                                    .or_insert_with(|| Debouncer::new(QUIET))
                                    .trigger(now);
                            }
                        }
                        Err(RecvError::Closed) => break,
                    }
                }
                // The quiet window elapsed for at least one project: reload the due ones.
                () = sleep_until(&self.clock, next_due) => {
                    let now = self.clock.now();
                    let Some(supervisor) = self.supervisor.upgrade() else {
                        break;
                    };
                    let mut due: Vec<ProjectId> = Vec::new();
                    debouncers.retain(|&project, debouncer| {
                        if debouncer.take_if_due(now) {
                            due.push(project);
                        }
                        debouncer.due_at().is_some()
                    });
                    for project in due {
                        // A failed reload is dropped, not fatal: a mid-edit save can be
                        // invalid YAML (the config keeps its last good state and the next
                        // save re-triggers), and a project removed while debouncing is
                        // simply unknown. An optional subsystem never crashes the core.
                        let _ = ProjectService::new(
                            &self.projects,
                            &self.config,
                            &supervisor,
                            &self.bus,
                        )
                        .reload(project);
                    }
                }
            }
        }
    }

    /// Rebuilds the `solo.yml`-path index from the registry, wholesale, so a removed project's
    /// config path simply drops out of matching. Which directories are actually watched, and
    /// what a refusal or a degradation means, is
    /// [`crate::watchset::ProjectWatchSet`]'s concern entirely — this reactor only decides which
    /// changed paths, once reported, are worth a reload. A failed registry read changes nothing —
    /// the next lifecycle event re-syncs.
    fn resync(&self, config_paths: &mut HashMap<PathBuf, ProjectId>) {
        let Ok(records) = self.projects.list() else {
            return;
        };
        config_paths.clear();
        for record in records {
            config_paths.insert(config_path(&record.root), record.id);
        }
    }
}

#[cfg(test)]
#[path = "config_watch_tests.rs"]
mod tests;
