//! What an exchange with a remote is described by: which exchange, whether a person may be asked
//! for a credential, the signal that stops it, and where it says what it is doing.
//!
//! These travel together across the port because they are one decision each about the same thing —
//! an operation that reaches a machine Soloist has no say over, may need something only a person
//! has, may take longer than anybody is willing to wait, and is worth hearing from while it does.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Which exchange with a remote to make. A closed set: each one is a different bargain with a
/// machine that may not answer, so every implementation states what it does for each.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SyncOp {
    /// Bring the remote's commits into the repository without touching the working tree.
    Fetch,
    /// Bring them in and reconcile them with what is checked out, however the user's own
    /// configuration says to — which is why no strategy is named here.
    Pull,
    /// Hand the branch's commits to the upstream it already tracks.
    Push,
    /// The same for a branch that tracks nothing yet: hand it to the remote and record it as the
    /// branch's upstream from then on.
    Publish,
}

/// Whether the exchange may stop and ask a person for a credential.
///
/// A closed set rather than a flag, and decided by *which* façade the caller holds rather than
/// passed down by whoever felt like it: the local user clicked something and can answer, so a
/// prompt is correct for them; a session-scoped caller is an agent, and a prompt there is a window
/// opening on a desktop nobody asked to look at, in front of a request nobody is waiting at.
///
/// It is a permission, never a requirement: where a credential is already arranged — an agent, an
/// unlocked keyring, a helper holding a token — nothing is asked either way and both answers behave
/// identically.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prompting {
    /// A person is at the window that asked for this, so let version control ask them.
    Allowed,
    /// Nobody is watching, so a credential that needs a person is a failure rather than a wait.
    Denied,
}

/// A one-way signal that the operation carrying it should stop.
///
/// Cheap to clone and shared by both sides: whoever runs the operation is handed one and looks at it
/// as it waits, and whoever might change their mind holds the same one and sets it. Nothing is ever
/// unset — an operation that was told to stop stays told, and the next operation gets a new signal
/// of its own.
#[derive(Clone, Default)]
pub struct Stop(Arc<AtomicBool>);

impl Stop {
    /// Asks whatever is carrying this to stop. Idempotent, and instant: it takes no lock the
    /// operation being stopped could be holding, which is what lets it be called while that
    /// operation runs.
    pub fn stop(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether stopping has been asked for.
    pub fn stopped(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Where an operation says what it is doing while it is still doing it, for a caller that asked to
/// be told.
///
/// **Asked to be told** is the whole of the condition. An operation nobody is listening to reports
/// nothing, does no extra work to have nothing to report, and behaves exactly as it did before any
/// of this existed — which is why this is a value carried alongside the operation rather than a
/// setting somewhere, and why every surface that does not want it holds [`Progress::unwatched`].
///
/// What crosses here is whatever the operation said about itself, verbatim and already coalesced to
/// the latest remark. Nothing decides anything from it: it is prose, from a program Soloist does not
/// own, on its way to a person or an agent who wants to know the operation is alive and roughly
/// where it has got to.
#[derive(Clone, Default)]
pub struct Progress(Option<Observer>);

/// Whoever is listening to an operation, handed each remark as it is made. Shared rather than owned,
/// because the operation runs on a thread of the pool's choosing while whoever asked waits elsewhere.
pub type Observer = Arc<dyn Fn(&str) + Send + Sync>;

impl Progress {
    /// Nobody is listening. The default, and what every surface that never asked for progress holds.
    pub fn unwatched() -> Self {
        Self(None)
    }

    /// Somebody is listening, and this is where to say it.
    pub fn watched_by(observer: Observer) -> Self {
        Self(Some(observer))
    }

    /// Whether anybody is listening — asked by an adapter deciding whether to make the underlying
    /// tool produce progress at all, since a tool asked to report to nobody is pure cost.
    pub fn is_watched(&self) -> bool {
        self.0.is_some()
    }

    /// Says `remark` to whoever is listening. Costs nothing when nobody is.
    pub fn report(&self, remark: &str) {
        if let Some(observer) = &self.0 {
            observer(remark);
        }
    }
}

impl fmt::Debug for Progress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Progress").field(&self.is_watched()).finish()
    }
}
