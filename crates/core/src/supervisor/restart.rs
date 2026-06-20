//! Crash auto-restart policy (part of context C2): the rate-limit gate that makes a
//! trusted `auto_restart` command self-healing without hot-looping.
//!
//! Three cohesive pieces live here so the policy is one source of truth, never scattered
//! into the actor or the registry:
//! - [`RestartWindow`] — a pure, per-process sliding window of recent restarts, driven by
//!   [`crate::ports::Clock`]-sourced instants (like [`crate::debounce::Debouncer`]); holds
//!   no timer of its own.
//! - [`RestartPolicy`] — the shared per-process windows plus a shutdown latch.
//! - the [`Supervisor`] glue ([`Supervisor::auto_restart_after_crash`]) and the reactor
//!   ([`Supervisor::self_healing_loop`]) that watches the process event stream and applies
//!   the policy, reusing the supervisor's own launch primitive and trust gate.

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::broadcast::error::RecvError;

use crate::events::DomainEvent;
use crate::ids::ProcessId;
use crate::process::ProcStatus;
use crate::sync::lock;

use super::{apply_transition, Supervisor};

/// The maximum automatic restarts allowed within [`WINDOW`] before a command is held in
/// [`ProcStatus::RestartExhausted`] — the documented crash-restart gate.
const MAX_RESTARTS: u32 = 10;

/// The sliding window over which automatic restarts are counted.
const WINDOW: Duration = Duration::from_secs(60);

/// What the policy decides about a single crash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RestartDecision {
    /// Relaunch the command; `attempt` is its 1-based position in the current window.
    Restart { attempt: u32 },
    /// Hold the command exhausted — too many restarts within the window.
    Exhaust,
}

/// A per-process sliding window of recent automatic-restart instants. Pure and driven by
/// the caller's [`crate::ports::Clock`]-sourced `now`, so it is fully deterministic under
/// the mock clock and adds no timer of its own.
struct RestartWindow {
    restarts: VecDeque<Instant>,
}

impl RestartWindow {
    fn new() -> Self {
        Self {
            restarts: VecDeque::new(),
        }
    }

    /// Decides what to do about a crash at `now`. Restarts that have aged out of the
    /// trailing [`WINDOW`] are dropped first, so a command crashing slowly over time never
    /// exhausts; once [`MAX_RESTARTS`] remain within the window, the next crash exhausts.
    fn on_crash(&mut self, now: Instant) -> RestartDecision {
        if let Some(cutoff) = now.checked_sub(WINDOW) {
            while self.restarts.front().is_some_and(|first| *first < cutoff) {
                self.restarts.pop_front();
            }
        }
        if self.restarts.len() as u32 >= MAX_RESTARTS {
            RestartDecision::Exhaust
        } else {
            self.restarts.push_back(now);
            RestartDecision::Restart {
                attempt: self.restarts.len() as u32,
            }
        }
    }
}

/// The crash auto-restart policy: per-process rate-limit windows plus a shutdown latch.
/// Cloneable; all clones share one state so the supervisor and its reactor agree. The
/// window map is bounded by the number of live processes — an entry is dropped when its
/// process stops, exits cleanly, or leaves the registry.
#[derive(Clone, Default)]
pub(crate) struct RestartPolicy {
    inner: Arc<State>,
}

#[derive(Default)]
struct State {
    windows: Mutex<HashMap<ProcessId, RestartWindow>>,
    shutting_down: AtomicBool,
}

impl RestartPolicy {
    /// Records a crash of `id` at `now` and decides whether to relaunch or exhaust.
    fn on_crash(&self, id: ProcessId, now: Instant) -> RestartDecision {
        lock(&self.inner.windows)
            .entry(id)
            .or_insert_with(RestartWindow::new)
            .on_crash(now)
    }

    /// Drops a process's crash history — a user stop or retry, a clean exit, or removal.
    pub(crate) fn forget(&self, id: ProcessId) {
        lock(&self.inner.windows).remove(&id);
    }

    /// Latches the policy closed so no further crash is auto-restarted (app shutdown).
    pub(crate) fn begin_shutdown(&self) {
        self.inner.shutting_down.store(true, Ordering::SeqCst);
    }

    fn is_shutting_down(&self) -> bool {
        self.inner.shutting_down.load(Ordering::SeqCst)
    }
}

/// The outcome of applying the policy to one crash. Surfaced for tests; the reactor acts
/// through the event bus and ignores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RestartOutcome {
    /// Not eligible to auto-restart: not an `auto_restart` command, now untrusted, gone,
    /// or the app is shutting down.
    NotEligible,
    /// Relaunched; `attempt` is its position in the rate-limit window.
    Restarted { attempt: u32 },
    /// Held in [`ProcStatus::RestartExhausted`].
    Exhausted,
}

impl Supervisor {
    /// Applies the crash auto-restart policy to a crashed process: relaunch a trusted
    /// `auto_restart` command unless it has crashed too often within the window, in which
    /// case hold it exhausted. An ineligible or untrusted command is left crashed, and
    /// nothing is restarted while the app is shutting down.
    pub(crate) fn auto_restart_after_crash(&self, id: ProcessId) -> RestartOutcome {
        if self.restart_policy.is_shutting_down() {
            return RestartOutcome::NotEligible;
        }
        let Some(info) = self.registry.describe(id) else {
            return RestartOutcome::NotEligible;
        };
        if !info.auto_restart {
            return RestartOutcome::NotEligible;
        }
        // Re-check trust on every crash: an untrusted command never auto-restarts, and a
        // store error fails closed (no restart), matching the start gate.
        let trusted = match &info.trust_variant {
            Some(variant) => self
                .trust
                .is_trusted(info.project, variant)
                .unwrap_or(false),
            None => true,
        };
        if !trusted {
            return RestartOutcome::NotEligible;
        }
        match self.restart_policy.on_crash(id, self.clock.now()) {
            RestartDecision::Restart { attempt } => {
                self.bus
                    .publish(DomainEvent::RestartScheduled { id, attempt });
                // Relaunch through the shared launch primitive (the one place a process is
                // spawned) rather than the public `restart`, which would clear the window.
                self.launch_actor(id, info.launch, None);
                RestartOutcome::Restarted { attempt }
            }
            RestartDecision::Exhaust => {
                // Transition from the *current* status: if a concurrent user restart has
                // moved it off Crashed, the edge is illegal and this is a safe no-op.
                let current = self.registry.status(id).unwrap_or(ProcStatus::Crashed);
                let settled = apply_transition(
                    &self.registry,
                    &self.bus,
                    id,
                    current,
                    ProcStatus::RestartExhausted,
                    None,
                );
                if settled == ProcStatus::RestartExhausted {
                    self.bus.publish(DomainEvent::RestartExhausted { id });
                    RestartOutcome::Exhausted
                } else {
                    RestartOutcome::NotEligible
                }
            }
        }
    }

    /// Drops a process's crash-restart history so its rate-limit window never lingers —
    /// called when it stops, exits cleanly, or leaves the registry (bounded state).
    pub(crate) fn clear_restart_tracking(&self, id: ProcessId) {
        self.restart_policy.forget(id);
    }

    /// The self-healing reactor loop: watch the process event stream and drive the crash
    /// auto-restart policy. Returned as a future for the composition root to spawn on its
    /// runtime. It holds only a [`std::sync::Weak`] reference, so it ends when the
    /// supervisor is dropped (app shutdown) instead of keeping it alive; start it once.
    pub(crate) fn self_healing_loop(self: &Arc<Self>) -> impl Future<Output = ()> + Send + 'static {
        let weak = Arc::downgrade(self);
        let mut events = self.bus.subscribe();
        async move {
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    // A lagging reactor missed events; the next crash re-drives it.
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                };
                let Some(sup) = weak.upgrade() else { break };
                match event {
                    DomainEvent::ProcessStatusChanged {
                        id,
                        to: ProcStatus::Crashed,
                        ..
                    } => {
                        sup.auto_restart_after_crash(id);
                    }
                    // A user stop or a clean exit clears crash tracking.
                    DomainEvent::ProcessStatusChanged {
                        id,
                        to: ProcStatus::Stopped,
                        ..
                    } => sup.clear_restart_tracking(id),
                    DomainEvent::ProcessRemoved { id } => sup.clear_restart_tracking(id),
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProcessSpec;
    use crate::ports::{Clock, TrustRepo};
    use crate::supervisor::test_support::{
        auto_restart_spec, harness, status_of, Harness, PROJECT,
    };
    use crate::supervisor::Registration;
    use crate::testing::{FakeSpawner, MockClock};
    use std::collections::BTreeMap;
    use std::path::Path;
    use tokio::sync::broadcast::error::RecvError;

    #[test]
    fn the_window_restarts_up_to_the_max_then_exhausts() {
        let clock = MockClock::new();
        let mut window = RestartWindow::new();
        for attempt in 1..=MAX_RESTARTS {
            assert_eq!(
                window.on_crash(clock.now()),
                RestartDecision::Restart { attempt }
            );
        }
        // The crash past the limit is held, and stays held while the window is full.
        assert_eq!(window.on_crash(clock.now()), RestartDecision::Exhaust);
        assert_eq!(window.on_crash(clock.now()), RestartDecision::Exhaust);
    }

    #[test]
    fn restarts_aging_out_of_the_window_do_not_exhaust() {
        let clock = MockClock::new();
        let mut window = RestartWindow::new();
        for _ in 0..MAX_RESTARTS {
            window.on_crash(clock.now());
        }
        // Past the window, every recorded restart ages out, so the next crash restarts.
        clock.advance(WINDOW + Duration::from_secs(1));
        assert_eq!(
            window.on_crash(clock.now()),
            RestartDecision::Restart { attempt: 1 }
        );
    }

    #[test]
    fn forgetting_a_process_clears_its_window() {
        let clock = MockClock::new();
        let policy = RestartPolicy::default();
        let id = ProcessId::next();
        for _ in 0..MAX_RESTARTS {
            policy.on_crash(id, clock.now());
        }
        assert_eq!(policy.on_crash(id, clock.now()), RestartDecision::Exhaust);
        // A user retry forgets the history, so the command gets a fresh window.
        policy.forget(id);
        assert_eq!(
            policy.on_crash(id, clock.now()),
            RestartDecision::Restart { attempt: 1 }
        );
    }

    fn register_trusted_auto_restart(h: &Harness) -> ProcessId {
        let spec = auto_restart_spec("crash");
        let id = h.sup.register(Registration::command(
            PROJECT,
            Path::new("/p"),
            "Crasher",
            &spec,
        ));
        h.trust
            .set_trusted(PROJECT, &spec.variant_hash())
            .expect("trust");
        id
    }

    #[tokio::test]
    async fn a_crashing_command_is_restarted_until_the_limit_then_exhausted() {
        // A command that crashes on every launch is relaunched exactly MAX_RESTARTS times
        // within the window, then held — proving the gate and the no-hot-loop guarantee.
        let mut h = harness(FakeSpawner::exits_with_code(1));
        let id = register_trusted_auto_restart(&h);
        tokio::spawn(h.sup.self_healing_loop());
        h.sup.start(id).expect("start");

        let mut scheduled = 0u32;
        loop {
            match h.rx.recv().await {
                Ok(DomainEvent::RestartScheduled { id: got, attempt }) if got == id => {
                    scheduled += 1;
                    assert_eq!(attempt, scheduled, "attempts are sequential");
                }
                Ok(DomainEvent::RestartExhausted { id: got }) if got == id => break,
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => panic!("event bus closed"),
            }
        }
        assert_eq!(scheduled, MAX_RESTARTS, "restarted exactly the limit");
        assert_eq!(status_of(&h.sup, id), ProcStatus::RestartExhausted);
    }

    #[tokio::test]
    async fn shutdown_disables_auto_restart() {
        // An otherwise-eligible (trusted, auto_restart) command is not relaunched once the
        // app is shutting down — quitting must not resurrect the children it is reaping.
        let h = harness(FakeSpawner::exits_with_code(1));
        let id = register_trusted_auto_restart(&h);
        h.sup.shutdown().await;
        assert_eq!(
            h.sup.auto_restart_after_crash(id),
            RestartOutcome::NotEligible
        );
    }

    #[tokio::test]
    async fn an_untrusted_or_non_auto_restart_command_is_not_restarted() {
        let h = harness(FakeSpawner::exits_with_code(1));

        // auto_restart but never trusted → never auto-restarts.
        let spec = auto_restart_spec("crash");
        let untrusted = h.sup.register(Registration::command(
            PROJECT,
            Path::new("/p"),
            "Untrusted",
            &spec,
        ));
        assert_eq!(
            h.sup.auto_restart_after_crash(untrusted),
            RestartOutcome::NotEligible
        );

        // trusted but auto_restart off → not a restart candidate.
        let plain = ProcessSpec {
            command: "crash".into(),
            working_dir: None,
            auto_start: false,
            auto_restart: false,
            restart_when_changed: Vec::new(),
            env: BTreeMap::new(),
        };
        let no_restart = h.sup.register(Registration::command(
            PROJECT,
            Path::new("/p"),
            "NoRestart",
            &plain,
        ));
        h.trust
            .set_trusted(PROJECT, &plain.variant_hash())
            .expect("trust");
        assert_eq!(
            h.sup.auto_restart_after_crash(no_restart),
            RestartOutcome::NotEligible
        );
    }
}
