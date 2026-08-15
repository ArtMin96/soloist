use std::path::Path;

use soloist_core::ProcessId;
use soloist_ipc::IpcRequest;
use tokio::net::UnixStream;

use crate::ipc::{request, spawned_task, FixtureResult};
use crate::plans::{CLOSE_SIGNAL_FILE, LINEAGE_TASK, POLL_INTERVAL, WORKER_TOOL_ENV};

pub(crate) async fn run(
    stream: &mut UnixStream,
    process: ProcessId,
    data_dir: &Path,
) -> FixtureResult<()> {
    let worker_tool = std::env::var(WORKER_TOOL_ENV)
        .map_err(|_| format!("{WORKER_TOOL_ENV} is not set — the harness names the worker tool"))?;
    spawned_task(
        request(
            stream,
            IpcRequest::SpawnAgent {
                tool: worker_tool.clone(),
                extra_args: Vec::new(),
                prompt: Some(LINEAGE_TASK.to_owned()),
                todo_id: None,
                include_agent_instructions: false,
            },
        )
        .await?,
    )?;
    println!("lead spawned worker ({worker_tool})");

    let close_signal = data_dir.join(CLOSE_SIGNAL_FILE);
    while !close_signal.exists() {
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    let _ = request(stream, IpcRequest::CloseProcess { process }).await;
    Ok(())
}
