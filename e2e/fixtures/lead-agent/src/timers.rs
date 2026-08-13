use soloist_ipc::IpcRequest;
use tokio::io::BufReader;
use tokio::net::UnixStream;

use crate::ipc::{request, spawned_task, submitted_turn, FixtureResult};
use crate::plans::{TimerPlan, WORKER_TOOL_ENV};

pub(crate) async fn run(stream: &mut UnixStream, plan: &TimerPlan) -> FixtureResult<()> {
    let worker_tool = std::env::var(WORKER_TOOL_ENV)
        .map_err(|_| format!("{WORKER_TOOL_ENV} is not set — the harness names the worker tool"))?;
    let (worker, _) = spawned_task(
        request(
            stream,
            IpcRequest::SpawnAgent {
                tool: worker_tool.clone(),
                extra_args: Vec::new(),
                prompt: Some(plan.worker_task.clone()),
                todo_id: None,
                include_agent_instructions: false,
            },
        )
        .await?,
    )?;
    println!("lead spawned worker ({worker_tool})");

    request(
        stream,
        IpcRequest::TimerFireWhenIdleAll {
            body: plan.body.clone(),
            processes: vec![worker],
            max_wait_ms: Some(plan.max_wait_ms),
        },
    )
    .await?;
    println!("lead armed a fire-when-idle-all timer watching the worker");

    let mut input = BufReader::new(tokio::io::stdin());
    let turn = submitted_turn(&mut input).await?;
    println!("{}: {}", plan.submitted, turn.trim());
    Ok(())
}
