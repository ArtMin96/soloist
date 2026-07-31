//! What a session-scoped caller may do to a process (context C8) — the scoped half of process
//! supervision.
//!
//! Each action resolves the session's effective-project scope before routing to the one C2
//! behaviour, so a scoped caller can act only within its own project and the trust gate still
//! refuses an untrusted command. The scope rule itself lives once in
//! [`scoped`](super::scoped); this module only spends it.

use std::time::Duration;

use super::scoped::{ReportToLeadError, ScopedActionError, ScopedFacade, SpawnAgentError};
use super::AgentLaunch;
use crate::ids::{ProcessId, ProjectId};
use crate::process::{ProcessKind, ProcessView};
use crate::supervisor::{ClosePolicy, StartSummary};
use crate::support::orchestration_preamble;
use crate::turn::submitted_turn;

/// How many trailing rendered lines `send_input`'s `wait_ms` snapshot returns — a bounded
/// tail (about a screenful), never the whole scrollback, so the reply stays small.
const INPUT_TAIL_LINES: usize = 24;

/// The most one worker's report may carry. A report is written into the lead's terminal as a
/// turn it will read, so the cap is the same as a timer body's for the same reason: a wake is
/// a summary, and an unbounded one would let a worker flood the context of the agent it is
/// reporting to.
pub const MAX_REPORT_BYTES: usize = 16 * 1024;

/// What an over-cap report is named as in the refusal, matching how the coordination write caps
/// name the payload they refused.
const REPORT: &str = "the report";

/// The longest `send_input` waits before snapshotting the tail, regardless of the requested
/// `wait_ms`. A bound (per the longevity rules) so a large value cannot tie up the request,
/// and it stays well under the IPC client's request timeout.
pub(in crate::facade) const MAX_INPUT_WAIT: Duration = Duration::from_secs(10);

/// What a spawned worker's row is closed on: nothing unless the caller asked for it, and — when
/// the worker has a lead — not until its report has actually reached that lead, so a run whose
/// result never landed keeps its row and its output for the user to read.
fn close_policy(close_when_done: bool, lead: Option<ProcessId>) -> ClosePolicy {
    match (close_when_done, lead) {
        (false, _) => ClosePolicy::Keep,
        (true, None) => ClosePolicy::WhenRunEnds,
        (true, Some(_)) => ClosePolicy::WhenRunEndsAndHandedOver,
    }
}

impl ScopedFacade<'_> {
    /// Starts one process for a scoped session, after confirming it is in scope. The
    /// trust gate in the supervisor still applies, so an untrusted command is refused.
    pub fn start_process(&self, process: ProcessId) -> Result<(), ScopedActionError> {
        self.require_in_scope(process)?;
        self.inner.supervisor().start(process)?;
        Ok(())
    }

    /// Requests a graceful stop of one in-scope process, returning whether it was live.
    pub fn stop_process(&self, process: ProcessId) -> Result<bool, ScopedActionError> {
        self.require_in_scope(process)?;
        Ok(self.inner.supervisor().stop(process))
    }

    /// Restarts one in-scope process (stop then start with its saved config); trust-gated.
    pub fn restart_process(&self, process: ProcessId) -> Result<(), ScopedActionError> {
        self.require_in_scope(process)?;
        self.inner.supervisor().restart(process)?;
        Ok(())
    }

    /// Renames one in-scope process's display label. A scoped action — the label is shared
    /// read-model state every viewer sees — so it is confined to the session's project.
    /// Ungated by trust: a rename runs nothing.
    pub fn rename_process(
        &self,
        process: ProcessId,
        label: String,
    ) -> Result<(), ScopedActionError> {
        self.require_in_scope(process)?;
        self.inner.supervisor().rename(process, label)?;
        Ok(())
    }

    /// Closes one in-scope process: stops and reaps it, then removes it from the registry. A
    /// scoped action confined to the session's project. Async because it awaits the group's
    /// reap before the process is forgotten, so no child is abandoned. Ungated by trust:
    /// stopping and forgetting a process runs nothing.
    pub async fn close_process(&self, process: ProcessId) -> Result<(), ScopedActionError> {
        self.require_in_scope(process)?;
        self.inner.supervisor().close(process).await?;
        Ok(())
    }

    /// Writes input to an in-scope process's PTY — UTF-8 text, including control characters,
    /// sent verbatim (include `\r` to submit a line, `\u{3}` for Ctrl-C). When `wait` is set, waits
    /// up to [`MAX_INPUT_WAIT`] for the process to react, then returns the rendered terminal
    /// tail so the caller sees the effect; without it, returns `None` immediately. The clock
    /// is injected, so a test drives the wait without real time passing.
    pub async fn send_input(
        &self,
        process: ProcessId,
        input: Vec<u8>,
        wait: Option<Duration>,
    ) -> Result<Option<String>, ScopedActionError> {
        self.require_in_scope(process)?;
        self.inner.supervisor().write_stdin(process, input).await?;
        let Some(wait) = wait else {
            return Ok(None);
        };
        self.inner.clock.sleep(wait.min(MAX_INPUT_WAIT)).await;
        Ok(self
            .inner
            .supervisor()
            .rendered_tail(process, INPUT_TAIL_LINES)
            .map(|lines| lines.join("\n")))
    }

    /// Spawns a configured agent tool as a worker in the session's effective project and
    /// starts it, returning its process id — a lead agent spawning a worker over MCP. Reuses
    /// [`Facade::launch_agent`] for the one launch behaviour; the worker always
    /// lands in the caller's own project (the resolved scope), so it can never spawn into
    /// another and needs no project argument. The new agent auto-binds via the injected
    /// `SOLOIST_PROCESS_ID`. Whenever the caller is identifiable — by its binding, else by the
    /// process group it connects from — the worker's lineage is recorded under it so the
    /// orchestration tree nests it; only a caller Soloist did not launch and that never bound
    /// spawns a root. Delegation is one level deep: a caller that was
    /// itself spawned as a worker this run is refused with
    /// [`SpawnAgentError::WorkerMayNotSpawn`], whether it identified itself by binding or is
    /// recognised by the process group it connects from. Only an **agent** can be a lead, so a
    /// caller that resolves to a terminal or a command spawns a root: no lineage edge is recorded
    /// and the worker is briefed as having no lead. With `close_when_done` the worker is closed
    /// once its run ends on its own and — when it has a lead — its report has reached that lead;
    /// without it the finished worker rests in the registry with its output intact.
    ///
    /// The worker opens on an [orchestration preamble](orchestration_preamble) naming its lead,
    /// its project, and the coordination tools, so an agent that loads no skill and reads no
    /// project file still knows what it is and what it can reach. Only the scoped spawn carries
    /// one — an agent the user launches from the dashboard is theirs to open, not a worker being
    /// briefed. Must run within a `tokio` runtime (starting spawns the actor).
    pub fn spawn_agent(
        &self,
        tool: &str,
        extra_args: Vec<String>,
        close_when_done: bool,
    ) -> Result<ProcessId, SpawnAgentError> {
        let project = self
            .inner
            .effective_project(self.session)
            .ok_or(SpawnAgentError::NoProjectScope)?;
        // Delegation is one level deep: a caller recorded as a spawned worker is refused for
        // its whole run — deliberately unfiltered by parent liveness, so a closed lead never
        // promotes its workers to spawners. Refusal precedes the launch: nothing is spawned,
        // registered, or recorded. The caller is resolved from the kernel-reported peer group
        // as well as from its own binding, so a worker cannot lift the gate by never binding.
        let caller_is_worker = [
            self.home_process(),
            self.inner.identity.origin(self.session).process(),
        ]
        .into_iter()
        .flatten()
        .any(|caller| self.inner.lineage.parent_of(caller).is_some());
        if caller_is_worker {
            return Err(SpawnAgentError::WorkerMayNotSpawn);
        }
        // The caller resolved once: it is both the lead the worker is told about and the lead its
        // lineage nests under, so the tree and the briefing can never name different processes.
        // A lead is always an agent — only an agent runs the loop that reads a submitted turn —
        // so a caller Soloist resolves to a terminal or a command is nobody's lead, and the
        // worker it asks for is a root. That is settled here, at the one place an edge is
        // recorded, rather than left for the report to discover: nothing may later type a
        // worker's multi-line result into a live interactive shell, where each line would run.
        let lead = self
            .caller_process()
            .filter(|caller| self.inner.is_agent(*caller));
        let worker = self.inner.launch_agent(
            AgentLaunch::new(project, tool, extra_args)
                .closing(close_policy(close_when_done, lead))
                .opening_with(orchestration_preamble(
                    &self.inner.project_ref(project),
                    lead,
                )),
        )?;
        // A worker nests under its lead whenever Soloist can name an agent caller at all — by its
        // own binding, else by the process group it connects from — so the same identity the gate
        // above recognises is the one the tree records.
        if let Some(lead) = lead {
            self.inner.lineage.record(worker, lead);
        }
        Ok(worker)
    }

    /// Hands this worker's result to the lead that spawned it, delivered as a fresh submitted
    /// turn on the lead's terminal — the reply half of [`spawn_agent`](Self::spawn_agent). The
    /// turn is composed by the same `submitted_turn` a fired timer wakes an agent with, so the
    /// two differ only in the header they carry and a wake reads the same whatever produced it.
    ///
    /// **This is how a worker signals completion.** Terminal quiet is not: a worker that has
    /// finished and one that is still thinking are indistinguishable from outside, so nothing
    /// derived from output can stand in for the worker saying so. A fire-when-idle timer wakes a
    /// lead on quiet, which is where to look, not proof that the delegated work is done.
    ///
    /// **The lead is resolved from the recorded spawn lineage, never named by the caller.** A
    /// caller cannot choose a target, so this can only ever reach the one agent that spawned it —
    /// it is not a way to type into an arbitrary process in the project. A caller with no
    /// recorded lead is refused rather than defaulted to anyone.
    ///
    /// Never blocks: the report is refused rather than awaited if the lead has stopped draining
    /// its input, so one deaf lead cannot stall the caller — and the caller learns its result did
    /// not land instead of being told it did.
    pub fn report_to_lead(&self, report: String) -> Result<(), ReportToLeadError> {
        if report.len() > MAX_REPORT_BYTES {
            return Err(ReportToLeadError::TooLong {
                what: REPORT,
                max_bytes: MAX_REPORT_BYTES,
            });
        }
        let worker = self.caller_process().ok_or(ReportToLeadError::NoLead)?;
        // The registry is the source of truth for who exists at *both* ends of the edge: a caller
        // Soloist has forgotten is no longer a node in the tree, so its recorded edge is stale
        // rather than an address it may still spend.
        let worker_view = self
            .inner
            .process_view(worker)
            .ok_or(ReportToLeadError::NoLead)?;
        let lead = self
            .inner
            .lineage
            .parent_of(worker)
            .ok_or(ReportToLeadError::NoLead)?;
        let lead_view = match self.resolve_in_scope(lead) {
            Ok(view) => view,
            // A lead that has left the registry is gone; the caller's own scope failing to resolve
            // is a different fault, and reporting it as a departed lead would send a worker to
            // abandon a report the lead could still have taken.
            Err(ScopedActionError::UnknownProcess) => return Err(ReportToLeadError::LeadGone),
            Err(err) => return Err(ReportToLeadError::Scope(err)),
        };
        // Only an agent reads a submitted turn as a turn. The spawn records no edge onto anything
        // else, and this is the second line of defence: written into a live shell the report would
        // be submitted with a carriage return and run line by line as commands.
        if lead_view.kind != ProcessKind::Agent {
            return Err(ReportToLeadError::NoLead);
        }
        // A lead whose own run has ended keeps its registry row *and* an input channel nobody
        // drains, so a write into it would be accepted and read by no one. Refuse instead: a
        // worker told its result was delivered has no reason to keep it.
        if !lead_view.status.is_active() {
            return Err(ReportToLeadError::LeadGone);
        }
        let header = format!(
            "[Soloist worker #{worker} \"{}\"] report",
            worker_view.label
        );
        if !self
            .inner
            .supervisor()
            .try_write_stdin(lead, submitted_turn(&header, &report))
        {
            return Err(ReportToLeadError::NotDelivered);
        }
        // The handover a worker's own auto-close waits on. Only now, with the report on its lead's
        // input, may the row and the output behind it be discarded when its run ends.
        self.inner.supervisor().record_handover(worker);
        Ok(())
    }

    /// Starts every trusted command in the session's effective project, regardless of
    /// `auto_start` — the scoped `start_all_commands` tool. Returns what started and what was
    /// skipped as untrusted. Distinct from the dashboard's auto-start path; an untrusted
    /// command is reported, never run.
    pub fn start_all_commands(&self) -> Result<StartSummary, ScopedActionError> {
        let project = self.scope()?;
        Ok(self.inner.supervisor().start_all_commands(project)?)
    }

    /// Gracefully stops every running command in the session's effective project (leaving
    /// agents and terminals running), returning how many were messaged.
    pub fn stop_all_commands(&self) -> Result<usize, ScopedActionError> {
        let project = self.scope()?;
        Ok(self.inner.supervisor().stop_all_commands(project))
    }

    /// Restarts every trusted command in the session's effective project — running ones
    /// cycle, resting ones start — bringing the command set up fresh. Untrusted skipped.
    pub fn restart_all_commands(&self) -> Result<(), ScopedActionError> {
        let project = self.scope()?;
        self.inner.supervisor().restart_all_commands(project)?;
        Ok(())
    }

    /// Every process, with rows outside the session's effective project reduced to identity
    /// — the scoped `list_processes`. A caller keeps a cross-project overview (which projects
    /// and processes exist) but reads no foreign project's ports, exit code, or output-derived
    /// state. With no project in scope every row is foreign, so all are redacted.
    pub fn snapshot_scoped(&self) -> Vec<ProcessView> {
        let scope = self.inner.effective_project(self.session);
        self.inner
            .snapshot()
            .into_iter()
            .map(|view| {
                if Some(view.project) == scope {
                    view
                } else {
                    view.redacted_identity()
                }
            })
            .collect()
    }

    /// The processes in `project`, with rows outside the session's effective project reduced to
    /// identity — the same rule [`snapshot_scoped`](Self::snapshot_scoped) applies, narrowed to one
    /// project.
    ///
    /// Naming a project is not itself a disclosure (`list_projects` already lists them all), so a
    /// foreign project reads back as the bare identities of its processes and nothing more: no
    /// ports, exit code, or output-derived state. Composing this from an unscoped snapshot in an
    /// adapter is what let a scoped caller read a foreign project's rows in full.
    pub fn project_processes_scoped(&self, project: ProjectId) -> Vec<ProcessView> {
        let scope = self.inner.effective_project(self.session);
        self.inner
            .snapshot()
            .into_iter()
            .filter(|view| view.project == project)
            .map(|view| {
                if Some(view.project) == scope {
                    view
                } else {
                    view.redacted_identity()
                }
            })
            .collect()
    }

    /// The services of the session's effective project: its command processes with their
    /// status, discovered ports, and readiness (the [`ProcessView`] read model). Scoped to
    /// the project so a caller sees only its own services; agents and terminals are omitted.
    pub fn services_list(&self) -> Result<Vec<ProcessView>, ScopedActionError> {
        let project = self.scope()?;
        Ok(self
            .inner
            .snapshot()
            .into_iter()
            .filter(|view| view.is_command_in(project))
            .collect())
    }

    /// The status view of one in-scope process — the scoped `get_process_status`. Refuses a
    /// process outside the session's project rather than disclose its state across the
    /// project-isolation boundary; the open [`process_view`](Self::process_view) stays for
    /// the local (unscoped) UI and the HTTP API.
    pub fn process_status_scoped(
        &self,
        process: ProcessId,
    ) -> Result<ProcessView, ScopedActionError> {
        self.resolve_in_scope(process)
    }
}
