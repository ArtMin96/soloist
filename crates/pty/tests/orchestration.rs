//! The coordination layer's end-to-end delegation loop (context C6) against real stub agents on
//! real PTYs: a lead spawns a worker, hands it a locked todo, arms a fire-when-idle timer watching
//! the worker, and is woken with a fresh turn the moment the worker goes idle — the token-free
//! orchestration the coordination surface exists to make possible.
//!
//! Everything runs through the one `Facade` over the real PTY spawner, the real idle sampler, and the
//! real timer scheduler, so the worker reaches idle the way it does in the running app — its terminal
//! output settling, classified by the agent idle FSM — rather than by a synthesised event. The
//! durable coordination stores are in-memory fakes (their SQLite implementations are covered by the
//! store's own tests); what this proves is that the pieces compose into the whole loop. It runs on a
//! multi-threaded runtime, as the app does, so the background loops and process actors make steady
//! progress alongside the test even when the machine is busy with the rest of the suite.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::Duration;

use soloist_core::testing::{
    terminal_registration, FakeAgentToolRepo, FakeLockRepo, FakeProjectRepo, FakeTimerRepo,
    FakeTodoRepo, FakeTrustRepo,
};
use soloist_core::{
    AgentActivity, AgentKind, AgentSignal, AgentTool, CorePorts, Facade, IdleMode, PeerCredentials,
    ProcStatus, ProcessId, ProcessKind, ProcessSpec, PromptMode, SpawnProcessRequest, TodoDoc,
    TodoStatus, TokioClock, TrustRepo,
};
use soloist_pty::PtyProcessSpawner;
use tokio::time::{sleep, timeout};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_lead_spawns_a_worker_assigns_a_locked_todo_and_is_woken_when_the_worker_goes_idle() {
    let dir = tempfile::tempdir().expect("temp dir");

    // A worker "agent" that announces itself once and then waits quietly — no further output, so the
    // idle sampler classifies it Idle a few ticks after its terminal settles, exactly as a real agent
    // CLI that has finished its turn and is sitting at an idle prompt.
    let worker_script = dir.path().join("worker.sh");
    std::fs::write(
        &worker_script,
        "#!/bin/sh\nprintf 'WORKER STARTED\\n'\nexec sleep 600\n",
    )
    .expect("write the worker stub");
    std::fs::set_permissions(&worker_script, std::fs::Permissions::from_mode(0o755))
        .expect("chmod the worker stub");

    let worker_tool = AgentTool {
        name: "Worker".into(),
        command: worker_script.to_string_lossy().into_owned(),
        default_args: Vec::new(),
        kind: AgentKind::Generic,
        prompt_mode: PromptMode::AppendedArg,
    };

    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(PtyProcessSpawner),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .agent_tools(Arc::new(FakeAgentToolRepo::new(vec![worker_tool])))
        .lock_repo(Arc::new(FakeLockRepo::new()))
        .timer_repo(Arc::new(FakeTimerRepo::new()))
        .todo_repo(Arc::new(FakeTodoRepo::new()))
        .build(),
    );

    // The two long-lived loops the composition root runs: the idle sampler reclassifies the worker
    // from its terminal output, and the scheduler fires the timer when the worker reaches idle and
    // delivers its body to the lead. Spawned here so the test exercises the same wiring the app runs.
    tokio::spawn(facade.idle_sampler_loop());
    tokio::spawn(facade.timer_scheduler_loop());

    let project = facade
        .projects()
        .add(dir.path(), None, None)
        .expect("register the project");

    // The lead is a real process running `cat`, so a timer body delivered to it is echoed back to its
    // terminal — a visible, end-to-end proof the lead received its fresh turn.
    let lead = facade
        .supervisor()
        .register(terminal_registration(project.id, "lead", "cat"));
    facade.supervisor().start(lead).expect("the lead starts");
    assert!(
        await_status(&facade, lead, ProcStatus::Running).await,
        "the lead reaches Running"
    );

    // Bind a session to the lead the way the UDS adapter binds an MCP client from its peer
    // credentials: authentic against the lead's real process group. This gives the session a project
    // scope and an owner for the todo lock and the timer.
    let pgid = facade
        .supervisor()
        .pgid_of(lead)
        .expect("the running lead has a live group");
    let session = facade.open_session(PeerCredentials::in_group(pgid));
    facade
        .scoped(session)
        .bind_session_process(lead)
        .expect("bind the session to the lead it shares a group with");

    // The lead spawns a worker into its own project — the scoped spawn_agent over launch_agent.
    let worker = facade
        .scoped(session)
        .spawn_agent("Worker", Vec::new())
        .expect("spawn the worker agent");
    assert!(
        await_status(&facade, worker, ProcStatus::Running).await,
        "the worker reaches Running"
    );

    // The lead writes a disciplined todo and locks it — "signals, not ownership": the lock records
    // the lead as the holder.
    let todo = facade
        .scoped(session)
        .todo_create(worker_todo(), None)
        .expect("create the todo")
        .view;
    let locked = facade
        .scoped(session)
        .todo_lock(todo.id)
        .expect("lock the todo");
    assert_eq!(
        locked.locked_by,
        Some(lead),
        "the lead holds the lock on the todo it handed out"
    );

    // The lead arms a fire-when-idle-all timer watching the worker, with a long backstop so only the
    // worker going idle — never the max-wait — can fire it.
    let outcome = facade
        .scoped(session)
        .timer_fire_when_idle(
            "integrate the worker's result".into(),
            vec![worker],
            IdleMode::All,
            Some(Duration::from_secs(3600)),
        )
        .expect("arm the fire-when-idle timer");
    assert!(
        !outcome.timer.already_idle,
        "the freshly spawned worker is not idle yet"
    );
    assert_eq!(
        outcome.timer.waiting_on,
        vec![worker],
        "the timer is waiting on the worker"
    );

    // The worker's terminal settles; the idle sampler classifies it Idle; the scheduler fires the
    // timer and delivers its body to the lead, whose `cat` echoes it. Observing the echo in the
    // lead's rendered output proves the lead got its fresh turn without ever polling.
    let woken = read_rendered_until(
        &facade,
        lead,
        "integrate the worker's result",
        Duration::from_secs(30),
    )
    .await;
    assert!(
        woken,
        "the lead is woken with the timer's body once the worker goes idle"
    );

    // The same real classification the timer fired on is what the UI seeds its idle badges from:
    // `agent_activity` reports the worker as Idle, so a webview reload or a dropped
    // `AgentActivityChanged` recovers the true badge rather than showing edge-triggered stale state.
    assert!(
        facade.agent_activity().contains(&AgentSignal {
            id: worker,
            activity: AgentActivity::Idle,
        }),
        "agent_activity reflects the worker's live idle classification"
    );

    // The fired timer delivered exactly once and is gone, so the lead owns no armed timer now.
    assert!(
        facade
            .scoped(session)
            .timer_list()
            .expect("list timers")
            .is_empty(),
        "the fired timer is consumed, not left armed"
    );

    // Stop and reap the long-lived stubs so the test leaves no survivors.
    facade.supervisor().stop(worker);
    facade.supervisor().stop(lead);
    for id in [worker, lead] {
        assert!(
            await_status(&facade, id, ProcStatus::Stopped).await,
            "the {id:?} stub is reaped"
        );
    }
}

/// A well-formed work item for the worker: a non-blank title and a Markdown body — the fields the
/// todo aggregate validates on write.
fn worker_todo() -> TodoDoc {
    TodoDoc {
        title: "Add the CSV export endpoint".into(),
        body: "Add GET /export and wire it to the report service.\n\n\
               ## Acceptance criteria\n- [ ] GET /export returns 200 with a CSV body"
            .into(),
        status: TodoStatus::InProgress,
    }
}

/// Polls process `id`'s read-model status until it equals `target`, or a bounded budget elapses.
/// Polling the snapshot (rather than waiting on the event stream) is order-independent — two
/// processes stopping in either order are both observed — and robust under load, since a terminal
/// status is stable rather than a transient event that can be missed.
async fn await_status(facade: &Facade, id: ProcessId, target: ProcStatus) -> bool {
    timeout(Duration::from_secs(10), async {
        loop {
            if facade.process_view(id).map(|view| view.status) == Some(target) {
                return true;
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap_or(false)
}

/// Polls process `id`'s rendered terminal tail until a line contains `needle`, or the budget
/// elapses. Returns whether it appeared — the observable that a delivered timer body reached the
/// process's terminal.
async fn read_rendered_until(
    facade: &Facade,
    id: ProcessId,
    needle: &str,
    budget: Duration,
) -> bool {
    timeout(budget, async {
        loop {
            if let Some(lines) = facade.supervisor().rendered_tail(id, 64) {
                if lines.iter().any(|line| line.contains(needle)) {
                    return true;
                }
            }
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap_or(false)
}

/// A lead spawns a **trusted command** into its own project on a real PTY: the child actually
/// runs, and the orchestration tree nests it under the lead that asked for it. An untrusted
/// variant is refused, and — the half that matters — nothing is registered for it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_lead_spawns_a_trusted_command_and_an_untrusted_one_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");

    // A command that announces itself and then waits, so its output proves the real child ran.
    let script = dir.path().join("serve.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\nprintf 'SERVICE LISTENING\\n'\nexec sleep 600\n",
    )
    .expect("write the command stub");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("chmod the command stub");
    let command = script.to_string_lossy().into_owned();

    let trust = Arc::new(FakeTrustRepo::new());
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(PtyProcessSpawner),
            Arc::new(TokioClock),
            trust.clone(),
            Arc::new(FakeProjectRepo::new()),
        )
        .build(),
    );
    let project = facade
        .projects()
        .add(dir.path(), None, None)
        .expect("register the project");

    // The lead is a real process, bound the way the UDS adapter binds an MCP client from its peer
    // credentials — which is what gives the spawn its scope and its parent.
    let lead = facade
        .supervisor()
        .register(terminal_registration(project.id, "lead", "cat"));
    facade.supervisor().start(lead).expect("the lead starts");
    assert!(
        await_status(&facade, lead, ProcStatus::Running).await,
        "the lead reaches Running"
    );
    let pgid = facade
        .supervisor()
        .pgid_of(lead)
        .expect("the running lead has a live group");
    let session = facade.open_session(PeerCredentials::in_group(pgid));
    facade
        .scoped(session)
        .bind_session_process(lead)
        .expect("bind the session to the lead it shares a group with");

    // Untrusted first: the refusal must leave the registry exactly as it was.
    let registered_before = facade.snapshot().len();
    assert!(
        facade
            .scoped(session)
            .spawn_process(spawn_request(&command))
            .is_err(),
        "an untrusted command variant is refused"
    );
    assert_eq!(
        facade.snapshot().len(),
        registered_before,
        "the refusal registered nothing"
    );

    // The user approves that exact variant, as the trust dialog does, and the spawn goes through.
    trust
        .set_trusted(project.id, &spawn_spec(&command).variant_hash(), &command)
        .expect("trust the command variant");
    let spawned = facade
        .scoped(session)
        .spawn_process(spawn_request(&command))
        .expect("a trusted command spawns");

    assert!(
        await_status(&facade, spawned, ProcStatus::Running).await,
        "the spawned command reaches Running"
    );
    assert!(
        read_rendered_until(
            &facade,
            spawned,
            "SERVICE LISTENING",
            Duration::from_secs(30)
        )
        .await,
        "the real child ran and wrote to its own PTY"
    );

    let snapshot = facade
        .orchestration_snapshot(project.id)
        .expect("the orchestration read model");
    let node = snapshot
        .agents
        .iter()
        .find(|node| node.id == spawned)
        .expect("the spawned command is a node in the tree");
    assert_eq!(node.parent, Some(lead), "it nests under the lead");
    assert_eq!(node.kind, ProcessKind::Command);

    facade.supervisor().stop(spawned);
    facade.supervisor().stop(lead);
    for id in [spawned, lead] {
        assert!(
            await_status(&facade, id, ProcStatus::Stopped).await,
            "the {id:?} stub is reaped"
        );
    }
}

/// The spec whose variant the trust dialog approves, and the request that asks to run it — the
/// same three fields, so the two digests agree.
fn spawn_spec(command: &str) -> ProcessSpec {
    ProcessSpec {
        command: command.into(),
        working_dir: None,
        auto_start: false,
        auto_restart: false,
        restart_when_changed: Vec::new(),
        env: BTreeMap::new(),
    }
}

fn spawn_request(command: &str) -> SpawnProcessRequest {
    SpawnProcessRequest {
        command: command.into(),
        working_dir: None,
        env: BTreeMap::new(),
        label: None,
    }
}
