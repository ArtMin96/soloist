//! The git context's second driven port: handing one of a project's files to the desktop, plus
//! the no-op default.
//!
//! Opening a file starts whatever program this machine has registered for it, on a path that came
//! from a repository. That is two things worth being careful about, and they are answered in two
//! places. **Whether** a file may be opened at all is decided here in the core, against the
//! project's trust gate — because starting a program on a repository's contents is the repository
//! running code by proxy, which is exactly what trusting a project authorises. **Where** the path
//! actually leads is the implementation's, because only something holding the filesystem can
//! follow a link: the lexical guard the core applies refuses `..`, an absolute path and an empty
//! one, and it cannot refuse a symbolic link inside the repository that points outside it.

use std::path::Path;

/// Hands one file to whatever the desktop has registered to open it.
///
/// An implementation is **blocking**: it reaches the desktop, so callers reach it from the
/// blocking pool ([`crate::facade::Facade::blocking`]) rather than a runtime worker. It must
/// return promptly — it starts a program rather than waiting for one — and it must leave nothing
/// behind that outlives the program it started.
///
/// The contract it carries alone: **`path` is resolved before anything is opened, and one that
/// resolves outside `root` is [`OpenError::Outside`]**. The core has already refused every path
/// that says so in its own text; what is left is a path that names something inside the
/// repository and leads out of it, which only the filesystem can tell.
pub trait FileOpener: Send + Sync {
    /// Opens the file `path` names inside `root`, relative to it.
    fn open(&self, root: &Path, path: &str) -> Result<(), OpenError>;
}

/// Why a file was not handed to the desktop.
///
/// Machine data only: what a desktop's own tooling printed about a failure does not cross the
/// port, so no behaviour here comes to depend on the wording of a program Soloist does not own.
#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
pub enum OpenError {
    /// The path resolved to somewhere outside the repository. A link inside a repository can point
    /// anywhere on the disk, and following one would hand a program a file the project never held.
    #[error("that path is not inside the repository")]
    Outside,
    /// Nothing on this machine would open it — no registered program, or the one registered could
    /// not be started.
    #[error("nothing on this machine could open that file")]
    Unopenable,
}

/// A [`FileOpener`] that opens nothing — the default until the desktop adapter is wired (headless
/// tools, tests that do not exercise a desktop).
///
/// Unlike the read-only driven ports it does not degrade silently, because there is nothing
/// quieter to degrade to: a file that was not opened has no stand-in, and reporting success would
/// tell a surface a window appeared somewhere when none did.
#[derive(Clone, Copy, Default)]
pub struct NoopFileOpener;

impl FileOpener for NoopFileOpener {
    fn open(&self, _root: &Path, _path: &str) -> Result<(), OpenError> {
        Err(OpenError::Unopenable)
    }
}
