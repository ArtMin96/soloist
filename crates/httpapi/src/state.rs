//! The HTTP API's shared state: a handle to the one core [`Facade`].

use std::sync::Arc;

use soloist_core::Facade;

/// What every HTTP handler is given: a clone-cheap handle to the single core façade. The
/// adapter holds nothing else — no business state lives here, every route maps to one
/// façade call.
#[derive(Clone)]
pub struct ApiState {
    facade: Arc<Facade>,
}

impl ApiState {
    /// Wraps the shared façade for the router.
    pub fn new(facade: Arc<Facade>) -> Self {
        Self { facade }
    }

    /// The core façade every read and mutation routes through.
    pub fn facade(&self) -> &Facade {
        &self.facade
    }
}
