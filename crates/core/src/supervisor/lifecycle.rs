//! The supervisor's lifecycle commands (context C2): what a caller asks of a process.
//!
//! Start, stop, restart, resume, rename, close, and shutdown — each resolves the registry entry,
//! spends the trust gate where the FSM demands it, and drives the owning actor by message. The
//! actor still solely owns its child, PTY, and exit watcher: nothing here touches a child
//! directly, so the single-writer rule holds and a command is a message, never a shared mutation.

use super::actor::ActorMsg;
use super::registration::Registration;
use super::{signal_stop, Supervisor, SupervisorError, MAX_SHUTDOWN_IDLE_PASSES};
use crate::events::DomainEvent;
use crate::ids::{ProcessId, ProjectId};

impl Supervisor {
    /// Starts a process. A trust-gated command whose variant is not trusted is refused
    /// (untrusted cannot run by any path). Starting an already-active
    /// process is a no-op.
    pub fn start(&self, id: ProcessId) -> Result<(), SupervisorError> {
        let info = self
            .registry
            .describe(id)
            .ok_or(SupervisorError::NotFound(id))?;
        if info.status.is_active() {
            return Ok(());
        }
        self.guard_trust(info.project, info.trust_variant.as_ref())?;
        // A user-initiated start is an explicit retry: clear any crash-restart history so
        // a previously exhausted command starts with a fresh rate-limit window.
        self.restart_policy.forget(id);
        self.launch_actor(id, info.launch, None);
        Ok(())
    }

    /// Requests a graceful stop. Returns whether an active process was messaged; a
    /// resting or already-finished process reports `false`. Because the mailbox is installed as
    /// the launch is claimed, an active process always has a live control surface — including one
    /// still in its launch window — so the stop is delivered, never dropped while reporting success.
    pub fn stop(&self, id: ProcessId) -> bool {
        match self.registry.status(id) {
            Some(status) if status.is_active() => {
                self.registry.signal(id, ActorMsg::Stop);
                true
            }
            _ => false,
        }
    }

    /// Restarts a process: a running one is told to cycle in place; a stopped one is
    /// started. Trust is re-checked on either path.
    pub fn restart(&self, id: ProcessId) -> Result<(), SupervisorError> {
        let info = self
            .registry
            .describe(id)
            .ok_or(SupervisorError::NotFound(id))?;
        self.guard_trust(info.project, info.trust_variant.as_ref())?;
        // A user-initiated restart is an explicit retry — reset crash tracking, as a stop
        // would (the auto-restart path relaunches directly and never clears).
        self.restart_policy.forget(id);
        if info.status.is_active() {
            self.registry.signal(id, ActorMsg::Restart);
        } else {
            self.launch_actor(id, info.launch, None);
        }
        Ok(())
    }

    /// Resumes a resting process from its stored resume command — an agent's "Resume last
    /// session", relaunching its CLI on the conversation it left rather than a fresh one. The
    /// resume command runs **in place of** the fresh `launch.command` for this launch only; the
    /// stored fresh command is untouched, so a later plain [`start`](Self::start) still starts
    /// fresh (Start and Resume are independent affordances). Trust is re-checked and crash
    /// history cleared exactly as a start. Refused with [`SupervisorError::NotResumable`] if the
    /// process has no resume command (a command, terminal, or unsupported-provider agent);
    /// resuming an already-active process is a no-op.
    pub fn resume(&self, id: ProcessId) -> Result<(), SupervisorError> {
        let info = self
            .registry
            .describe(id)
            .ok_or(SupervisorError::NotFound(id))?;
        let Some(resume_command) = info.resume_command else {
            return Err(SupervisorError::NotResumable(id));
        };
        if info.status.is_active() {
            return Ok(());
        }
        self.guard_trust(info.project, info.trust_variant.as_ref())?;
        self.restart_policy.forget(id);
        let mut spec = info.launch;
        spec.command = resume_command;
        self.launch_actor(id, spec, None);
        Ok(())
    }

    /// Renames a process's display label, announcing the change so adapters update its row.
    /// The label is display-only: it never affects trust (keyed on the command variant) or
    /// identity/scope. Returns [`SupervisorError::NotFound`] if the process is no longer
    /// registered.
    pub fn rename(&self, id: ProcessId, label: String) -> Result<(), SupervisorError> {
        if !self.registry.set_label(id, label.clone()) {
            return Err(SupervisorError::NotFound(id));
        }
        self.bus.publish(DomainEvent::ProcessRenamed { id, label });
        Ok(())
    }

    /// Resolves a `solo.yml` process name to its registered command's id in `project`, if one
    /// exists — the config-reload path's lookup for the registration to update or drop.
    pub(crate) fn command_id_by_name(&self, project: ProjectId, name: &str) -> Option<ProcessId> {
        self.registry.command_id_by_name(project, name)
    }

    /// Applies a changed `solo.yml` spec to an already-registered command **in place**, keeping
    /// its id (config-reload never duplicates a command) and its live actor if it is running —
    /// the new spec takes effect on the next restart, which the trust gate re-checks. Recomputes
    /// whether the new variant needs trust; announces [`DomainEvent::ProcessRenamed`] only when
    /// the label actually changed (a `solo.yml` rename). Returns whether it was still registered.
    pub(crate) fn update_command(&self, id: ProcessId, registration: Registration) -> bool {
        let Registration {
            project,
            label,
            launch,
            trust_variant,
            auto_start,
            auto_restart,
            restart_when_changed,
            // `kind`, `project_root`, and `resume_command` are invariant for a reloaded command.
            ..
        } = registration;
        let requires_trust = self.requires_trust(project, trust_variant.as_ref());
        let renamed = self
            .registry
            .label_of(id)
            .is_some_and(|previous| previous != label);
        let updated = self.registry.update_command_spec(
            id,
            label.clone(),
            launch,
            trust_variant,
            auto_start,
            auto_restart,
            restart_when_changed,
            requires_trust,
        );
        if updated && renamed {
            self.bus.publish(DomainEvent::ProcessRenamed { id, label });
        }
        updated
    }

    /// Drops a registration **only if it is not active** — the config-reload path removing a
    /// command deleted from `solo.yml` without killing running work. Returns `true` when the
    /// resting entry was removed (announcing [`DomainEvent::ProcessRemoved`]), `false` when the
    /// process was live and so left running for the caller to surface. A resting entry holds no
    /// actor, so removal needs no reap and stays synchronous.
    pub(crate) fn deregister_if_resting(&self, id: ProcessId) -> bool {
        if self.registry.remove_if_resting(id) {
            self.bus.publish(DomainEvent::ProcessRemoved { id });
            true
        } else {
            false
        }
    }

    /// Stops a process and removes it from the registry entirely — the one path that forgets
    /// a managed process, unlike [`stop`](Self::stop), which leaves it resting. The entry is
    /// removed up front, atomically taking any live actor handle; its group is then reaped
    /// (messaged to stop, then awaited) before [`DomainEvent::ProcessRemoved`] is announced,
    /// so no child is abandoned. Removing the entry *first* is what keeps that safe under a
    /// concurrent crash: once the id is gone the self-healing loop's relaunch finds no entry
    /// (`begin_launch` returns `None`), so a crash mid-close cannot resurrect a child that the
    /// removal would then orphan. `ProcessRemoved` also drops the process's crash history
    /// (single source). Returns [`SupervisorError::NotFound`] if it is no longer registered.
    pub async fn close(&self, id: ProcessId) -> Result<(), SupervisorError> {
        let Some(handle) = self.registry.remove_returning_handle(id) else {
            return Err(SupervisorError::NotFound(id));
        };
        // Reap a live actor's group — the single-process form of `shutdown`'s reap step. The
        // entry is already gone, so no relaunch can re-enter the registry behind us. A mid-launch
        // actor whose join is not yet attached has no child to await; the stop (and the dropped
        // mailbox) reach it before it can spawn one.
        if let Some(handle) = handle {
            if let Some(join) = signal_stop(handle) {
                let _ = join.await;
            }
        }
        // The process is gone for good — free its terminal channel (buffers + live broadcast).
        // The actor is reaped above, so its recorder is dropped and nothing still writes here;
        // without this a long session that opens and closes many processes leaks their scrollback.
        self.terminals.remove(id);
        self.bus.publish(DomainEvent::ProcessRemoved { id });
        Ok(())
    }

    /// Stops every live process across all projects and awaits each actor's exit, so no
    /// children leak on app quit (the deterministic-shutdown contract). Wired into the
    /// Tauri shell's exit event so a normal quit reaps every process group.
    pub async fn shutdown(&self) {
        // Latch the policy closed first so no crash during teardown is auto-restarted: the
        // children we are about to reap must not be relaunched.
        self.restart_policy.begin_shutdown();
        // Reap in passes until none remain. A crash whose auto-restart check slipped in
        // just before the latch became visible can still spawn one last actor while we
        // reap; the latch stops the reactor from launching any further, so the set is
        // finite and this converges.
        let mut idle_passes = 0;
        loop {
            let mut joins = Vec::new();
            let mut mid_launch = false;
            for id in self.registry.with_live_actor() {
                match self.registry.take_handle(id) {
                    Some(handle) => {
                        if let Some(join) = signal_stop(handle) {
                            joins.push(join);
                        }
                    }
                    // A launch installed the mailbox but has not attached the join yet (the
                    // launch window). Stop it in place — honored before it can spawn its child —
                    // and retry so its exit is still awaited and no child is abandoned. The
                    // window is a few synchronous statements in the launcher, so it closes at once.
                    None => {
                        self.registry.signal(id, ActorMsg::Stop);
                        mid_launch = true;
                    }
                }
            }
            let awaited = !joins.is_empty();
            for join in joins {
                let _ = join.await;
            }
            if !mid_launch {
                break;
            }
            // Only mid-launch entries remained this pass: yield so their launchers can attach the
            // join before the next scan, rather than spinning on the lock. Bounded per the
            // no-unbounded-retry rule: awaiting a join is progress and resets the count; a run of
            // pure-mid-launch passes with no join to await means a launcher is wedged before
            // `attach_join`, so stop retrying. Bailing is safe — the in-place stop is honored before
            // the actor spawns, so no child is abandoned even unawaited.
            if awaited {
                idle_passes = 0;
            } else {
                idle_passes += 1;
                if idle_passes >= MAX_SHUTDOWN_IDLE_PASSES {
                    break;
                }
                tokio::task::yield_now().await;
            }
        }
    }
}
