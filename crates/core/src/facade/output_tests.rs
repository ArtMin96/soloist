use super::*;

use crate::events::DomainEvent;
use crate::ids::ProjectId;
use crate::ports::{CorePorts, TokioClock};
use crate::process::ProcStatus;
use crate::testing::{terminal_registration, FakeProjectRepo, FakeSpawner, FakeTrustRepo};
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;

/// A list whose total size exceeds the reply budget: each line is ~100 KiB with a unique
/// prefix, so a handful already overflows the 1 MiB cap and forces a trim while staying
/// distinguishable by `first`/`last`.
fn oversized_lines() -> Vec<String> {
    (0..20)
        .map(|n| format!("line {n} ") + &"x".repeat(100 * 1024))
        .collect()
}

#[test]
fn within_budget_keeps_the_newest_lines_under_the_cap() {
    let lines = oversized_lines();
    let kept = within_reply_budget(lines.clone(), Keep::Newest);

    let bytes: usize = kept.iter().map(|line| line.len() + 1).sum();
    assert!(
        bytes <= MAX_REPLY_BYTES,
        "the kept reply fits the byte budget"
    );
    assert!(kept.len() < lines.len(), "an oversized reply is trimmed");
    assert_eq!(
        kept.last(),
        lines.last(),
        "a tail keeps the most recent line"
    );
}

#[test]
fn within_budget_keeps_the_earliest_matches_under_the_cap() {
    let lines = oversized_lines();
    let kept = within_reply_budget(lines.clone(), Keep::Earliest);

    let bytes: usize = kept.iter().map(|line| line.len() + 1).sum();
    assert!(
        bytes <= MAX_REPLY_BYTES,
        "the kept reply fits the byte budget"
    );
    assert!(kept.len() < lines.len(), "an oversized reply is trimmed");
    assert_eq!(
        kept.first(),
        lines.first(),
        "an ordered match list keeps its earliest entries"
    );
}

#[test]
fn within_budget_returns_everything_under_the_cap_unchanged() {
    let lines = vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()];
    assert_eq!(within_reply_budget(lines.clone(), Keep::Newest), lines);
    assert_eq!(within_reply_budget(lines.clone(), Keep::Earliest), lines);
}

#[test]
fn within_budget_of_nothing_is_nothing() {
    assert!(within_reply_budget(Vec::new(), Keep::Newest).is_empty());
}

/// A façade whose one terminal streams `chunks` then exits, so the public output reads have real
/// buffered lines, bytes, and (empty) ports to return. Starts the process and waits for it to
/// drain to `Stopped` before returning the façade and the process id.
async fn facade_with_streamed_output(chunks: Vec<Vec<u8>>) -> (Facade, ProcessId) {
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::streams_then_exits(chunks)),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .build(),
    );
    let mut rx = facade.subscribe();
    let id =
        facade
            .supervisor()
            .register(terminal_registration(ProjectId::from_raw(1), "term", "cmd"));
    facade.supervisor().start(id).expect("start the terminal");
    loop {
        match rx.recv().await {
            Ok(DomainEvent::ProcessStatusChanged {
                to: ProcStatus::Stopped,
                ..
            }) => break,
            Ok(_) | Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => panic!("event bus closed"),
        }
    }
    (facade, id)
}

#[tokio::test]
async fn process_output_returns_the_buffered_lines_and_honors_the_line_count() {
    let (facade, id) =
        facade_with_streamed_output(vec![b"line one\nline two\nline three\n".to_vec()]).await;

    // No count → all buffered lines (well under the default cap).
    assert_eq!(
        facade.process_output(id, None).expect("registered"),
        vec![
            "line one".to_string(),
            "line two".to_string(),
            "line three".to_string()
        ]
    );
    // An explicit count keeps the newest lines.
    assert_eq!(
        facade.process_output(id, Some(2)).expect("registered"),
        vec!["line two".to_string(), "line three".to_string()]
    );
    // A count far above the ceiling is bounded to what exists (and never exceeds the cap).
    let capped = facade.process_output(id, Some(10_000)).expect("registered");
    assert_eq!(capped.len(), 3);
    assert!(capped.len() <= MAX_OUTPUT_LINES);
}

#[tokio::test]
async fn raw_output_ports_and_search_read_the_buffers() {
    let chunk = b"line one\nline two\nline three\n".to_vec();
    let (facade, id) = facade_with_streamed_output(vec![chunk.clone()]).await;

    // Raw output is the byte scrollback, control sequences intact.
    assert_eq!(facade.process_raw_output(id).expect("registered"), chunk);
    // Search returns only the matching rendered lines.
    assert_eq!(
        facade.search_output(id, "two", None).expect("registered"),
        vec!["line two".to_string()]
    );
    // A registered process with no discovered ports reads as an empty list, not a refusal.
    assert_eq!(
        facade.process_ports(id).expect("registered"),
        Vec::<u16>::new()
    );
}

#[tokio::test]
async fn a_read_of_an_unknown_process_is_none() {
    let (facade, _id) = facade_with_streamed_output(vec![b"anything\n".to_vec()]).await;
    let unknown = ProcessId::from_raw(9_999);

    assert!(facade.process_output(unknown, None).is_none());
    assert!(facade.process_raw_output(unknown).is_none());
    assert!(facade.search_output(unknown, "x", None).is_none());
    assert!(facade.process_ports(unknown).is_none());
}
