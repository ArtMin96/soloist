//! Resolving the user's login shell, shared by the OS adapters that run a command through it.

use nix::unistd::{Uid, User};

/// Fallback shell when neither `$SHELL` nor the passwd entry yields one.
const FALLBACK_SHELL: &str = "/bin/sh";

/// Resolves the user's login shell: `$SHELL`, then the passwd-entry shell, then `/bin/sh`. The
/// same resolution the PTY spawner uses, so an adapter runs commands under the user's real shell —
/// a desktop launcher does not always export `$SHELL`, so the passwd fallback keeps commands
/// running under it.
pub(crate) fn login_shell() -> String {
    if let Ok(shell) = std::env::var("SHELL") {
        if !shell.is_empty() {
            return shell;
        }
    }
    if let Ok(Some(user)) = User::from_uid(Uid::current()) {
        if let Some(shell) = user.shell.to_str() {
            if !shell.is_empty() {
                return shell.to_owned();
            }
        }
    }
    FALLBACK_SHELL.to_string()
}
