//! The HTTP API's shared state: a handle to the one core [`Facade`], the per-launch auth
//! token every request must present, and the callback that raises the desktop window.

use std::sync::Arc;

use soloist_core::Facade;

/// Raises and focuses the desktop window. The composition root wires this to the Tauri
/// window; it defaults to a no-op so the adapter stays Tauri-free and testable without a
/// window — the window lives in the app, not the core, so `/focus` is the one effect that
/// cannot route through the [`Facade`].
pub type FocusFn = Arc<dyn Fn() + Send + Sync>;

/// What every HTTP handler is given: a clone-cheap handle to the single core façade, the
/// per-launch token the auth gate checks, plus the focus callback. No business state lives
/// here — every route maps to one façade call.
#[derive(Clone)]
pub struct ApiState {
    facade: Arc<Facade>,
    token: Arc<str>,
    focus: FocusFn,
}

impl ApiState {
    /// Wraps the shared façade and the launch's auth `token` for the router, with focus a
    /// no-op until the composition root supplies one via [`with_focus`](Self::with_focus).
    /// The token is injected (the composition root mints it, a test passes a known value),
    /// so the adapter stays a pure function of its inputs.
    pub fn new(facade: Arc<Facade>, token: impl Into<Arc<str>>) -> Self {
        Self {
            facade,
            token: token.into(),
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

    /// Runs a synchronous façade `op` on tokio's blocking pool and awaits it, so a durable-store
    /// `fsync` (slow or full disk) can never park a runtime worker — no blocking call runs on the
    /// runtime. The cloned `Arc` keeps the façade alive for the task. Every synchronous read and
    /// mutation routes through here; the handful that await the core stay on the runtime.
    pub async fn blocking<T, F>(&self, op: F) -> T
    where
        F: FnOnce(&Facade) -> T + Send + 'static,
        T: Send + 'static,
    {
        let facade = Arc::clone(&self.facade);
        tokio::task::spawn_blocking(move || op(&facade))
            .await
            .expect("a façade call must not panic on the blocking pool")
    }

    /// The per-launch token every request must present, matched by the auth gate.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Raises the desktop window (a no-op when no window is wired).
    pub fn focus(&self) {
        (self.focus)();
    }
}
