export type ToastId = string | number;

// The countdown behind an in-app toast, kept out of the toast library on purpose.
//
// A toast pauses while the pointer is over the stack and then starts again **from its full
// duration** — you moved the mouse away, so you get the whole time to read it, not the 100 ms that
// happened to be left when you arrived. sonner's own timer resumes from what was left instead, and
// exposes no way to re-arm it, so every toast is handed to it with no expiry of its own and the
// clock lives here. Holding each toast's full duration rather than a deadline is what makes
// restarting from full a single re-arm.
//
// Bounded by the toasts on screen: a countdown is dropped when it fires and when its toast is
// dismissed, and a toast with no countdown (one that persists until acted on) is never held at all.
export interface ToastDismissal {
  /** Dismiss `id` in `ms`, replacing any countdown already running for it. */
  schedule(id: ToastId, ms: number): void;
  /** Forget `id` — it is gone, by its own countdown or by the user. */
  forget(id: ToastId): void;
  /** Freeze every countdown: the pointer is over the stack. */
  pause(): void;
  /** Start every frozen countdown again, each from its full duration. */
  resume(): void;
  /** Drop every countdown without dismissing anything — the surface is going away. */
  cancel(): void;
}

export function createToastDismissal(dismiss: (id: ToastId) => void): ToastDismissal {
  const durations = new Map<ToastId, number>();
  const timers = new Map<ToastId, ReturnType<typeof setTimeout>>();
  let paused = false;

  const arm = (id: ToastId, ms: number) => {
    timers.set(
      id,
      setTimeout(() => {
        timers.delete(id);
        durations.delete(id);
        dismiss(id);
      }, ms),
    );
  };

  const disarm = (id: ToastId) => {
    const timer = timers.get(id);
    if (timer === undefined) return;
    clearTimeout(timer);
    timers.delete(id);
  };

  const disarmAll = () => {
    timers.forEach((timer) => clearTimeout(timer));
    timers.clear();
  };

  return {
    schedule(id, ms) {
      disarm(id);
      durations.set(id, ms);
      if (!paused) arm(id, ms);
    },
    forget(id) {
      disarm(id);
      durations.delete(id);
    },
    pause() {
      paused = true;
      disarmAll();
    },
    resume() {
      paused = false;
      durations.forEach((ms, id) => arm(id, ms));
    },
    cancel() {
      paused = false;
      disarmAll();
      durations.clear();
    },
  };
}
