//! Calling the synchronous façade from async without parking a runtime worker.

use std::sync::Arc;

use super::Facade;
use crate::supervision::run_blocking;

impl Facade {
    /// Runs a synchronous façade `op` on tokio's blocking pool and awaits its result, so a
    /// durable-store write's `fsync` (a slow or full disk) can never park a runtime worker.
    ///
    /// Every adapter that reaches the synchronous façade from async routes through here, so the
    /// rule holds in one place rather than once per surface. The façade methods that are
    /// themselves `async` already await the core and must not be wrapped in this.
    ///
    /// Must be called from within a `tokio` runtime.
    ///
    /// # Panics
    ///
    /// Resumes a panic raised by `op` in the calling task, so a bug surfaces at the call site
    /// carrying its original payload.
    pub async fn blocking<T, F>(self: &Arc<Self>, op: F) -> T
    where
        F: FnOnce(&Facade) -> T + Send + 'static,
        T: Send + 'static,
    {
        let facade = Arc::clone(self);
        run_blocking(move || op(&facade)).await
    }
}

#[cfg(test)]
#[path = "blocking_tests.rs"]
mod tests;
