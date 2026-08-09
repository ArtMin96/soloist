//! The desktop adapter for opening a file: implements the core's [`FileOpener`] over the Tauri
//! opener plugin.
//!
//! The core has already decided *whether* — the project is trusted and the path says it is inside
//! the repository. What is settled here is *where the path actually leads*, because that is the
//! one guard a pure core cannot make: a symbolic link inside a repository can point anywhere on
//! this disk, and only something holding the filesystem can follow one. Both sides are resolved
//! and the file must still be under the project, or nothing is opened.
//!
//! **The webview never reaches the plugin.** Its `open_path` command is not in the app's
//! capability, so the only route to the desktop is this adapter, behind a core method behind the
//! trust gate. That is deliberately stronger than the alternative: the plugin's own scope is a
//! static list of path patterns written at build time, and a project's root is a folder the user
//! picks while the app is running, so no pattern narrow enough to mean "this project" could ever
//! be written down.

use std::path::{Path, PathBuf};

use soloist_core::{FileOpener, OpenError};
use tauri::{AppHandle, Runtime};
use tauri_plugin_opener::OpenerExt;

/// Opens files through the desktop's own registered programs. Holds an [`AppHandle`], so it is
/// constructed in the composition root once the app exists.
pub struct DesktopFileOpener<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> DesktopFileOpener<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> FileOpener for DesktopFileOpener<R> {
    fn open(&self, root: &Path, path: &str) -> Result<(), OpenError> {
        let file = contained(root, path).ok_or(OpenError::Outside)?;
        self.app
            .opener()
            .open_path(file.to_string_lossy(), None::<&str>)
            .map_err(|_| OpenError::Unopenable)
    }
}

/// Where `path` leads from `root` when it leads anywhere inside it, `None` otherwise.
///
/// Both sides are resolved before they are compared, so what is checked is where the file really
/// is rather than what it was called: a link inside the repository pointing at a key in the user's
/// home directory resolves to that key, which is not under the project, and is refused. Resolving
/// the root too is what keeps the comparison honest on a machine whose project path runs through a
/// link of its own — `/tmp` being one on many of them.
///
/// A path that resolves to nothing at all — a file removed since the listing was read — is refused
/// as well: there is nothing there to open, and no way to know where it would have led.
fn contained(root: &Path, path: &str) -> Option<PathBuf> {
    let root = root.canonicalize().ok()?;
    let file = root.join(path).canonicalize().ok()?;
    file.starts_with(&root).then_some(file)
}

#[cfg(test)]
#[path = "opener_tests.rs"]
mod tests;
