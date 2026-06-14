//! Stable, newtype identifiers for domain aggregates.
//!
//! IDs are opaque newtypes, never bare integers, so the compiler rejects mixing a
//! [`ProcessId`] with a [`ProjectId`]. Each id is minted from a process-wide
//! monotonic counter via `next`; values are unique within a single run (the runtime
//! registry is rebuilt on every launch, so cross-run stability is not required here).

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
            pub fn get(self) -> u64 {
                self.0
            }

            /// Reconstructs an id from a raw value that crossed a process boundary
            /// (e.g. an IPC argument). For adapters decoding the wire only — never
            /// to mint a new id (use `next`).
            pub fn from_raw(value: u64) -> Self {
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
    fn distinct_id_types_do_not_share_a_counter_value_meaning() {
        // Both are u64-backed but are distinct types; this only checks Display.
        let p = ProjectId::next();
        assert_eq!(p.to_string(), p.get().to_string());
    }
}
