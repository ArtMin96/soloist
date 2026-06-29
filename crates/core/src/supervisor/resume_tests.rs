//! [`Supervisor::resume`]: replaying an agent's stored resume command for "Resume last
//! session". Resume relaunches the resting process with its resume command in place of the
//! fresh one, leaves the fresh command intact for a later plain start, and is refused for a
//! process with no last session to resume.

use crate::process::{ProcStatus, ProcessKind};
use crate::supervisor::test_support::{harness, spawn_spec, terminal, wait_all, PROJECT};
use crate::supervisor::{Registration, SupervisorError};
use crate::testing::FakeSpawner;

/// A resting agent whose fresh launch is `claude` and whose resume relaunch is
/// `claude --continue` — the shape the façade builds for a resumable provider.
fn resumable_agent(sup: &crate::supervisor::Supervisor) -> crate::ids::ProcessId {
    sup.register(
        Registration::launched(PROJECT, ProcessKind::Agent, "Claude", spawn_spec("claude"))
            .resumable_with(Some("claude --continue".to_string())),
    )
}

#[tokio::test]
async fn resume_relaunches_a_stopped_agent_with_its_resume_command() {
    let (spawner, commands) = FakeSpawner::records_command();
    let mut h = harness(spawner);
    let id = resumable_agent(&h.sup);
    // A resume command was stored, so the read-model marks the process resumable.
    assert!(h.sup.view(id).expect("registered").resumable);

    // A fresh start runs the original command; once it rests, resume runs the resume command.
    h.sup.start(id).expect("start");
    wait_all(&mut h.rx, &[id], ProcStatus::Running).await;
    h.sup.stop(id);
    wait_all(&mut h.rx, &[id], ProcStatus::Stopped).await;

    h.sup.resume(id).expect("resume");
    wait_all(&mut h.rx, &[id], ProcStatus::Running).await;

    assert_eq!(
        *commands.lock().expect("commands"),
        vec!["claude".to_string(), "claude --continue".to_string()],
        "the fresh start ran `claude`, the resume ran `claude --continue`"
    );
}

#[tokio::test]
async fn resume_does_not_replace_the_fresh_command_for_a_later_start() {
    // Resume is a one-off relaunch: it must not overwrite the stored fresh command, so Start
    // (fresh) and Resume (continue) stay independent across stop/start cycles.
    let (spawner, commands) = FakeSpawner::records_command();
    let mut h = harness(spawner);
    let id = resumable_agent(&h.sup);

    h.sup.start(id).expect("start");
    wait_all(&mut h.rx, &[id], ProcStatus::Running).await;
    h.sup.stop(id);
    wait_all(&mut h.rx, &[id], ProcStatus::Stopped).await;

    h.sup.resume(id).expect("resume");
    wait_all(&mut h.rx, &[id], ProcStatus::Running).await;
    h.sup.stop(id);
    wait_all(&mut h.rx, &[id], ProcStatus::Stopped).await;

    // A plain start after a resume uses the original fresh command again.
    h.sup.start(id).expect("start fresh again");
    wait_all(&mut h.rx, &[id], ProcStatus::Running).await;

    assert_eq!(
        *commands.lock().expect("commands"),
        vec![
            "claude".to_string(),
            "claude --continue".to_string(),
            "claude".to_string(),
        ],
        "start → resume → start runs fresh, resume, then fresh again"
    );
}

#[tokio::test]
async fn resume_is_refused_for_a_process_with_no_last_session() {
    let h = harness(FakeSpawner::exits_on_terminate());
    let term = terminal(&h.sup, "bash");
    assert!(
        !h.sup.view(term).expect("registered").resumable,
        "a terminal has no resume command, so it is not resumable"
    );
    assert!(matches!(
        h.sup.resume(term),
        Err(SupervisorError::NotResumable(_))
    ));
}

#[tokio::test]
async fn resuming_an_already_running_agent_does_not_relaunch_it() {
    let (spawner, commands) = FakeSpawner::records_command();
    let mut h = harness(spawner);
    let id = resumable_agent(&h.sup);

    h.sup.start(id).expect("start");
    wait_all(&mut h.rx, &[id], ProcStatus::Running).await;

    // Resuming an active process is a no-op — it never spawns a second child.
    h.sup.resume(id).expect("resume is a no-op while active");
    tokio::task::yield_now().await;
    assert_eq!(
        *commands.lock().expect("commands"),
        vec!["claude".to_string()],
        "no resume relaunch while the agent is still running"
    );
}
