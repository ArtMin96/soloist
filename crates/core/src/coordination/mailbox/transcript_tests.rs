//! Behavioural tests for the retained transcript, kept out of the implementation file. They drive
//! the aggregate directly over the mock clock, so every timestamp is deterministic with no real
//! time elapsed: both ceilings evict rather than refuse, a closed recipient keeps its history, a
//! removed project loses its own and nothing else's, and an oversized body is cut on a codepoint
//! boundary rather than panicking.

use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::coordination::{AgentMessage, AgentMessageKind};
use crate::ids::ProcessId;
use crate::ports::Clock;
use crate::testing::MockClock;

fn mailbox_on(clock: &MockClock) -> AgentMailbox {
    AgentMailbox::new(Arc::new(clock.clone()))
}

fn mailbox() -> AgentMailbox {
    mailbox_on(&MockClock::new())
}

/// A delivery as the facade would hand one to `record`, with a fresh id each call.
fn delivery(project: ProjectId, body: &str) -> AgentMessageDelivery {
    AgentMessageDelivery {
        message: AgentMessage {
            id: AgentMessageId::next(),
            project,
            sender: ProcessId::next(),
            recipient: ProcessId::next(),
            kind: AgentMessageKind::Direct,
            body: body.to_owned(),
            todo_id: None,
        },
        outcome: AgentMessageOutcome::Queued,
    }
}

fn bodies(mailbox: &AgentMailbox, project: ProjectId) -> Vec<String> {
    mailbox
        .transcript(project)
        .into_iter()
        .map(|record| record.delivery.message.body)
        .collect()
}

#[test]
fn an_overflowing_project_transcript_evicts_its_oldest_entry() {
    let mailbox = mailbox();
    let project = ProjectId::next();
    for index in 0..MAX_TRANSCRIPT_ENTRIES_PER_PROJECT {
        mailbox.record(&delivery(project, &index.to_string()));
    }

    mailbox.record(&delivery(project, "newest"));

    let bodies = bodies(&mailbox, project);
    assert_eq!(bodies.len(), MAX_TRANSCRIPT_ENTRIES_PER_PROJECT);
    assert_eq!(
        bodies.first().map(String::as_str),
        Some("1"),
        "the oldest exchange made room for the newest",
    );
    assert_eq!(bodies.last().map(String::as_str), Some("newest"));
}

#[test]
fn the_application_wide_ceiling_evicts_from_the_fullest_project() {
    let mailbox = mailbox();
    let full = MAX_TRANSCRIPT_ENTRIES / MAX_TRANSCRIPT_ENTRIES_PER_PROJECT;
    let projects: Vec<_> = (0..full).map(|_| ProjectId::next()).collect();
    for project in &projects {
        for index in 0..MAX_TRANSCRIPT_ENTRIES_PER_PROJECT {
            mailbox.record(&delivery(*project, &index.to_string()));
        }
    }
    let latecomer = ProjectId::next();

    mailbox.record(&delivery(latecomer, "latecomer"));

    let retained: usize = projects
        .iter()
        .chain([&latecomer])
        .map(|project| mailbox.transcript(*project).len())
        .sum();
    assert_eq!(
        retained, MAX_TRANSCRIPT_ENTRIES,
        "the application-wide ceiling holds across projects",
    );
    assert_eq!(
        bodies(&mailbox, latecomer),
        vec!["latecomer".to_owned()],
        "the newest exchange is retained rather than refused",
    );
    let victim = projects.first().copied().expect("a filled project");
    assert_eq!(
        bodies(&mailbox, victim).first().map(String::as_str),
        Some("1"),
        "the fullest project gave up its oldest entry",
    );
}

#[test]
fn a_record_survives_the_recipient_closing() {
    let mailbox = mailbox();
    let project = ProjectId::next();
    let sender = ProcessId::next();
    let recipient = ProcessId::next();
    let queued = mailbox
        .enqueue(
            project,
            sender,
            recipient,
            AgentMessageKind::Direct,
            "review the parser".to_owned(),
            None,
        )
        .expect("queue the message");
    mailbox.record(&queued);

    mailbox.remove_process(recipient);

    assert!(
        mailbox.list(recipient).is_empty(),
        "closing the recipient clears its inbox",
    );
    assert_eq!(
        bodies(&mailbox, project),
        vec!["review the parser".to_owned()],
        "a closed worker's messages stay readable",
    );
}

#[test]
fn forgetting_a_project_leaves_every_other_project_intact() {
    let mailbox = mailbox();
    let removed = ProjectId::next();
    let kept = ProjectId::next();
    mailbox.record(&delivery(removed, "in the removed project"));
    mailbox.record(&delivery(kept, "in the kept project"));

    mailbox.forget_project(removed);

    assert!(mailbox.transcript(removed).is_empty());
    assert_eq!(
        bodies(&mailbox, kept),
        vec!["in the kept project".to_owned()],
    );
}

#[test]
fn a_forgotten_projects_entries_are_returned_to_the_application_wide_ceiling() {
    let mailbox = mailbox();
    let forgotten = ProjectId::next();
    let kept = ProjectId::next();
    for index in 0..MAX_TRANSCRIPT_ENTRIES_PER_PROJECT {
        mailbox.record(&delivery(forgotten, &index.to_string()));
    }

    mailbox.forget_project(forgotten);
    // Refilling to the application-wide ceiling must not evict anything: the forgotten project's
    // entries no longer occupy it.
    let full = MAX_TRANSCRIPT_ENTRIES / MAX_TRANSCRIPT_ENTRIES_PER_PROJECT;
    let projects: Vec<_> = (0..full).map(|_| ProjectId::next()).collect();
    for project in &projects {
        for index in 0..MAX_TRANSCRIPT_ENTRIES_PER_PROJECT {
            mailbox.record(&delivery(*project, &index.to_string()));
        }
    }
    mailbox.record(&delivery(kept, "still room"));

    assert_eq!(
        bodies(&mailbox, kept),
        vec!["still room".to_owned()],
        "the newest exchange is retained",
    );
    assert_eq!(
        mailbox.transcript(projects[0]).len(),
        MAX_TRANSCRIPT_ENTRIES_PER_PROJECT - 1,
        "exactly one entry was evicted, so the forgotten project freed its own count",
    );
}

#[test]
fn an_oversized_body_is_truncated_on_a_codepoint_boundary() {
    let mailbox = mailbox();
    let project = ProjectId::next();
    // A three-byte codepoint, so the byte cap lands mid-character and a naive byte cut panics.
    let body = "…".repeat(MAX_TRANSCRIPT_BODY_BYTES);

    mailbox.record(&delivery(project, &body));

    let record = mailbox
        .transcript(project)
        .pop()
        .expect("the exchange was recorded");
    let retained = record.delivery.message.body;
    assert!(record.truncated);
    assert!(retained.len() <= MAX_TRANSCRIPT_BODY_BYTES);
    assert!(
        body.starts_with(&retained),
        "the retained body is a prefix of the original",
    );
    assert!(
        retained.len() > MAX_TRANSCRIPT_BODY_BYTES - "…".len(),
        "the cut walks back to the nearest boundary, not further",
    );
}

#[test]
fn a_body_within_the_cap_is_retained_whole() {
    let mailbox = mailbox();
    let project = ProjectId::next();
    let body = "a".repeat(MAX_TRANSCRIPT_BODY_BYTES);

    mailbox.record(&delivery(project, &body));

    let record = mailbox
        .transcript(project)
        .pop()
        .expect("the exchange was recorded");
    assert!(!record.truncated);
    assert_eq!(record.delivery.message.body, body);
}

#[test]
fn a_record_timestamps_from_the_clock_port() {
    let clock = MockClock::new();
    let mailbox = mailbox_on(&clock);
    let project = ProjectId::next();
    let armed_at = clock.now_unix_millis();
    let elapsed = Duration::from_millis(5_000);

    mailbox.record(&delivery(project, "first"));
    clock.advance(elapsed);
    mailbox.record(&delivery(project, "second"));

    let stamps: Vec<_> = mailbox
        .transcript(project)
        .into_iter()
        .map(|record| record.at_unix_millis)
        .collect();
    assert_eq!(
        stamps,
        vec![armed_at, armed_at + elapsed.as_millis() as u64],
        "each record carries the injected clock's wall time, not the system's",
    );
}

#[test]
fn a_late_outcome_against_an_evicted_entry_neither_resurrects_nor_appends() {
    let mailbox = mailbox();
    let project = ProjectId::next();
    let evicted = delivery(project, "evicted");
    mailbox.record(&evicted);
    for index in 0..MAX_TRANSCRIPT_ENTRIES_PER_PROJECT {
        mailbox.record(&delivery(project, &index.to_string()));
    }

    mailbox.record_outcome(
        project,
        evicted.message.id,
        AgentMessageOutcome::Acknowledged,
    );

    let entries = mailbox.transcript(project);
    assert_eq!(entries.len(), MAX_TRANSCRIPT_ENTRIES_PER_PROJECT);
    assert!(
        !entries
            .iter()
            .any(|record| record.delivery.message.id == evicted.message.id),
        "an evicted exchange stays evicted",
    );
}
