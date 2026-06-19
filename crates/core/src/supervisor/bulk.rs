//! Project-wide bulk lifecycle operations: starting, stopping, and restarting every
//! applicable process in a project in one call. Each delegates to the per-process
//! lifecycle and trust gate, so bulk behaviour can never diverge from the single-process
//! paths.

use crate::ids::{ProcessId, ProjectId};

use super::{Supervisor, SupervisorError};

/// The outcome of a bulk start: what was started, and what was skipped because its
/// command variant is not trusted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StartSummary {
    pub started: Vec<ProcessId>,
    pub skipped_untrusted: Vec<ProcessId>,
}

impl Supervisor {
    /// Starts every trusted `auto_start` command in a project; untrusted candidates are
    /// reported, not run.
    pub fn start_all(&self, project: ProjectId) -> Result<StartSummary, SupervisorError> {
        let mut summary = StartSummary::default();
        for candidate in self.registry.auto_start_candidates(project) {
            let trusted = match &candidate.trust_variant {
                Some(variant) => self.trust.is_trusted(project, variant)?,
                None => true,
            };
            if trusted {
                if self.launch_actor(candidate.id, candidate.launch, None) {
                    summary.started.push(candidate.id);
                }
            } else {
                summary.skipped_untrusted.push(candidate.id);
            }
        }
        Ok(summary)
    }

    /// Requests a graceful stop of every live process in a project.
    pub fn stop_all(&self, project: ProjectId) {
        for id in self.registry.live_in(project) {
            self.stop(id);
        }
    }

    /// Restarts every currently-running process in a project (trusted only; an
    /// untrusted one is skipped).
    pub fn restart_running(&self, project: ProjectId) -> Result<(), SupervisorError> {
        for id in self.registry.running_in(project) {
            match self.restart(id) {
                Ok(()) | Err(SupervisorError::Untrusted) | Err(SupervisorError::NotFound(_)) => {}
                Err(err @ SupervisorError::Store(_)) => return Err(err),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::ports::TrustRepo;
    use crate::process::ProcStatus;
    use crate::supervisor::test_support::{
        command_spec, harness, next_to, status_of, terminal, wait_all, PROJECT,
    };
    use crate::supervisor::Registration;
    use crate::testing::FakeSpawner;
    use std::path::Path;

    #[tokio::test]
    async fn start_all_starts_only_trusted_auto_start_commands() {
        let mut h = harness(FakeSpawner::exits_on_kill());
        let auto_trusted = command_spec("run a", true);
        let auto_untrusted = command_spec("run b", true);
        let manual_trusted = command_spec("run c", false);

        let a = h.sup.register(Registration::command(
            PROJECT,
            Path::new("/p"),
            "A",
            &auto_trusted,
        ));
        let b = h.sup.register(Registration::command(
            PROJECT,
            Path::new("/p"),
            "B",
            &auto_untrusted,
        ));
        let c = h.sup.register(Registration::command(
            PROJECT,
            Path::new("/p"),
            "C",
            &manual_trusted,
        ));
        let term = terminal(&h.sup, "bash");

        h.trust
            .set_trusted(PROJECT, &auto_trusted.variant_hash())
            .expect("trust a");
        h.trust
            .set_trusted(PROJECT, &manual_trusted.variant_hash())
            .expect("trust c");

        let summary = h.sup.start_all(PROJECT).expect("start_all");
        assert_eq!(
            summary.started,
            vec![a],
            "only the trusted auto-start command"
        );
        assert_eq!(summary.skipped_untrusted, vec![b]);

        wait_all(&mut h.rx, &[a], ProcStatus::Running).await;
        // The non-auto command, the untrusted one, and the terminal stay put.
        assert_eq!(status_of(&h.sup, b), ProcStatus::Stopped);
        assert_eq!(status_of(&h.sup, c), ProcStatus::Stopped);
        assert_eq!(status_of(&h.sup, term), ProcStatus::Stopped);
    }

    #[tokio::test]
    async fn stop_all_stops_every_live_process_in_the_project() {
        let mut h = harness(FakeSpawner::exits_on_terminate());
        let one = terminal(&h.sup, "sleep 60");
        let two = terminal(&h.sup, "sleep 60");
        h.sup.start(one).expect("start one");
        h.sup.start(two).expect("start two");
        wait_all(&mut h.rx, &[one, two], ProcStatus::Running).await;

        h.sup.stop_all(PROJECT);
        wait_all(&mut h.rx, &[one, two], ProcStatus::Stopped).await;
        assert_eq!(status_of(&h.sup, one), ProcStatus::Stopped);
        assert_eq!(status_of(&h.sup, two), ProcStatus::Stopped);
    }

    #[tokio::test]
    async fn restart_running_restarts_the_running_processes() {
        let mut h = harness(FakeSpawner::exits_on_terminate());
        let id = terminal(&h.sup, "sleep 60");
        h.sup.start(id).expect("start");
        wait_all(&mut h.rx, &[id], ProcStatus::Running).await;

        h.sup.restart_running(PROJECT).expect("restart_running");
        assert_eq!(next_to(&mut h.rx).await, ProcStatus::Restarting);
    }
}
