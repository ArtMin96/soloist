use soloist_core::{
    AgentMessage, AgentMessageId, AgentMessageOutcome, AgentRosterEntry, ProcessId, TodoDoc,
    TodoId, TodoStatus,
};
use soloist_ipc::{read_frame, write_frame, IpcRequest, IpcResponse, IpcResult};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;

pub(crate) type FixtureResult<T> = Result<T, Box<dyn std::error::Error>>;

pub(crate) async fn request(
    stream: &mut UnixStream,
    req: IpcRequest,
) -> FixtureResult<IpcResponse> {
    write_frame(stream, &req).await?;
    match read_frame::<_, IpcResult>(stream).await? {
        Some(Ok(reply)) => Ok(reply),
        Some(Err(err)) => Err(format!("app refused {req:?}: {err}").into()),
        None => Err(format!("connection closed before replying to {req:?}").into()),
    }
}

pub(crate) fn spawned_task(
    reply: IpcResponse,
) -> FixtureResult<(ProcessId, (AgentMessageId, AgentMessageOutcome))> {
    match reply {
        IpcResponse::SpawnedWithMessage {
            process,
            initial_message_id,
            delivery,
        } => Ok((process, (initial_message_id, delivery))),
        other => {
            Err(format!("expected a prompt-bearing spawned-agent reply, got {other:?}").into())
        }
    }
}

pub(crate) async fn retrieve_and_ack(stream: &mut UnixStream) -> FixtureResult<Vec<AgentMessage>> {
    let pending = match request(stream, IpcRequest::AgentMessageList).await? {
        IpcResponse::AgentMessages(deliveries) => deliveries,
        other => return Err(format!("expected an agent-message list, got {other:?}").into()),
    };
    let mut accepted = Vec::with_capacity(pending.len());
    for summary in pending {
        let summary_id = summary.message.id;
        let message = match request(
            stream,
            IpcRequest::AgentMessageGet {
                message_id: summary_id,
            },
        )
        .await?
        {
            IpcResponse::AgentMessage(delivery) => delivery.message,
            other => {
                return Err(format!("expected agent message {summary_id}, got {other:?}").into())
            }
        };
        match request(
            stream,
            IpcRequest::AgentMessageAcknowledge {
                message_id: message.id,
            },
        )
        .await?
        {
            IpcResponse::AgentMessageDelivery(delivery)
                if delivery.message.id == message.id
                    && delivery.outcome == AgentMessageOutcome::Acknowledged => {}
            other => {
                return Err(
                    format!("message {} was not acknowledged: {other:?}", message.id).into(),
                )
            }
        }
        accepted.push(message);
    }
    Ok(accepted)
}

pub(crate) async fn roster(stream: &mut UnixStream) -> FixtureResult<Vec<AgentRosterEntry>> {
    match request(stream, IpcRequest::AgentRoster).await? {
        IpcResponse::AgentRoster(entries) => Ok(entries),
        other => Err(format!("expected an agent roster, got {other:?}").into()),
    }
}

pub(crate) async fn submitted_turn(
    input: &mut BufReader<tokio::io::Stdin>,
) -> FixtureResult<String> {
    let mut turn = String::new();
    if input.read_line(&mut turn).await? == 0 {
        return Err("PTY input closed before a submitted turn arrived".into());
    }
    Ok(turn)
}

pub(crate) fn scratchpad_revision(reply: IpcResponse) -> FixtureResult<u64> {
    match reply {
        IpcResponse::ScratchpadWritten { scratchpad, .. } => Ok(scratchpad.revision),
        other => Err(format!("expected a scratchpad-written reply, got {other:?}").into()),
    }
}

pub(crate) fn todo_id(reply: IpcResponse) -> FixtureResult<TodoId> {
    match reply {
        IpcResponse::TodoCreated { todo, .. } => Ok(todo.id),
        other => Err(format!("expected a todo-created reply, got {other:?}").into()),
    }
}

pub(crate) fn todo_doc(title: &str) -> TodoDoc {
    TodoDoc {
        title: title.to_owned(),
        body: "Seeded by the e2e lead stub.".to_owned(),
        status: TodoStatus::Open,
    }
}
