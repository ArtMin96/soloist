//! Stable, newtype identifiers for domain aggregates.
//!
//! IDs are opaque newtypes, never bare integers, so the compiler rejects mixing a
//! [`ProcessId`] with a [`ProjectId`]. A [`ProcessId`] is minted per run from a
//! process-wide monotonic counter via `next`; the runtime process registry is
//! rebuilt on every launch, so its values need only be unique within a run. A
//! [`ProjectId`], by contrast, is **durable**: the store assigns it from a project's
//! canonical root and reconstructs it via `from_raw` on later runs, so trust keyed
//! by project survives restarts.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

macro_rules! id_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Serialize, Deserialize)]
        pub struct $name(u64);

        impl $name {
            /// Mints the next unique id from a process-wide monotonic counter.
            pub fn next() -> Self {
                static COUNTER: AtomicU64 = AtomicU64::new(1);
                Self(COUNTER.fetch_add(1, Ordering::Relaxed))
            }

            /// The raw underlying value (for wire encoding and display only).
            pub const fn get(self) -> u64 {
                self.0
            }

            /// Reconstructs an id from a raw value that crossed a boundary — an IPC
            /// argument, or a durable id the store assigned (see [`ProjectId`]).
            /// Used by adapters decoding the wire and by the store; never to mint a
            /// fresh runtime id (use `next`).
            pub const fn from_raw(value: u64) -> Self {
                Self(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

id_newtype!(
    /// Identifies a project (a filesystem workspace root).
    ProjectId
);
id_newtype!(
    /// Identifies a managed process within a run.
    ProcessId
);
id_newtype!(
    /// Identifies one MCP client session — one connection to the IPC server — within a
    /// run. Minted per connection; the bound process and selected project that scope a
    /// session's tool calls are keyed by it.
    SessionId
);
id_newtype!(
    /// Identifies a coordination timer (context C6) within a run. The durable store assigns
    /// it on creation and reconstructs it via [`from_raw`](TimerId::from_raw) when reading a
    /// row back. Like a lease, a timer is process-owned and per-run, so a value is meaningful
    /// only within the run that created it (launch reconciliation clears the table).
    TimerId
);
id_newtype!(
    /// Identifies a coordination scratchpad (context C6) — a durable, project-scoped shared
    /// document. The store assigns it on creation and reconstructs it via
    /// [`from_raw`](ScratchpadId::from_raw) when reading a row back. Unlike a timer or lease, a
    /// scratchpad is **not** process-owned and is **durable**: it survives an app restart, so its
    /// id is stable across runs. The mutable [`name`](crate::coordination::ScratchpadView::name)
    /// is the handle callers address; this id is the stable identity a rename does not change.
    ScratchpadId
);
id_newtype!(
    /// Identifies a prompt template (context C6) — a durable reusable prompt, global or
    /// project-scoped. The store assigns it on creation and reconstructs it via
    /// [`from_raw`](PromptTemplateId::from_raw) when reading a row back. Like a scratchpad, a
    /// template is durable content addressed by its per-scope-unique name; this id is the
    /// stable identity behind that handle.
    PromptTemplateId
);
id_newtype!(
    /// Identifies a coordination todo (context C6) — a durable, project-scoped shared work item.
    /// The store assigns it on creation and reconstructs it via [`from_raw`](TodoId::from_raw) when
    /// reading a row back. Like a scratchpad and unlike a timer or lease, a todo is **durable**: it
    /// survives an app restart, so its id is stable across runs and a sibling todo can name it as a
    /// blocker. A todo's *lock* is process-owned and per-run (cleared on launch), but the todo
    /// itself is content that persists.
    TodoId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_monotonic_and_unique() {
        let a = ProcessId::next();
        let b = ProcessId::next();
        assert_ne!(a, b);
        assert!(b.get() > a.get());
    }

    #[test]
    fn display_matches_the_raw_value() {
        let id = ProjectId::next();
        assert_eq!(id.to_string(), id.get().to_string());
    }
}
