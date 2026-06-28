//! Tools settings (global Tools tab): the default external editor and terminal used when opening a
//! project. The default editor can be overridden per project (11a); the resolver layers the project
//! override over this global default in one place, so there is a single source for "which editor".

use serde::{Deserialize, Serialize};

/// The Tools tab document. Each default is the chosen application's launch name (e.g. `code`,
/// `zed`); `None` means "use the system default". The frontend offers the probed installed
/// applications; the core only stores the chosen name.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolDefaults {
    /// Editor used when opening a project, unless a project overrides it.
    pub default_editor: Option<String>,
    /// Terminal used when opening a project from the sidebar.
    pub default_terminal: Option<String>,
}
