//! Behaviour tests for the per-process terminal channel: the restart banner the actor
//! draws between runs, and the buffer/live-stream reuse that lets a process's output
//! survive a relaunch (so a crash auto-restart neither wipes the scrollback nor freezes
//! an attached pane).

use super::*;
use crate::ids::ProcessId;

#[test]
fn a_restart_mark_draws_a_banner_only_once_there_is_prior_output() {
    let terminals = Terminals::default();
    let id = ProcessId::from_raw(1);
    let actor = terminals.open(id);

    // Nothing has run yet, so there is no boundary to draw — the mark is a no-op.
    actor.recorder.mark_restart();
    assert!(terminals.scrollback(id).expect("channel").is_empty());
    assert!(terminals.rendered(id).expect("channel").lines.is_empty());

    // Once a run has produced output, a restart mark inserts the banner before what the
    // next run will print, and the prior output is left intact.
    actor.recorder.record(b"hello\n".to_vec());
    actor.recorder.mark_restart();

    let rendered = terminals.rendered(id).expect("channel");
    assert!(
        rendered.lines.iter().any(|line| line.contains("restarted")),
        "the rendered scrollback shows a restart banner: {rendered:?}"
    );
    let raw = String::from_utf8_lossy(&terminals.scrollback(id).expect("channel")).into_owned();
    assert!(raw.contains("hello"), "the prior run's output is retained");
    assert!(
        raw.contains("restarted"),
        "the raw stream carries the banner"
    );
}

#[tokio::test]
async fn a_relaunch_reuses_the_buffers_and_the_live_stream() {
    let terminals = Terminals::default();
    let id = ProcessId::from_raw(2);

    // First run produces output.
    let first = terminals.open(id);
    first.recorder.record(b"run one\n".to_vec());

    // A viewer attaches mid-life, capturing the scrollback and a live subscription.
    let (scrollback, mut live) = terminals.attach(id).expect("a terminal channel");
    assert!(String::from_utf8_lossy(&scrollback).contains("run one"));

    // The process crashes and is relaunched: a new actor opens the same channel. The
    // previous run's output must survive, and the viewer must stay subscribed across the
    // relaunch rather than freezing against a dropped sender.
    let second = terminals.open(id);
    assert!(
        String::from_utf8_lossy(&terminals.scrollback(id).expect("channel")).contains("run one"),
        "the relaunch keeps the previous run's output"
    );

    second.recorder.mark_restart();
    second.recorder.record(b"run two\n".to_vec());

    // The viewer that attached before the relaunch still receives the banner and the new
    // run's output over the same live stream.
    let banner = live.recv().await.expect("banner chunk");
    assert!(String::from_utf8_lossy(&banner).contains("restarted"));
    let next = live.recv().await.expect("new output chunk");
    assert_eq!(next.as_ref(), b"run two\n");
}
