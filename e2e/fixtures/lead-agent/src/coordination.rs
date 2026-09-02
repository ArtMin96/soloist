use std::path::Path;

use soloist_ipc::IpcRequest;
use tokio::net::UnixStream;

use crate::ipc::{request, scratchpad_revision, todo_doc, todo_id, FixtureResult};
use crate::plans::{CoordinationPlan, POLL_INTERVAL, SCRATCHPAD_REWRITE_FILE};

pub(crate) async fn run(
    stream: &mut UnixStream,
    plan: &CoordinationPlan,
    data_dir: &Path,
) -> FixtureResult<()> {
    let base_revision = scratchpad_revision(
        request(
            stream,
            IpcRequest::ScratchpadWrite {
                name: plan.scratchpad.clone(),
                body: plan.body_v1.clone(),
                expected_revision: None,
            },
        )
        .await?,
    )?;
    println!(
        "lead created scratchpad {} at revision {base_revision}",
        plan.scratchpad
    );

    let blocker = todo_id(
        request(
            stream,
            IpcRequest::TodoCreate {
                doc: todo_doc(&plan.blocker),
                scratchpad: None,
            },
        )
        .await?,
    )?;
    let blocked = todo_id(
        request(
            stream,
            IpcRequest::TodoCreate {
                doc: todo_doc(&plan.blocked),
                scratchpad: None,
            },
        )
        .await?,
    )?;
    request(
        stream,
        IpcRequest::TodoSetBlockers {
            todo: blocked,
            blockers: vec![blocker],
        },
    )
    .await?;

    let commented = todo_id(
        request(
            stream,
            IpcRequest::TodoCreate {
                doc: todo_doc(&plan.commented),
                scratchpad: None,
            },
        )
        .await?,
    )?;
    request(
        stream,
        IpcRequest::TodoCommentCreate {
            todo: commented,
            body: plan.comment.clone(),
        },
    )
    .await?;
    println!("lead seeded a blocker chain and a comment");

    // A second chain with two unmet blockers, so the board has both a singular and a plural
    // unmet-blocker count to show from the same wire-built state.
    request(
        stream,
        IpcRequest::TodoSetBlockers {
            todo: commented,
            blockers: vec![blocker, blocked],
        },
    )
    .await?;

    // The session-work context the agent terminal header renders is recorded by the core from
    // these tool calls: the lock is what "Current work" derives from, and a read through a tool is
    // what "This session" is made of.
    request(stream, IpcRequest::TodoLock { todo: blocked }).await?;
    request(stream, IpcRequest::TodoGet { todo: commented }).await?;
    request(
        stream,
        IpcRequest::ScratchpadRead {
            name: plan.scratchpad.clone(),
        },
    )
    .await?;
    println!("lead locked a todo and read a todo and the scratchpad");

    let rewrite = data_dir.join(SCRATCHPAD_REWRITE_FILE);
    while !rewrite.exists() {
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    request(
        stream,
        IpcRequest::ScratchpadWrite {
            name: plan.scratchpad.clone(),
            body: plan.body_v2.clone(),
            expected_revision: Some(base_revision),
        },
    )
    .await?;
    println!("lead re-wrote scratchpad {}", plan.scratchpad);
    Ok(())
}
