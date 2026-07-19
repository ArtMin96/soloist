//! Test doubles shared by the crate's test modules: one fake app and one handler builder, so a
//! test of the tool surface and a test of the prompts primitive drive the same real
//! [`SoloistMcp`] over the same real IPC transport.

use std::path::PathBuf;
use std::sync::Arc;

use soloist_core::McpToolGroups;
use soloist_ipc::{read_frame, write_frame, IpcRequest, IpcResult};
use tokio::net::UnixListener;

use crate::client::AppClient;
use crate::server::SoloistMcp;

/// Spawns a fake app on `socket` that answers each request via `respond` until the client
/// disconnects, so a test drives the real [`SoloistMcp`] handler through the real IPC
/// transport — exercising dispatch, response projection, and error mapping end to end.
pub(crate) fn spawn_fake_app(
    socket: PathBuf,
    respond: impl Fn(IpcRequest) -> IpcResult + Send + 'static,
) {
    let listener = UnixListener::bind(&socket).expect("bind");
    tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.expect("accept");
        while let Some(request) = read_frame::<_, IpcRequest>(&mut stream)
            .await
            .expect("read request")
        {
            let reply = respond(request);
            write_frame(&mut stream, &reply).await.expect("write reply");
        }
    });
}

/// Every feature group enabled — the full surface, so a test exercises what it is about regardless
/// of the default gating (the gating tests construct specific enablements).
pub(crate) fn all_feature_groups() -> McpToolGroups {
    McpToolGroups {
        scratchpads: true,
        todos: true,
        timers: true,
        key_value: true,
        prompt_templates: true,
    }
}

/// A handler whose single client connection talks to the fake app on `socket`, with every feature
/// group enabled.
pub(crate) fn handler(socket: PathBuf) -> SoloistMcp {
    SoloistMcp::new(Arc::new(AppClient::new(None, socket)), all_feature_groups())
}

/// A handler with the given feature-group enablement, talking to the fake app on `socket`.
pub(crate) fn handler_on(socket: PathBuf, groups: McpToolGroups) -> SoloistMcp {
    SoloistMcp::new(Arc::new(AppClient::new(None, socket)), groups)
}

/// A handler with the given feature-group enablement and no reachable app — for a test that reads
/// only what the handler decides on its own, never opening a connection.
pub(crate) fn handler_with_groups(groups: McpToolGroups) -> SoloistMcp {
    handler_on(PathBuf::from("unused.sock"), groups)
}
