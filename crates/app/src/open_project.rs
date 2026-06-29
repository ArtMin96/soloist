//! Opening a project from a filesystem path Soloist was handed. The `solo.yml` file
//! association passes the chosen file here, and a second launch forwards its arguments
//! here through the single-instance plugin. Both route to the one core `load_project`
//! command the UI's folder picker already uses — the app translates an OS input into a
//! core command and holds no project logic of its own.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use soloist_core::Facade;
use tauri::{AppHandle, Manager};

/// Resolves a handed-in path to the project root to open: a directory opens itself; a
/// file (the associated `solo.yml`, or any file named on the command line) opens its
/// containing directory; a path that does not exist opens nothing.
fn project_root_for(path: &Path) -> Option<PathBuf> {
    if path.is_dir() {
        Some(path.to_path_buf())
    } else if path.is_file() {
        path.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

/// Loads the project resolved from `path` through the one core command, then reveals the
/// window so the opened project is visible. A path that resolves to nothing, or a load
/// that fails, is logged and otherwise ignored — opening a project must never take the
/// app down.
pub fn open(app: &AppHandle, path: &Path) {
    let Some(root) = project_root_for(path) else {
        return;
    };
    if let Err(err) = app.state::<Arc<Facade>>().load_project(&root) {
        eprintln!(
            "soloist: could not open project at {} ({err})",
            root.display()
        );
        return;
    }
    reveal(app);
}

/// Opens the first existing path among `args`, skipping `args[0]` (the binary itself).
/// Used for the launching process's own arguments; the single-instance plugin handles a
/// second launch's arguments.
pub fn open_from_args<I: IntoIterator<Item = String>>(app: &AppHandle, args: I) {
    if let Some(path) = args
        .into_iter()
        .skip(1)
        .map(PathBuf::from)
        .find(|candidate| candidate.exists())
    {
        open(app, &path);
    }
}

/// Brings the main window to the foreground.
pub fn reveal(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
#[path = "open_project_tests.rs"]
mod tests;
