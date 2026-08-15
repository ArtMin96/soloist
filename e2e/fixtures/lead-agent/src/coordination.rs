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
