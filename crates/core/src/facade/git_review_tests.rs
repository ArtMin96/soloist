//! Behavioural tests for the façade's review door — the one every adapter comes through.
//!
//! The handoff is what most of these are about, because it is the one behaviour that leaves version
//! control: they assemble a real [`Facade`] with a real supervisor over a fake spawner that records
//! every byte written to a process's input, so what is asserted is what actually reached the
//! agent's session.

use std::sync::Arc;
use std::time::Duration;

use tempfile::TempDir;

use crate::composition::CorePorts;
use crate::facade::{Facade, Handoff, HandoffError};
use crate::git::{CheckState, HandoffSubject, MergeMethod, Progress};
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{ProjectRepo, TokioClock};
use crate::process::ProcStatus;
use crate::sync::lock;
use crate::testing::{
    agent_registration, check_run, git_status, pull_request_review, review_thread, FakeGitForge,
    FakeGitRepository, FakeProjectRepo, FakeSpawner, FakeTrustRepo,
};

const BRANCH: &str = "feature";
const CHECK: &str = "build";
const THREAD: &str = "t1";
const NUMBER: u64 = 12;

/// How long a test waits for bytes to travel through the actor's input pump. A bound of its own, so
/// input that never arrives is reported in seconds rather than hanging the suite.
const PATIENCE: Duration = Duration::from_secs(10);
const STEP: Duration = Duration::from_millis(10);

/// Everything a handoff test needs: a façade with one trusted project whose branch has a pull
/// request under review, and the buffer every process's input is recorded into.
struct Harness {
    facade: Arc<Facade>,
    project: ProjectId,
    input: Arc<std::sync::Mutex<Vec<u8>>>,
    forge: FakeGitForge,
    repository: FakeGitRepository,
    _dir: TempDir,
}

fn harness(forge: FakeGitForge) -> Harness {
    let dir = tempfile::tempdir().expect("temp dir");
    let projects = Arc::new(FakeProjectRepo::new());
    let root = dir.path().canonicalize().expect("canonical root");
    let project = projects.upsert(&root, None, None).expect("add project").id;
    let (spawner, input) = FakeSpawner::records_input();
    let repository = FakeGitRepository::reporting(git_status(BRANCH));
    let ports = CorePorts::builder(
        Arc::new(spawner),
        Arc::new(TokioClock),
        Arc::new(FakeTrustRepo::new().trusting_project(project)),
        projects,
    )
    .git_repository(Arc::new(repository.clone()))
    .git_forge(Arc::new(forge.clone()))
    .build();
    Harness {
        facade: Arc::new(Facade::new(ports)),
        project,
        input,
        forge,
        repository,
        _dir: dir,
    }
}

/// A forge whose branch has one failing check and one conversation open on it.
fn reviewed() -> FakeGitForge {
    reviewed_printing("error: the thing is wrong")
}

/// The same forge, where the failing check printed `log`.
fn reviewed_printing(log: &str) -> FakeGitForge {
    FakeGitForge::ready()
        .reviewing(pull_request_review(
            BRANCH,
            vec![check_run(CHECK, CheckState::Failed)],
            vec![review_thread(THREAD, "src/main.rs", 42, "this leaks")],
        ))
        .logging(Some(log))
}

impl Harness {
    /// A running agent in the project, whose input the recorder captures. It is waited for rather
    /// than assumed: an agent that has been asked to start is not one that is running, and a
    /// handoff is offered against what the registry says now.
    async fn agent(&self, name: &str) -> ProcessId {
        let id = self
            .facade
            .supervisor()
            .register(agent_registration(self.project, name));
        self.facade.supervisor().start(id).expect("start the agent");
        let deadline = std::time::Instant::now() + PATIENCE;
        while self
            .facade
            .supervisor()
            .view(id)
            .is_none_or(|view| view.status != ProcStatus::Running)
        {
            assert!(
                std::time::Instant::now() < deadline,
                "the agent never reached running within the budget",
            );
            tokio::time::sleep(STEP).await;
        }
        id
    }

    /// Everything written to any process's input so far.
    fn written(&self) -> String {
        String::from_utf8_lossy(&lock(&self.input)).into_owned()
    }

    /// Waits until `pred` holds, or fails within [`PATIENCE`] — so input that never arrives is a
    /// failure in seconds rather than a suite that hangs.
    async fn until(&self, pred: impl Fn(&str) -> bool) -> String {
        let deadline = std::time::Instant::now() + PATIENCE;
        loop {
            let written = self.written();
            if pred(&written) {
                return written;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "nothing matching arrived in the agent's input within the budget; got: {written}",
            );
            tokio::time::sleep(STEP).await;
        }
    }

    fn check() -> HandoffSubject {
        HandoffSubject::Check {
            name: CHECK.to_string(),
        }
    }
}

#[tokio::test]
async fn a_handoff_lands_in_the_one_running_agents_session() {
    let h = harness(reviewed());
    let agent = h.agent("worker").await;

    let delivered = h
        .facade
        .git_hand_off(h.project, Harness::check(), None)
        .await
        .expect("hand off");

    match delivered {
        Handoff::Delivered { process, text } => {
            assert_eq!(process, agent);
            assert!(text.contains(CHECK));
        }
        Handoff::Copy { .. } => panic!("there is an agent running to take it"),
    }
    let written = h.until(|written| written.contains(CHECK)).await;
    assert!(
        written.contains("error: the thing is wrong"),
        "the whole context reaches the session, not a summary of it: {written}",
    );
}

#[tokio::test]
async fn a_handoff_is_submitted_as_exactly_one_turn() {
    let h = harness(reviewed());
    h.agent("worker").await;

    h.facade
        .git_hand_off(h.project, Harness::check(), None)
        .await
        .expect("hand off");

    let written = h.until(|written| written.contains("end of context")).await;
    assert!(
        written.ends_with('\r') && written.matches('\r').count() == 1,
        "a semantic handoff is submitted exactly once: {written:?}",
    );
}

#[tokio::test]
async fn a_check_that_printed_carriage_returns_is_still_submitted_as_exactly_one_turn() {
    // What a build job printed is the case that matters: its output was written to a terminal, so
    // its lines are separated by the returns a terminal separates lines with. Carried through as
    // they are, each one submits the handoff early and the rest of it arrives as several turns of
    // its own — so what an agent gets is a fragment, then somebody else's build log as commands.
    let h = harness(reviewed_printing(
        "error: first\r\nerror: second\rerror: third\r\n",
    ));
    h.agent("worker").await;

    h.facade
        .git_hand_off(h.project, Harness::check(), None)
        .await
        .expect("hand off");

    // The closing fence is the last of the composed text, so a buffer holding it and ending in a
    // return holds every one of the log's returns too — whatever order the writes arrived in.
    let written = h
        .until(|written| written.contains("end of context") && written.ends_with('\r'))
        .await;
    assert_eq!(
        written.matches('\r').count(),
        1,
        "only the submit is a return; every return the check printed became a newline: {written:?}",
    );
    assert!(
        written.contains("error: first\nerror: second\nerror: third"),
        "the log arrives whole, its lines still separated: {written:?}",
    );
}

#[tokio::test]
async fn with_no_agent_running_the_context_comes_back_to_be_copied() {
    let h = harness(reviewed());

    let answer = h
        .facade
        .git_hand_off(h.project, Harness::check(), None)
        .await
        .expect("an agentless project is not a failure");

    match answer {
        Handoff::Copy { text } => assert!(text.contains(CHECK)),
        Handoff::Delivered { .. } => panic!("there was nobody to deliver to"),
    }
    assert_eq!(h.written(), "", "nothing was written anywhere");
}

#[tokio::test]
async fn with_several_agents_and_none_named_the_context_comes_back_rather_than_being_guessed_at() {
    let h = harness(reviewed());
    h.agent("one").await;
    h.agent("two").await;

    let answer = h
        .facade
        .git_hand_off(h.project, Harness::check(), None)
        .await
        .expect("hand off");

    assert!(
        matches!(answer, Handoff::Copy { .. }),
        "choosing between two sessions would be guessing whose work this belongs to",
    );
    assert_eq!(h.written(), "");
}

#[tokio::test]
async fn a_named_agent_takes_it_even_where_another_is_running() {
    let h = harness(reviewed());
    h.agent("one").await;
    let chosen = h.agent("two").await;

    let delivered = h
        .facade
        .git_hand_off(h.project, Harness::check(), Some(chosen))
        .await
        .expect("hand off");

    assert!(matches!(delivered, Handoff::Delivered { process, .. } if process == chosen));
}

#[tokio::test]
async fn a_process_that_is_not_a_running_agent_of_this_project_takes_nothing() {
    let h = harness(reviewed());
    let stranger = ProcessId::from_raw(9_999);

    let refused = h
        .facade
        .git_hand_off(h.project, Harness::check(), Some(stranger))
        .await
        .expect_err("that is nobody's agent");

    assert!(matches!(refused, HandoffError::NotAnAgent));
    assert_eq!(h.written(), "");
}

#[tokio::test]
async fn a_comment_handed_over_carries_where_in_the_change_it_hangs() {
    let h = harness(reviewed());
    h.agent("worker").await;

    h.facade
        .git_hand_off(
            h.project,
            HandoffSubject::Thread {
                id: THREAD.to_string(),
            },
            None,
        )
        .await
        .expect("hand off");

    let written = h.until(|written| written.contains("this leaks")).await;
    assert!(written.contains("src/main.rs:42"), "{written}");
}

#[tokio::test]
async fn merging_reaches_the_service_and_re_reads_what_the_working_tree_now_says() {
    let h = harness(reviewed());
    let before = h.repository.reads();

    h.facade
        .blocking(move |f| {
            f.git_merge_pull_request(
                h.project,
                NUMBER,
                MergeMethod::Squash,
                &Progress::unwatched(),
            )
        })
        .await
        .expect("merge");

    assert_eq!(h.forge.merged(), vec![(NUMBER, MergeMethod::Squash)]);
    assert!(
        h.repository.reads() > before,
        "the branch it merged is gone from the remote and the working tree stands differently \
         against it, so what every version-control surface shows has to be read again",
    );
}

/// How long the forge is made to dwell answering, and how far short of that a sibling timer is set
/// — wide enough apart that the timer firing first is only possible if the runtime kept scheduling
/// other work while the forge call was still in flight.
const FORGE_DWELL: Duration = Duration::from_millis(200);

#[tokio::test(flavor = "current_thread")]
async fn composing_a_handoff_leaves_the_forge_off_the_runtime_worker() {
    // One worker, deliberately: nothing here can progress by luck of a second OS thread stealing
    // the sibling timer. If composing the handoff ran inline, it would hold that one worker for
    // the whole dwell and the timer below could not fire until it let go — refused or not, the
    // forge is still asked before a target is even looked at.
    let h = harness(reviewed().slow(FORGE_DWELL));
    let stranger = ProcessId::from_raw(9_999);

    let handoff = h.facade.git_hand_off(
        h.project,
        HandoffSubject::Thread {
            id: THREAD.to_string(),
        },
        Some(stranger),
    );
    tokio::pin!(handoff);

    tokio::select! {
        result = &mut handoff => {
            panic!(
                "the hand-off resolved ({result:?}) before a concurrent timer could fire, so \
                 composing it held the runtime worker for the whole forge round trip",
            );
        }
        () = tokio::time::sleep(STEP) => {}
    }

    let refused = handoff.await.expect_err("that is nobody's agent");
    assert!(matches!(refused, HandoffError::NotAnAgent));
}
