//! Coordination timer tools: scheduling a fresh turn for later, or when watched agents go idle,
//! and managing the timers you set.
//!
//! A timer is owned by the caller's bound process; when it fires it delivers its `body` to that
//! process as a fresh, submitted turn — both the ownership and the delivery are enforced in the
//! core, not here. The fire-when-idle tools are the token-free way for a lead agent to wait until
//! the workers it spawned are done, without polling.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_core::{ProcessId, TimerId};
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::args::{TimerArg, TimerFireWhenIdleArg, TimerSetArg};
use crate::server::SoloistMcp;
use crate::tools::reply::{app_error, structured, unexpected};

#[tool_router(router = timer_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(
        description = "Set a timer that delivers `body` to your bound process as a fresh, submitted turn after `after_ms` milliseconds (omit to fire as soon as possible). Returns the new timer's id and deadline. The timer is one-shot."
    )]
    pub(crate) async fn timer_set(
        &self,
        Parameters(TimerSetArg { body, after_ms }): Parameters<TimerSetArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::TimerSet { body, after_ms })
            .await
        {
            Ok(IpcResponse::TimerArmed(view)) => structured(&view),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Set a timer that delivers `body` to your bound process when ANY watched process is idle, or `max_wait_ms` elapses (omit for the default backstop). Returns the timer plus whether the condition is already met and which processes it is still waiting on. The token-free way to react as soon as one worker is free."
    )]
    pub(crate) async fn timer_fire_when_idle_any(
        &self,
        Parameters(arg): Parameters<TimerFireWhenIdleArg>,
    ) -> Result<CallToolResult, ErrorData> {
        self.fire_when_idle(IpcRequest::TimerFireWhenIdleAny {
            body: arg.body,
            processes: process_ids(arg.processes),
            max_wait_ms: arg.max_wait_ms,
        })
        .await
    }

    #[tool(
        description = "Set a timer that delivers `body` to your bound process when ALL watched processes are idle, or `max_wait_ms` elapses (omit for the default backstop). Returns the timer plus whether the condition is already met and which processes it is still waiting on. The token-free way to wait until every worker you spawned is done."
    )]
    pub(crate) async fn timer_fire_when_idle_all(
        &self,
        Parameters(arg): Parameters<TimerFireWhenIdleArg>,
    ) -> Result<CallToolResult, ErrorData> {
        self.fire_when_idle(IpcRequest::TimerFireWhenIdleAll {
            body: arg.body,
            processes: process_ids(arg.processes),
            max_wait_ms: arg.max_wait_ms,
        })
        .await
    }

    #[tool(
        description = "Cancel a timer you own by id. Returns whether a timer was cancelled (false if it had already fired, been cancelled, or is not yours)."
    )]
    pub(crate) async fn timer_cancel(
        &self,
        Parameters(TimerArg { timer }): Parameters<TimerArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::TimerCancel {
                timer: TimerId::from_raw(timer),
            })
            .await
        {
            Ok(IpcResponse::TimerChanged(cancelled)) => {
                structured(&serde_json::json!({ "cancelled": cancelled }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Pause a timer you own by id, freezing the time that remains until it would fire. Returns whether a timer was paused. A paused timer never fires until resumed."
    )]
    pub(crate) async fn timer_pause(
        &self,
        Parameters(TimerArg { timer }): Parameters<TimerArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::TimerPause {
                timer: TimerId::from_raw(timer),
            })
            .await
        {
            Ok(IpcResponse::TimerChanged(paused)) => {
                structured(&serde_json::json!({ "paused": paused }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Resume a paused timer you own by id, re-arming it with the time that remained when it was paused. Returns whether a timer was resumed."
    )]
    pub(crate) async fn timer_resume(
        &self,
        Parameters(TimerArg { timer }): Parameters<TimerArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::TimerResume {
                timer: TimerId::from_raw(timer),
            })
            .await
        {
            Ok(IpcResponse::TimerChanged(resumed)) => {
                structured(&serde_json::json!({ "resumed": resumed }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "List every timer your bound process owns (armed or paused), with each timer's id, body, fire condition, deadline, and status."
    )]
    pub(crate) async fn timer_list(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::TimerList).await {
            Ok(IpcResponse::Timers(timers)) => structured(&serde_json::json!({ "timers": timers })),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    /// Sends a fire-when-idle request and projects the outcome — shared by the `_any` and `_all`
    /// tools, which differ only in the request variant.
    async fn fire_when_idle(&self, request: IpcRequest) -> Result<CallToolResult, ErrorData> {
        match self.client.request(request).await {
            Ok(IpcResponse::TimerWhenIdle(outcome)) => structured(&outcome),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }
}

/// Maps the wire's bare process ids to the typed [`ProcessId`]s the request carries.
fn process_ids(raw: Vec<u64>) -> Vec<ProcessId> {
    raw.into_iter().map(ProcessId::from_raw).collect()
}
