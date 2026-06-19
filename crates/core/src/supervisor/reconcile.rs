//! Orphan reconciliation: matching the runtime-state file against live process groups
//! on launch, and re-attaching a matched leftover to its registered command.
//!
//! A process group that survived a crash or force-quit is recorded in the runtime-state
//! file. On the next launch the supervisor classifies each record — adopt a live group
//! that matches a registered command, surface an unmatched live group for a user
//! decision, or prune a dead record — and drives an adopted group through the normal
//! actor so it behaves like any other managed process.

use crate::events::DomainEvent;
use crate::ids::ProcessId;
use crate::orphans::{classify, OrphanFate, OrphanInfo, OrphanReport};

use super::{adopt, Supervisor};

impl Supervisor {
    /// Reconciles the runtime-state file against live process groups on launch: prunes
    /// dead records, adopts live groups that match a registered command (re-attaching
    /// them as running), and surfaces unmatched live groups via [`DomainEvent::OrphansFound`]
    /// for a user Kill/Leave decision. Registered commands must be in place before this
    /// is called so matches can be found. Must run within a `tokio` runtime.
    pub fn reconcile_orphans(&self) -> OrphanReport {
        let records = self.runtime.load().unwrap_or_default();
        let fates = classify(
            records,
            |pgid| self.orphan_control.is_alive(pgid),
            |record| {
                self.registry.find_resting_match(
                    &record.project_root,
                    &record.name,
                    &record.command,
                )
            },
        );

        let mut report = OrphanReport::default();
        let mut surfaced = Vec::new();
        for fate in fates {
            match fate {
                OrphanFate::Adopt { record, target } => {
                    if self.adopt_orphan(target, record.pgid) {
                        report.adopted.push(target);
                    } else {
                        // The target was already claimed by another record with the
                        // same identity (a rare duplicate): surface this still-live
                        // group for a user decision rather than leave it running and
                        // unattended.
                        surfaced.push(OrphanInfo::from(&record));
                    }
                }
                OrphanFate::Surface(record) => surfaced.push(OrphanInfo::from(&record)),
                OrphanFate::Prune(record) => {
                    let _ = self.runtime.forget(record.pgid);
                    report.pruned += 1;
                }
            }
        }
        if !surfaced.is_empty() {
            self.bus.publish(DomainEvent::OrphansFound {
                orphans: surfaced.clone(),
            });
            report.surfaced = surfaced;
        }
        report
    }

    /// Re-attaches a leftover process group `pgid` to the resting registered process
    /// `target`, running it through the normal actor over a synthesized handle.
    fn adopt_orphan(&self, target: ProcessId, pgid: i32) -> bool {
        let Some(launch) = self.registry.describe(target).map(|info| info.launch) else {
            return false;
        };
        let spawned = adopt::adopt(pgid, self.orphan_control.clone(), self.clock.clone());
        self.launch_actor(target, launch, Some(spawned))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::OrphanRecord;
    use crate::process::ProcStatus;
    use crate::supervisor::test_support::{
        command_spec, harness, next_to, terminal, wait_all, PROJECT,
    };
    use crate::supervisor::Registration;
    use crate::testing::FakeSpawner;
    use std::path::{Path, PathBuf};
    use std::time::Duration;
    use tokio::sync::broadcast;
    use tokio::sync::broadcast::error::RecvError;

    /// A duration past the adopted-process liveness poll, so a death is observed.
    const PAST_POLL: Duration = Duration::from_secs(2);

    fn orphan_record(name: &str, command: &str, pgid: i32) -> OrphanRecord {
        OrphanRecord {
            project_root: PathBuf::from("/p"),
            name: name.into(),
            command: command.into(),
            pgid,
        }
    }

    async fn next_orphans(rx: &mut broadcast::Receiver<DomainEvent>) -> Vec<OrphanInfo> {
        loop {
            match rx.recv().await {
                Ok(DomainEvent::OrphansFound { orphans }) => return orphans,
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => panic!("event bus closed"),
            }
        }
    }

    #[tokio::test]
    async fn a_running_process_is_recorded_then_forgotten() {
        let mut h = harness(FakeSpawner::exits_on_terminate());
        let id = terminal(&h.sup, "sleep 60");
        h.sup.start(id).expect("start");
        wait_all(&mut h.rx, &[id], ProcStatus::Running).await;

        // While running, the process group is in the runtime-state file.
        assert_eq!(h.runtime.records().len(), 1, "recorded while running");

        h.sup.stop(id);
        wait_all(&mut h.rx, &[id], ProcStatus::Stopped).await;
        tokio::task::yield_now().await;
        assert!(h.runtime.records().is_empty(), "forgotten once reaped");
    }

    #[tokio::test]
    async fn reconcile_adopts_a_matching_live_orphan_then_can_stop_it() {
        let mut h = harness(FakeSpawner::exits_on_terminate());
        // A registered, resting command and a leftover group that matches it.
        let spec = command_spec("npm run dev", false);
        let id = h.sup.register(Registration::command(
            PROJECT,
            Path::new("/p"),
            "Web",
            &spec,
        ));
        h.runtime.seed(orphan_record("Web", "npm run dev", 555));
        h.orphans.set_alive(555);

        let report = h.sup.reconcile_orphans();
        assert_eq!(report.adopted, vec![id], "matched live orphan is adopted");
        assert!(report.surfaced.is_empty());
        wait_all(&mut h.rx, &[id], ProcStatus::Running).await;

        // Stopping the adopted process signals its group and clears its record.
        h.sup.stop(id);
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Stopping);
        tokio::task::yield_now().await;
        h.clock.advance(PAST_POLL);
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Stopped);
        assert!(
            h.orphans.signalled().contains(&(555, false)),
            "SIGTERM to group"
        );
        assert!(h.runtime.records().is_empty(), "record cleared on stop");
    }

    #[tokio::test]
    async fn reconcile_surfaces_an_unmatched_live_orphan() {
        let mut h = harness(FakeSpawner::exits_on_terminate());
        h.runtime.seed(orphan_record("stray", "weird --serve", 777));
        h.orphans.set_alive(777);

        let report = h.sup.reconcile_orphans();
        assert!(report.adopted.is_empty());
        assert_eq!(report.surfaced.len(), 1);
        assert_eq!(report.surfaced[0].pgid, 777);

        // The same candidate is announced for a user Kill/Leave decision.
        let announced = next_orphans(&mut h.rx).await;
        assert_eq!(announced.len(), 1);
        assert_eq!(announced[0].name, "stray");
    }

    #[tokio::test]
    async fn reconcile_prunes_a_dead_orphan() {
        let h = harness(FakeSpawner::exits_on_terminate());
        // Recorded but no longer alive (never marked alive in the fake control).
        h.runtime.seed(orphan_record("gone", "old", 888));

        let report = h.sup.reconcile_orphans();
        assert_eq!(report.pruned, 1);
        assert!(report.adopted.is_empty());
        assert!(report.surfaced.is_empty());
        assert!(h.runtime.records().is_empty(), "stale record pruned");
    }

    #[tokio::test]
    async fn reconcile_surfaces_a_duplicate_that_loses_the_adoption() {
        let mut h = harness(FakeSpawner::exits_on_terminate());
        // One registered command, but two live leftover groups with the same identity.
        let spec = command_spec("npm run dev", false);
        let id = h.sup.register(Registration::command(
            PROJECT,
            Path::new("/p"),
            "Web",
            &spec,
        ));
        h.runtime.seed(orphan_record("Web", "npm run dev", 555));
        h.runtime.seed(orphan_record("Web", "npm run dev", 556));
        h.orphans.set_alive(555);
        h.orphans.set_alive(556);

        // The command can adopt only one group; the duplicate is surfaced for a user
        // decision rather than silently left running and unattended.
        let report = h.sup.reconcile_orphans();
        assert_eq!(report.adopted, vec![id]);
        assert_eq!(
            report.surfaced.len(),
            1,
            "the second live group is surfaced"
        );
        wait_all(&mut h.rx, &[id], ProcStatus::Running).await;
    }
}
