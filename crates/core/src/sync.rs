//! Small synchronization helpers shared across the core.

use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Locks a `Mutex`, recovering the guard if a previous holder panicked. The core
/// forbids `unwrap`/`expect`/`panic`, so lock poisoning is handled, not unwrapped.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Read-locks an `RwLock`, recovering the guard if a previous holder panicked — the poison-safe
/// counterpart to [`lock`] for a shared read.
pub(crate) fn read_lock<T>(rwlock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    rwlock
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Write-locks an `RwLock`, recovering the guard if a previous holder panicked.
pub(crate) fn write_lock<T>(rwlock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    rwlock
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
