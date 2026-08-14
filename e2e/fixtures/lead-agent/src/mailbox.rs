use soloist_core::{AcquireOutcome, AgentMessageKind, AgentRelationship};
use soloist_ipc::{IpcRequest, IpcResponse};
use tokio::io::BufReader;
use tokio::net::UnixStream;

use crate::ipc::{
    request, retrieve_and_ack, roster, spawned_task, submitted_turn, todo_doc, todo_id,
    FixtureResult,
};
use crate::plans::{MailboxPlan, MailboxRole, MAILBOX_ROLE_ARG};

pub(crate) async fn lead(stream: &mut UnixStream, plan: &MailboxPlan) -> FixtureResult<()> {
    let todo = todo_id(
        request(
            stream,
            IpcRequest::TodoCreate {
                doc: todo_doc(&plan.todo),
                scratchpad: None,
            },
        )
        .await?,
    )?;

    let (primary, primary_task) = spawned_task(
        request(
            stream,
            IpcRequest::SpawnAgent {
                tool: plan.primary.clone(),
                extra_args: vec![format!("{MAILBOX_ROLE_ARG}primary")],
                prompt: Some(plan.task.clone()),
                todo_id: Some(todo),
                // True is omitted by the protocol serializer. The primary's submitted-turn proof
                // therefore exercises the request decoder's default, rather than an explicit true.
                include_agent_instructions: true,
            },
        )
        .await?,
    )?;
    println!(
        "lead queued Task {} for {primary} ({:?})",
        primary_task.0, primary_task.1
    );

    let (peer, peer_task) = spawned_task(
        request(
            stream,
            IpcRequest::SpawnAgent {
                tool: plan.peer.clone(),
                extra_args: vec![format!("{MAILBOX_ROLE_ARG}peer")],
                prompt: Some(plan.peer_task.clone()),
                todo_id: None,
                include_agent_instructions: false,
            },
        )
        .await?,
    )?;
    println!(
        "lead queued peer Task {} for {peer} ({:?})",
        peer_task.0, peer_task.1
    );

    let mut input = BufReader::new(tokio::io::stdin());
    loop {
        let turn = submitted_turn(&mut input).await?;
        println!("lead {}: {}", plan.submitted, turn.trim());
        for message in retrieve_and_ack(stream).await? {
            println!(
                "lead retrieved and acknowledged {:?} {}: {}",
                message.kind, message.id, message.body
            );
            if message.kind == AgentMessageKind::Completion
                && message.body == plan.completion
                && message.todo_id == Some(todo)
            {
                println!("{}", plan.proof);
                return Ok(());
            }
        }
    }
}

pub(crate) async fn worker(
    stream: &mut UnixStream,
    role: MailboxRole,
    plan: &MailboxPlan,
) -> FixtureResult<()> {
    let mut input = BufReader::new(tokio::io::stdin());
    match role {
        MailboxRole::Primary => primary(stream, plan, &mut input).await,
        MailboxRole::Peer => peer(stream, plan, &mut input).await,
    }
}

async fn primary(
    stream: &mut UnixStream,
    plan: &MailboxPlan,
    input: &mut BufReader<tokio::io::Stdin>,
) -> FixtureResult<()> {
    println!("primary ready and waiting for an addressed Task");
    let turn = submitted_turn(input).await?;
    println!("primary {}: {}", plan.submitted, turn.trim());
    if turn.trim() != plan.instructions {
        return Err(format!(
            "default orchestration instructions were not the primary's first submitted turn: {turn:?}"
        )
        .into());
    }
    println!("{}", plan.instructions_received);

    let initial = retrieve_and_ack(stream).await?;
    for message in &initial {
        println!(
            "primary retrieved and acknowledged {:?} {}: {}",
            message.kind, message.id, message.body
        );
    }
    let task = initial
        .iter()
        .find(|message| message.kind == AgentMessageKind::Task && message.body == plan.task)
        .ok_or("primary did not retrieve its addressed Task")?;
    let todo = task.todo_id.ok_or("primary Task carried no todo id")?;
    println!("{}", plan.task_acknowledged);

    let primary_owner = match request(
        stream,
        IpcRequest::LockAcquire {
            key: plan.lease_key.clone(),
            ttl_ms: None,
        },
    )
    .await?
    {
        IpcResponse::LeaseOutcome(AcquireOutcome::Acquired(lease))
            if lease.key == plan.lease_key =>
        {
            println!("{}", plan.primary_lease_acquired);
            lease.owner
        }
        other => return Err(format!("primary did not acquire the file lease: {other:?}").into()),
    };

    let peer = roster(stream)
        .await?
        .into_iter()
        .find(|entry| entry.relationship == AgentRelationship::Sibling && entry.label == plan.peer)
        .ok_or("primary could not find its peer in the live agent roster")?
        .process;
    request(
        stream,
        IpcRequest::AgentMessageBroadcast {
            body: plan.broadcast.clone(),
            todo_id: None,
        },
    )
    .await?;
    request(
        stream,
        IpcRequest::AgentMessageSend {
            recipient: peer,
            body: plan.direct.clone(),
            todo_id: None,
        },
    )
    .await?;

    loop {
        let turn = submitted_turn(input).await?;
        println!("primary {}: {}", plan.submitted, turn.trim());
        let messages = retrieve_and_ack(stream).await?;
        for message in &messages {
            println!(
                "primary retrieved and acknowledged {:?} {}: {}",
                message.kind, message.id, message.body
            );
        }
        if messages.iter().any(|message| {
            message.kind == AgentMessageKind::Direct && message.body == plan.direct_reply
        }) {
            break;
        }
    }

    match request(
        stream,
        IpcRequest::LockRelease {
            key: plan.lease_key.clone(),
        },
    )
    .await?
    {
        IpcResponse::LeaseReleased(true) => println!("{}", plan.primary_lease_released),
        other => return Err(format!("primary did not release the file lease: {other:?}").into()),
    }
    request(
        stream,
        IpcRequest::AgentMessageSend {
            recipient: peer,
            body: plan.lease_ready.clone(),
            todo_id: None,
        },
    )
    .await?;

    if primary_owner == peer {
        return Err("primary lease unexpectedly named the peer as owner".into());
    }

    match request(
        stream,
        IpcRequest::AgentReportCompletion {
            task_message_id: task.id,
            todo_id: Some(todo),
            summary: plan.completion.clone(),
        },
    )
    .await?
    {
        IpcResponse::AgentCompletion(_) => println!("{}", plan.completion_reported),
        other => return Err(format!("expected completion report, got {other:?}").into()),
    }
    Ok(())
}

async fn peer(
    stream: &mut UnixStream,
    plan: &MailboxPlan,
    input: &mut BufReader<tokio::io::Stdin>,
) -> FixtureResult<()> {
    println!("peer ready and waiting for addressed messages");
    let mut accepted_task = false;
    let mut accepted_broadcast = false;
    let mut primary = None;
    while !(accepted_task && accepted_broadcast && primary.is_some()) {
        let turn = submitted_turn(input).await?;
        println!("peer {}: {}", plan.submitted, turn.trim());
        if turn.contains(&plan.instructions) {
            return Err("opted-out peer received orchestration instructions".into());
        }
        for message in retrieve_and_ack(stream).await? {
            println!(
                "peer retrieved and acknowledged {:?} {}: {}",
                message.kind, message.id, message.body
            );
            if message.kind == AgentMessageKind::Task && message.body == plan.peer_task {
                accepted_task = true;
            }
            if message.kind == AgentMessageKind::Direct && message.body == plan.broadcast {
                accepted_broadcast = true;
            }
            if message.kind == AgentMessageKind::Direct && message.body == plan.direct {
                primary = Some(message.sender);
            }
        }
    }
    println!("{}", plan.instructions_suppressed);

    let primary = primary.ok_or("peer did not retain the primary's identity")?;
    match request(
        stream,
        IpcRequest::LockAcquire {
            key: plan.lease_key.clone(),
            ttl_ms: None,
        },
    )
    .await?
    {
        IpcResponse::LeaseOutcome(AcquireOutcome::Held(lease))
            if lease.key == plan.lease_key && lease.owner == primary =>
        {
            println!("{}", plan.peer_lease_held);
        }
        other => {
            return Err(format!("peer did not observe the primary's held lease: {other:?}").into())
        }
    }
    request(
        stream,
        IpcRequest::AgentMessageSend {
            recipient: primary,
            body: plan.direct_reply.clone(),
            todo_id: None,
        },
    )
    .await?;

    loop {
        let turn = submitted_turn(input).await?;
        println!("peer {}: {}", plan.submitted, turn.trim());
        if turn.contains(&plan.instructions) {
            return Err("opted-out peer received orchestration instructions".into());
        }
        let messages = retrieve_and_ack(stream).await?;
        for message in &messages {
            println!(
                "peer retrieved and acknowledged {:?} {}: {}",
                message.kind, message.id, message.body
            );
        }
        if messages.iter().any(|message| {
            message.kind == AgentMessageKind::Direct
                && message.sender == primary
                && message.body == plan.lease_ready
        }) {
            break;
        }
    }

    match request(
        stream,
        IpcRequest::LockAcquire {
            key: plan.lease_key.clone(),
            ttl_ms: None,
        },
    )
    .await?
    {
        IpcResponse::LeaseOutcome(AcquireOutcome::Acquired(lease))
            if lease.key == plan.lease_key && lease.owner != primary =>
        {
            println!("{}", plan.peer_lease_acquired);
        }
        other => {
            return Err(format!("peer did not acquire the released file lease: {other:?}").into())
        }
    }
    match request(
        stream,
        IpcRequest::LockRelease {
            key: plan.lease_key.clone(),
        },
    )
    .await?
    {
        IpcResponse::LeaseReleased(true) => println!("{}", plan.peer_lease_released),
        other => return Err(format!("peer did not release the file lease: {other:?}").into()),
    }
    println!("{}", plan.peer_exchanged);
    Ok(())
}
