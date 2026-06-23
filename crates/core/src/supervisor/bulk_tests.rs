use std::path::Path;

use crate::ids::ProcessId;
use crate::ports::TrustRepo;
use crate::process::ProcStatus;
use crate::supervisor::test_support::{
    command_spec, harness, next_to, status_of, terminal, wait_all, Harness, PROJECT,
};
use crate::supervisor::Registration;
use crate::testing::FakeSpawner;

/// Registers a command and, when `trusted`, records trust for its variant — the fixture
/// the bulk tests reuse to set up trusted and untrusted commands side by side.
fn command(h: &Harness, name: &str, line: &str, auto_start: bool, trusted: bool) -> ProcessId {
    let spec = command_spec(line, auto_start);
    let id = h
        .sup
        .register(Registration::command(PROJECT, Path::new("/p"), name, &spec));
    if trusted {
        h.trust
            .set_trusted(PROJECT, &spec.variant_hash())
            .expect("trust command");
    }
    id
}

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
async fn start_all_commands_starts_every_trusted_command_regardless_of_auto_start() {
    let mut h = harness(FakeSpawner::exits_on_kill());
    // A trusted command with auto_start off — start_all would skip it, start_all_commands
    // must start it.
    let manual = command(&h, "Manual", "run a", false, true);
    // An untrusted command — reported, never run.
    let untrusted = command(&h, "Untrusted", "run b", true, false);
    let term = terminal(&h.sup, "bash");

    let summary = h
        .sup
        .start_all_commands(PROJECT)
        .expect("start_all_commands");
    assert_eq!(
        summary.started,
        vec![manual],
        "the trusted command, auto or not"
    );
    assert_eq!(summary.skipped_untrusted, vec![untrusted]);

    wait_all(&mut h.rx, &[manual], ProcStatus::Running).await;
    assert_eq!(status_of(&h.sup, untrusted), ProcStatus::Stopped);
    assert_eq!(
        status_of(&h.sup, term),
        ProcStatus::Stopped,
        "terminals are untouched"
    );
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
async fn stop_all_commands_stops_running_commands_but_leaves_terminals() {
    let mut h = harness(FakeSpawner::exits_on_terminate());
    let cmd = command(&h, "Web", "run server", false, true);
    let term = terminal(&h.sup, "sleep 60");
    h.sup.start(cmd).expect("start cmd");
    h.sup.start(term).expect("start term");
    wait_all(&mut h.rx, &[cmd, term], ProcStatus::Running).await;

    let stopped = h.sup.stop_all_commands(PROJECT);
    assert_eq!(stopped, 1, "only the running command is messaged");
    wait_all(&mut h.rx, &[cmd], ProcStatus::Stopped).await;
    assert_eq!(status_of(&h.sup, cmd), ProcStatus::Stopped);
    assert_eq!(
        status_of(&h.sup, term),
        ProcStatus::Running,
        "the terminal keeps running"
    );
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

#[tokio::test]
async fn restart_all_commands_starts_resting_trusted_commands_and_skips_untrusted() {
    let mut h = harness(FakeSpawner::exits_on_kill());
    let resting = command(&h, "Resting", "run a", false, true);
    let untrusted = command(&h, "Untrusted", "run b", false, false);

    h.sup
        .restart_all_commands(PROJECT)
        .expect("restart_all_commands");

    wait_all(&mut h.rx, &[resting], ProcStatus::Running).await;
    assert_eq!(
        status_of(&h.sup, untrusted),
        ProcStatus::Stopped,
        "an untrusted command is not started"
    );
}

#[tokio::test]
async fn restart_all_commands_cycles_a_running_command() {
    let mut h = harness(FakeSpawner::exits_on_terminate());
    let cmd = command(&h, "Web", "run server", false, true);
    h.sup.start(cmd).expect("start");
    wait_all(&mut h.rx, &[cmd], ProcStatus::Running).await;

    h.sup
        .restart_all_commands(PROJECT)
        .expect("restart_all_commands");
    assert_eq!(next_to(&mut h.rx).await, ProcStatus::Restarting);
}
