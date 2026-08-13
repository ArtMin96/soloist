use super::*;

fn enqueue_direct(
    mailbox: &AgentMailbox,
    project: ProjectId,
    sender: ProcessId,
    recipient: ProcessId,
    body: &str,
) -> Result<AgentMessageDelivery, MailboxCapacityError> {
    mailbox.enqueue(
        project,
        sender,
        recipient,
        AgentMessageKind::Direct,
        body.to_owned(),
        None,
    )
}

#[test]
fn recipient_capacity_refuses_without_dropping_existing_messages() {
    let mailbox = AgentMailbox::new();
    let project = ProjectId::next();
    let sender = ProcessId::next();
    let recipient = ProcessId::next();
    for index in 0..MAX_PENDING_MESSAGES_PER_RECIPIENT {
        enqueue_direct(&mailbox, project, sender, recipient, &index.to_string()).expect("enqueue");
    }

    assert_eq!(
        enqueue_direct(&mailbox, project, sender, recipient, "overflow"),
        Err(MailboxCapacityError::RecipientQueueFull)
    );
    assert_eq!(
        mailbox.list(recipient).len(),
        MAX_PENDING_MESSAGES_PER_RECIPIENT
    );
}

#[test]
fn project_capacity_is_shared_across_recipients() {
    let mailbox = AgentMailbox::new();
    let project = ProjectId::next();
    let sender = ProcessId::next();
    for index in 0..MAX_PENDING_MESSAGES_PER_PROJECT {
        let recipient =
            ProcessId::from_raw(10_000 + (index / MAX_PENDING_MESSAGES_PER_RECIPIENT) as u64);
        enqueue_direct(&mailbox, project, sender, recipient, "x").expect("enqueue");
    }

    assert_eq!(
        enqueue_direct(&mailbox, project, sender, ProcessId::next(), "overflow"),
        Err(MailboxCapacityError::ProjectQueueFull)
    );
}

#[test]
fn global_capacity_is_shared_across_projects() {
    let mailbox = AgentMailbox::new();
    let sender = ProcessId::next();
    for index in 0..MAX_PENDING_AGENT_MESSAGES {
        let project =
            ProjectId::from_raw(20_000 + (index / MAX_PENDING_MESSAGES_PER_PROJECT) as u64);
        let recipient =
            ProcessId::from_raw(30_000 + (index / MAX_PENDING_MESSAGES_PER_RECIPIENT) as u64);
        enqueue_direct(&mailbox, project, sender, recipient, "x").expect("enqueue");
    }

    assert_eq!(
        enqueue_direct(
            &mailbox,
            ProjectId::next(),
            sender,
            ProcessId::next(),
            "overflow",
        ),
        Err(MailboxCapacityError::GlobalQueueFull)
    );
}

#[test]
fn broadcast_capacity_refusal_is_atomic() {
    let mailbox = AgentMailbox::new();
    let project = ProjectId::next();
    let sender = ProcessId::next();
    let full = ProcessId::next();
    let empty = ProcessId::next();
    for _ in 0..MAX_PENDING_MESSAGES_PER_RECIPIENT {
        enqueue_direct(&mailbox, project, sender, full, "x").expect("enqueue");
    }

    assert_eq!(
        mailbox.enqueue_many(
            project,
            sender,
            &[empty, full],
            AgentMessageKind::Direct,
            "broadcast".to_owned(),
            None,
        ),
        Err(MailboxCapacityError::RecipientQueueFull)
    );
    assert!(mailbox.list(empty).is_empty());
}

#[test]
fn reserved_enqueue_cannot_bypass_recipient_capacity() {
    let mailbox = AgentMailbox::new();
    let project = ProjectId::next();
    let sender = ProcessId::next();
    let recipient = ProcessId::next();
    for _ in 0..MAX_PENDING_MESSAGES_PER_RECIPIENT {
        enqueue_direct(&mailbox, project, sender, recipient, "x").expect("enqueue");
    }
    mailbox.reserve_project_slot(project, 4).expect("reserve");

    assert_eq!(
        mailbox.enqueue_reserved(
            project,
            sender,
            recipient,
            AgentMessageKind::Task,
            "task".to_owned(),
            None,
        ),
        Err(MailboxCapacityError::RecipientQueueFull)
    );
    mailbox
        .reserve_project_slot(project, 4)
        .expect("reservation released");
}

#[test]
fn list_and_get_expose_each_messages_delivery_state() {
    let mailbox = AgentMailbox::new();
    let project = ProjectId::next();
    let sender = ProcessId::next();
    let recipient = ProcessId::next();
    let queued = enqueue_direct(&mailbox, project, sender, recipient, "first").expect("enqueue");
    let (envelope, claimed) = mailbox
        .claim_wake_envelope(recipient)
        .expect("wake envelope");
    assert!(envelope.contains(&queued.message.id.to_string()));
    mailbox.mark_wake_submitted(recipient, &claimed);

    assert_eq!(
        mailbox
            .get(recipient, queued.message.id)
            .expect("get")
            .outcome,
        AgentMessageOutcome::WakeSubmitted
    );
    assert_eq!(
        mailbox.list(recipient)[0].outcome,
        AgentMessageOutcome::WakeSubmitted
    );
}

#[test]
fn rapid_sends_submit_only_one_wake_until_activity_rearms_the_recipient() {
    let mailbox = AgentMailbox::new();
    let project = ProjectId::next();
    let sender = ProcessId::next();
    let recipient = ProcessId::next();
    enqueue_direct(&mailbox, project, sender, recipient, "first").expect("enqueue");
    let (_, claimed) = mailbox.claim_wake_envelope(recipient).expect("wake");
    mailbox.mark_wake_submitted(recipient, &claimed);
    let second = enqueue_direct(&mailbox, project, sender, recipient, "second").expect("enqueue");

    assert!(mailbox.claim_wake_envelope(recipient).is_none());
    mailbox.observe_non_idle(recipient);
    let (envelope, _) = mailbox
        .claim_wake_envelope(recipient)
        .expect("rearmed wake");
    assert!(envelope.contains(&second.message.id.to_string()));
}

#[test]
fn a_message_enqueued_after_a_wake_claim_is_not_marked_as_submitted() {
    let mailbox = AgentMailbox::new();
    let project = ProjectId::next();
    let sender = ProcessId::next();
    let recipient = ProcessId::next();
    let first = enqueue_direct(&mailbox, project, sender, recipient, "first").expect("enqueue");
    let (_, claimed) = mailbox.claim_wake_envelope(recipient).expect("wake");
    let second = enqueue_direct(&mailbox, project, sender, recipient, "second").expect("enqueue");

    mailbox.mark_wake_submitted(recipient, &claimed);

    assert_eq!(
        mailbox
            .get(recipient, first.message.id)
            .expect("first")
            .outcome,
        AgentMessageOutcome::WakeSubmitted
    );
    assert_eq!(
        mailbox
            .get(recipient, second.message.id)
            .expect("second")
            .outcome,
        AgentMessageOutcome::Queued
    );
}

#[test]
fn acknowledging_the_submitted_batch_rearms_queued_work() {
    let mailbox = AgentMailbox::new();
    let project = ProjectId::next();
    let sender = ProcessId::next();
    let recipient = ProcessId::next();
    let first = enqueue_direct(&mailbox, project, sender, recipient, "first").expect("enqueue");
    let (_, claimed) = mailbox.claim_wake_envelope(recipient).expect("wake");
    mailbox.mark_wake_submitted(recipient, &claimed);
    let second = enqueue_direct(&mailbox, project, sender, recipient, "second").expect("enqueue");

    mailbox
        .acknowledge(recipient, first.message.id)
        .expect("ack");
    let (envelope, _) = mailbox
        .claim_wake_envelope(recipient)
        .expect("rearmed wake");
    assert!(envelope.contains(&second.message.id.to_string()));
}

#[test]
fn acknowledged_tasks_remain_valid_completion_correlations_until_process_cleanup() {
    let mailbox = AgentMailbox::new();
    let project = ProjectId::next();
    let lead = ProcessId::next();
    let worker = ProcessId::next();
    let task = mailbox
        .enqueue(
            project,
            lead,
            worker,
            AgentMessageKind::Task,
            "research".to_owned(),
            None,
        )
        .expect("enqueue");
    mailbox.acknowledge(worker, task.message.id).expect("ack");

    assert_eq!(
        mailbox.task_for_completion(project, worker, task.message.id),
        Some((lead, None))
    );
    mailbox.remove_process(worker);
    assert_eq!(
        mailbox.task_for_completion(project, worker, task.message.id),
        None
    );
}
