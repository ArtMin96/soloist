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

    #[test]
    fn from_raw_round_trips_a_wire_value() {
        let id = ProcessId::next();
        assert_eq!(ProcessId::from_raw(id.get()), id);
    }
}
