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
use crate::ports::{OrphanRecord, SpawnError};

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
            |record| self.orphan_control.is_recorded_alive(record),
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
                    let info = OrphanInfo::from(&record);
                    if self.adopt_orphan(target, record) {
                        report.adopted.push(target);
                    } else {
                        // The target was already claimed by another record with the
                        // same identity (a rare duplicate): surface this still-live
                        // group for a user decision rather than leave it running and
                        // unattended.
                        surfaced.push(info);
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

    /// Forcibly reaps a surfaced orphan the user chose to kill — but only if the recorded
    /// group is still the *same* live process: its identity is re-checked so a pgid the OS
    /// reassigned between surfacing and this call is never SIGKILLed. On a match, SIGKILL
    /// the group and drop its record; on a mismatch or an already-exited group, drop the
    /// stale record without signalling. A failed SIGKILL is returned (the group survives,
    /// so its record is kept for the next launch) so the caller can surface it.
    pub fn kill_orphan(&self, pgid: i32) -> Result<(), SpawnError> {
        let record = self
            .runtime
            .load()
            .unwrap_or_default()
            .into_iter()
            .find(|record| record.pgid == pgid);
        match record {
            Some(record) if self.orphan_control.is_recorded_alive(&record) => {
                self.orphan_control.signal(pgid, true)?;
                let _ = self.runtime.forget(pgid);
                Ok(())
            }
            _ => {
                // No record, already exited, or a recycled pgid whose identity no longer
                // matches — nothing of ours to kill; drop the stale record.
                let _ = self.runtime.forget(pgid);
                Ok(())
            }
        }
    }

    /// Re-attaches a leftover process group to the resting registered process `target`,
    /// running it through the normal actor over a synthesized handle. The whole `record`
    /// (including its captured identity) rides along so the adopted group's liveness poll
    /// re-checks identity, not just a bare pgid.
    fn adopt_orphan(&self, target: ProcessId, record: OrphanRecord) -> bool {
        let Some(launch) = self.registry.describe(target).map(|info| info.launch) else {
            return false;
        };
        let spawned = adopt::adopt(record, self.orphan_control.clone(), self.clock.clone());
        self.launch_actor(target, launch, Some(spawned))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::{OrphanRecord, ProcessIdentity};
    use crate::process::ProcStatus;
    use crate::supervisor::test_support::{
        command_spec, harness, next_to, terminal, wait_all, PROJECT,
    };
    use crate::supervisor::Registration;
    use crate::testing::{fake_identity, FakeSpawner};
    use std::path::{Path, PathBuf};
    use std::time::Duration;
    use tokio::sync::broadcast;
    use tokio::sync::broadcast::error::RecvError;

    /// A duration past the adopted-process liveness poll, so a death is observed.
    const PAST_POLL: Duration = Duration::from_secs(2);

    /// A leftover record stamped with the canonical identity a live group set via
    /// `set_alive` also carries, so a legitimate leftover matches and is reconciled.
    fn orphan_record(name: &str, command: &str, pgid: i32) -> OrphanRecord {
        OrphanRecord {
            project_root: PathBuf::from("/p"),
            name: name.into(),
            command: command.into(),
            pgid,
            identity: Some(fake_identity()),
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

    #[tokio::test]
    async fn kill_orphan_sigkills_the_group_and_forgets_the_record() {
        let h = harness(FakeSpawner::exits_on_terminate());
        h.runtime.seed(orphan_record("stray", "weird --serve", 777));
        h.orphans.set_alive(777);

        h.sup.kill_orphan(777).expect("kill succeeds");

        assert!(
            h.orphans.signalled().contains(&(777, true)),
            "the chosen orphan group is SIGKILLed"
        );
        assert!(
            h.runtime.records().is_empty(),
            "its runtime-state record is forgotten"
        );
    }

    /// A different boot id means the recorded pgid is meaningless — the counter reset —
    /// so the record is pruned, never adopted or offered for kill, even though a matching
    /// command is registered.
    #[tokio::test]
    async fn reconcile_prunes_a_recycled_group_with_a_different_boot_id() {
        let h = harness(FakeSpawner::exits_on_terminate());
        let spec = command_spec("npm run dev", false);
        h.sup.register(Registration::command(
            PROJECT,
            Path::new("/p"),
            "Web",
            &spec,
        ));
        h.runtime.seed(orphan_record("Web", "npm run dev", 555));
        // The live group at pgid 555 belongs to a *different boot* than the record.
        h.orphans.set_identity(
            555,
            ProcessIdentity {
                boot_id: "boot-other".into(),
                started_at: fake_identity().started_at,
            },
        );

        let report = h.sup.reconcile_orphans();
        assert_eq!(report.pruned, 1, "the recycled group is pruned");
        assert!(
            report.adopted.is_empty(),
            "a recycled group is never adopted"
        );
        assert!(
            report.surfaced.is_empty(),
            "a recycled group is never surfaced for kill"
        );
        assert!(h.orphans.signalled().is_empty(), "nothing is signalled");
        assert!(
            h.runtime.records().is_empty(),
            "the stale record is dropped"
        );
    }

    /// Same boot but a different leader start-time means the pgid was reused within this
    /// boot — likewise pruned, never killed.
    #[tokio::test]
    async fn reconcile_prunes_a_recycled_group_with_a_different_start_time() {
        let h = harness(FakeSpawner::exits_on_terminate());
        h.runtime.seed(orphan_record("stray", "weird --serve", 777));
        h.orphans.set_identity(
            777,
            ProcessIdentity {
                boot_id: fake_identity().boot_id,
                started_at: fake_identity().started_at + 5000,
            },
        );

        let report = h.sup.reconcile_orphans();
        assert_eq!(report.pruned, 1);
        assert!(report.surfaced.is_empty());
        assert!(h.orphans.signalled().is_empty());
    }

    /// A legacy record written before identity stamping (no identity fields) is
    /// unverifiable, so it fails closed: pruned, never adopted, surfaced, or killed —
    /// even though its group is live and matches a registered command.
    #[tokio::test]
    async fn reconcile_fails_closed_on_a_legacy_record_without_identity() {
        let h = harness(FakeSpawner::exits_on_terminate());
        let spec = command_spec("npm run dev", false);
        h.sup.register(Registration::command(
            PROJECT,
            Path::new("/p"),
            "Web",
            &spec,
        ));
        h.runtime.seed(OrphanRecord {
            project_root: PathBuf::from("/p"),
            name: "Web".into(),
            command: "npm run dev".into(),
            pgid: 555,
            identity: None,
        });
        h.orphans.set_alive(555);

        let report = h.sup.reconcile_orphans();
        assert_eq!(report.pruned, 1, "a legacy record fails closed to prune");
        assert!(report.adopted.is_empty());
        assert!(report.surfaced.is_empty());
        assert!(h.orphans.signalled().is_empty());
    }

    /// The surface/kill TOCTOU: if the pgid is reassigned between being surfaced and the
    /// user clicking Kill, the identity re-check stops the SIGKILL — the unrelated group
    /// is not touched, and the stale record is dropped.
    #[tokio::test]
    async fn kill_orphan_does_not_signal_a_recycled_group() {
        let h = harness(FakeSpawner::exits_on_terminate());
        h.runtime.seed(orphan_record("stray", "weird --serve", 777));
        // By the time the user acts, pgid 777 is an unrelated group (different boot).
        h.orphans.set_identity(
            777,
            ProcessIdentity {
                boot_id: "boot-other".into(),
                started_at: fake_identity().started_at,
            },
        );

        h.sup
            .kill_orphan(777)
            .expect("no error when nothing is killed");

        assert!(
            h.orphans.signalled().is_empty(),
            "the recycled group is never SIGKILLed"
        );
        assert!(
            h.runtime.records().is_empty(),
            "the stale record is dropped"
        );
    }

    /// A failed SIGKILL is surfaced (returned as an error) and the record is kept, so the
    /// leftover is re-offered on the next launch rather than silently forgotten.
    #[tokio::test]
    async fn kill_orphan_reports_a_failed_signal_and_keeps_the_record() {
        let h = harness(FakeSpawner::exits_on_terminate());
        h.runtime.seed(orphan_record("stray", "weird --serve", 777));
        h.orphans.set_alive(777);
        h.orphans.fail_signals();

        let result = h.sup.kill_orphan(777);

        assert!(
            result.is_err(),
            "a failed SIGKILL is surfaced to the caller"
        );
        assert_eq!(
            h.runtime.records().len(),
            1,
            "the record is kept when the kill failed"
        );
    }
}
