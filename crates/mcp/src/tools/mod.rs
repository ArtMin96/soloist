//! The MCP tool surface, split by logical category. Each submodule is one
//! `#[tool_router(router = …)]` block of thin handlers over [`crate::server::SoloistMcp`];
//! [`crate::server::SoloistMcp::new`] composes their routers into the one served router via
//! [`rmcp::handler::server::tool::ToolRouter`]'s `Add`. A new tool lands in its category file
//! and its sub-router — never a single flat block. Shared reply helpers live in [`reply`].

pub(crate) mod agent;
pub(crate) mod bulk;
pub(crate) mod identity;
pub(crate) mod lock;
pub(crate) mod output;
pub(crate) mod process;
pub(crate) mod project;
pub(crate) mod reply;
pub(crate) mod scratchpad;
pub(crate) mod services;
pub(crate) mod timer;
