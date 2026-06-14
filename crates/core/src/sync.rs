//! Small synchronization helpers shared across the core.

use std::sync::{Mutex, MutexGuard};

/// Locks a `Mutex`, recovering the guard if a previous holder panicked. The core
/// forbids `unwrap`/`expect`/`panic`, so lock poisoning is handled, not unwrapped.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
