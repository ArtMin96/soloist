//! The HTTP API's shared state: a handle to the one core [`Facade`], the per-launch auth
//! token every request must present, and the callback that raises the desktop window.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use soloist_core::Facade;

/// The most agent spawns admitted per [`SPAWN_WINDOW`] on `POST /projects/:id/spawn-agent`. A
/// same-user caller is already authenticated by the token, so this is not access control; it caps
/// a runaway loop or script from launching agent processes without bound (defense-in-depth).
const SPAWN_MAX_PER_WINDOW: u32 = 10;

/// The fixed window over which [`SPAWN_MAX_PER_WINDOW`] applies.
const SPAWN_WINDOW: Duration = Duration::from_secs(10);

/// Raises and focuses the desktop window. The composition root wires this to the Tauri
/// window; it defaults to a no-op so the adapter stays Tauri-free and testable without a
/// window — the window lives in the app, not the core, so `/focus` is the one effect that
/// cannot route through the [`Facade`].
pub type FocusFn = Arc<dyn Fn() + Send + Sync>;

/// What every HTTP handler is given: a clone-cheap handle to the single core façade, the
/// per-launch token the auth gate checks, the focus callback, and the agent-spawn rate cap. No
/// business state lives here — every route maps to one façade call; the rate cap is transport-level
/// throttling, like the CORS and `Host` guards.
#[derive(Clone)]
pub struct ApiState {
    facade: Arc<Facade>,
    token: Arc<str>,
    focus: FocusFn,
    spawn_limiter: Arc<SpawnRateLimiter>,
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
            spawn_limiter: Arc::new(SpawnRateLimiter::new(Instant::now())),
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

    /// Whether another agent spawn is admitted now under the per-launch rate cap; a denial maps
    /// to `429` in the handler. Records the admission against the current window.
    pub fn allow_spawn(&self) -> bool {
        self.spawn_limiter.check(Instant::now())
    }
}

/// A fixed-window rate limiter: at most [`SPAWN_MAX_PER_WINDOW`] admissions per [`SPAWN_WINDOW`].
/// The count resets when a request arrives after the window elapses, so a steady, sub-cap rate is
/// never throttled while a burst past the cap is.
struct SpawnRateLimiter {
    window: Mutex<Window>,
}

/// The current window's start and how many admissions it has granted.
struct Window {
    start: Instant,
    count: u32,
}

impl SpawnRateLimiter {
    /// A limiter whose first window opens at `now`.
    fn new(now: Instant) -> Self {
        Self {
            window: Mutex::new(Window {
                start: now,
                count: 0,
            }),
        }
    }

    /// Whether a spawn at `now` is admitted, counting it against the current window and rolling
    /// the window over first when `now` is past it.
    fn check(&self, now: Instant) -> bool {
        let mut window = self
            .window
            .lock()
            .expect("spawn rate limiter mutex poisoned");
        if now.saturating_duration_since(window.start) >= SPAWN_WINDOW {
            window.start = now;
            window.count = 0;
        }
        if window.count >= SPAWN_MAX_PER_WINDOW {
            return false;
        }
        window.count += 1;
        true
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
