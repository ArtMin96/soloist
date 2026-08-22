//! Which shell an external run goes through.

use nix::unistd::{Uid, User};

/// The shell used when neither `$SHELL` nor the passwd entry names one.
const FALLBACK_SHELL: &str = "/bin/sh";

/// Resolves the user's login shell: `$SHELL`, then the passwd-entry shell, then `/bin/sh`.
///
/// A desktop launcher does not always export `$SHELL`, so the passwd fallback keeps commands
/// running under the user's real shell rather than a bare `/bin/sh`.
///
/// It lives beside the containment, and not in whichever adapter reaches for it, because launching
/// a managed process, capturing the environment that process will see, and detecting whether a CLI
/// is installed are three questions that only agree with each other while they are asking the same
/// shell — and so resolving against the same `PATH`. That is a promise two copies of this could
/// make and only one can keep.
pub fn login_shell() -> String {
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
