//! The HTTP API's shared state: a handle to the one core [`Facade`] and the callback that
//! raises the desktop window.

use std::sync::Arc;

use soloist_core::Facade;

/// Raises and focuses the desktop window. The composition root wires this to the Tauri
/// window; it defaults to a no-op so the adapter stays Tauri-free and testable without a
/// window — the window lives in the app, not the core, so `/focus` is the one effect that
/// cannot route through the [`Facade`].
pub type FocusFn = Arc<dyn Fn() + Send + Sync>;

/// What every HTTP handler is given: a clone-cheap handle to the single core façade plus
/// the focus callback. No business state lives here — every route maps to one façade call.
#[derive(Clone)]
pub struct ApiState {
    facade: Arc<Facade>,
    focus: FocusFn,
}

impl ApiState {
    /// Wraps the shared façade for the router, with focus a no-op until the composition
    /// root supplies one via [`with_focus`](Self::with_focus).
    pub fn new(facade: Arc<Facade>) -> Self {
        Self {
            facade,
            focus: Arc::new(|| {}),
        }
    }

    /// Sets the window-raise callback the running app provides.
    pub fn with_focus(mut self, focus: FocusFn) -> Self {
        self.focus = focus;
        self
    }

    /// The core façade every read and mutation routes through.
    pub fn facade(&self) -> &Facade {
        &self.facade
    }

    /// Raises the desktop window (a no-op when no window is wired).
    pub fn focus(&self) {
        (self.focus)();
    }
}
