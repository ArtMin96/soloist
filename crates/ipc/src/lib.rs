//! Local IPC transport over a Unix domain socket between the app and the MCP server,
//! plus the request/reply message types shared across the Soloist binaries.
//!
//! The app hosts the server side (routing each request to one `Facade` method); the MCP
//! server is a stateless client. Messages are length-prefixed JSON frames ([`frame`]);
//! the protocol ([`protocol`]) reuses the core domain types so ids and read-model rows
//! stay single-source. The socket location ([`paths`]) is resolved the same way by every
//! binary, so the client finds the server without being told where it is.

mod frame;
mod paths;
mod protocol;

pub use frame::{read_frame, write_frame, FrameError, MAX_FRAME};
pub use paths::{data_dir, ensure_data_dir, ensure_socket_path, socket_path, DataDirError};
pub use protocol::{
    IpcError, IpcRequest, IpcResponse, IpcResult, PortWaitOutcome, ProjectStatus, ProjectSummary,
};
