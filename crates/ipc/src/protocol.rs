//! The request/reply protocol between an IPC client (the MCP server) and the app.
//!
//! Each [`IpcRequest`] maps to exactly one `Facade` behaviour; the server pairs it with
//! the connection's identity session for scope. Replies reuse the core read-model types
//! (`ProcessView`) so the wire shape can never drift from the domain — except a project
//! is sent as a lean [`ProjectSummary`] (no UI icon blob, which an agent does not need).
//! One file per protocol half: requests in [`request`], replies in [`response`].

mod request;
mod response;

pub use request::IpcRequest;
pub use response::{IpcResponse, IpcResult, PortWaitOutcome, ProjectStatus, ProjectSummary};

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
