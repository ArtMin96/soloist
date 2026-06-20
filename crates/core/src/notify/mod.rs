//! Notifications (context C7): desktop toasts on the events that need the user's attention.
//!
//! This bounded context owns its own driven port ([`Notifier`]) and the [`NotificationReactor`]
//! that drives it from the event bus — a crash or an exhausted auto-restart becomes a desktop
//! toast. A global on/off gates every notification. Optional by design: with the default
//! [`NoopNotifier`] the reactor runs but shows nothing, so the core never depends on a
//! notification backend.
//!
//! Note: this module is named for the *capability* (notifications); it is unrelated to the
//! `notify` filesystem-watch crate, which the core never imports (it lives only in the
//! `crates/sys` file-watch adapter — the dependency-direction guard enforces the separation).

mod notifier;
mod reactor;

pub use notifier::{NoopNotifier, Notification, Notifier};
pub use reactor::NotificationReactor;
