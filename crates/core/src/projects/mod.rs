//! The projects bounded context (C1): the workspace roots Soloist manages.
//!
//! A project is a filesystem folder. Its durable identity is its **canonical** absolute
//! path, so the same workspace is one project however its path was written (symlinks,
//! `.`/`..`, trailing slash). The durable [`crate::ids::ProjectId`] is assigned by the
//! store and is stable across runs — which is what lets trust persist.
//!
//! This module is the single home for project behaviour: the durable [`registry`]
//! (identity + metadata), the display [`view`] read-model the UI groups by (which
//! resolves a project's name and icon together, into values the UI renders directly),
//! and the [`service`] lifecycle (opening and restoring). Consumers — the
//! [`Facade`](crate::facade::Facade), adapters, the UI — call into here; they do not
//! decide how a project is identified, named, shown, or opened.

mod registry;
mod service;
mod view;

pub use registry::{ProjectError, Projects};
pub use service::{LoadProjectError, ProjectLoad, ProjectService};
pub use view::ProjectView;
