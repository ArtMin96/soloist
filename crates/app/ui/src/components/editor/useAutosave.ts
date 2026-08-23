import { useCallback, useEffect, useRef, useState } from "react";
import { useLatestRef } from "@/store/useLatestRef";
import type { SaveOutcome } from "@/store/saveOutcome";

// The idle window after the last edit before an autosave fires. Named so a future settings knob can
// promote it without touching call sites.
const DEFAULT_DELAY_MS = 800;

export interface AutosaveController {
  /** Record a new value and schedule a debounced save — unless paused, when it only marks dirty. */
  push: (value: string) => void;
  /** Save the pending value now, cancelling any scheduled save. A no-op when clean, saving, or paused. */
  flush: () => void;
  /** True while a save is in flight. */
  saving: boolean;
  /** True when the latest value has not been persisted yet. */
  dirty: boolean;
}

export interface UseAutosaveOptions {
  /**
   * Persists the value and resolves with whether it went through. Never rejects: a refusal (a
   * conflict or any other error) is reported through the resolved {@link SaveOutcome}, not a
   * rejected promise — the caller's own state (a conflict banner, an error line) is where the
   * reason lives.
   */
  onSave: (value: string) => Promise<SaveOutcome>;
  /** The debounce window; defaults to {@link DEFAULT_DELAY_MS}. */
  delayMs?: number;
  /**
   * When true, edits are still tracked (dirty stays honest) but never auto-saved, and `flush` is a
   * no-op — the conflict pause. The caller resolves the conflict (reload or re-read) before saves
   * resume, so a stale write is never retried behind the user's back.
   */
  paused?: boolean;
}

/**
 * The autosave lifecycle for one document, owned as a single value so a save can only ever be
 * started from `dirty` — never re-entered while one is already `saving`. That structural guard is
 * the whole point: a second write can never fire against a revision guard the first write hasn't
 * refreshed yet. An edit that lands mid-flight is drained, not raced: it waits as `queued` (only the
 * latest survives — an older queued edit is coalesced away, never the in-flight one) and is sent the
 * moment the current write settles, reading whatever guard that write just advanced.
 */
type AutosaveState =
  | { phase: "clean" }
  | { phase: "dirty"; pending: string }
  | { phase: "saving"; inFlight: string }
  | { phase: "saving"; inFlight: string; queued: string };

/**
 * Feature-agnostic autosave: debounces edits into a single save, flushes on demand (blur, Cmd/Ctrl+S,
 * unmount), tracks dirty/saving, and pauses cleanly on conflict. It owns no document knowledge — the
 * caller decides what a value is and how to persist it — so scratchpads, todos, diagrams, and the
 * template editor all reuse it.
 *
 * The lifecycle lives in `stateRef`, not `useState`, so a keystroke updates it without a re-render;
 * `saving`/`dirty` are the only state, each set only when its derived value actually changes. On a
 * refusal, the refused value (or a newer queued one, if the user kept typing) is restored as
 * `pending` and the hook goes back to `dirty` — but does not re-arm the debounce timer. A write that
 * keeps failing must cost exactly one attempt per user action (CLAUDE.md's ban on unbounded retry);
 * the next attempt is the user's next keystroke or an explicit `flush`.
 */
export function useAutosave({
  onSave,
  delayMs = DEFAULT_DELAY_MS,
  paused = false,
}: UseAutosaveOptions): AutosaveController {
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const stateRef = useRef<AutosaveState>({ phase: "clean" });
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // The scheduled save reads these through refs so a re-render never re-arms the timer, and the
  // timer always sees the current callback and pause flag.
  const onSaveRef = useLatestRef(onSave);
  const pausedRef = useLatestRef(paused);

  // The one place the lifecycle actually moves, so `saving`/`dirty` are always derived from the same
  // value they are read from — never set independently and never allowed to disagree.
  const setPhase = useCallback((next: AutosaveState) => {
    stateRef.current = next;
    setSaving(next.phase === "saving");
    setDirty(next.phase !== "clean");
  }, []);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  // Sends `value`, then applies whatever it resolved to. Named so it can call itself when a write
  // settles onto a queued edit — the drain — without any external re-entry into `commit`.
  const send = useCallback(
    function send(value: string) {
      Promise.resolve(onSaveRef.current(value)).then((outcome) => {
        const current = stateRef.current;
        if (current.phase !== "saving") return; // a superseded call; nothing to reconcile.

        if (outcome === "refused") {
          // Whatever is newest — the queued edit if the user kept typing, else the refused value
          // itself — becomes pending. No timer is armed: that is the ban on auto-retry.
          setPhase({
            phase: "dirty",
            pending: "queued" in current ? current.queued : current.inFlight,
          });
          return;
        }

        if ("queued" in current) {
          const next = current.queued;
          if (pausedRef.current) {
            // A conflict landed mid-flight: surface the queued edit as dirty instead of racing a
            // save the caller is about to pause on.
            setPhase({ phase: "dirty", pending: next });
            return;
          }
          setPhase({ phase: "saving", inFlight: next });
          send(next);
          return;
        }

        setPhase({ phase: "clean" });
      });
    },
    [onSaveRef, pausedRef, setPhase],
  );

  const commit = useCallback(() => {
    clearTimer();
    if (pausedRef.current) return;
    const current = stateRef.current;
    if (current.phase !== "dirty") return; // clean: nothing pending; saving: single-flight — no re-entry.
    setPhase({ phase: "saving", inFlight: current.pending });
    send(current.pending);
  }, [clearTimer, pausedRef, setPhase, send]);

  const push = useCallback(
    (value: string) => {
      clearTimer();
      const current = stateRef.current;
      if (current.phase === "saving") {
        // Queue behind the in-flight write rather than racing it; only the latest queued edit
        // survives, so a burst of keystrokes mid-flight still drains as one follow-up save.
        setPhase({ phase: "saving", inFlight: current.inFlight, queued: value });
        return;
      }
      setPhase({ phase: "dirty", pending: value });
      if (pausedRef.current) return;
      timerRef.current = setTimeout(commit, delayMs);
    },
    [clearTimer, commit, delayMs, pausedRef, setPhase],
  );

  const flush = useCallback(() => commit(), [commit]);

  // Persist any pending edit when the editor unmounts — switching documents or closing the panel
  // must not silently drop the last keystrokes (a paused conflict is the exception: commit no-ops).
  // A value already queued behind an in-flight write needs no special handling here: the write's own
  // continuation drains it once it settles, whether or not this component is still mounted.
  useEffect(() => () => commit(), [commit]);

  return { push, flush, saving, dirty };
}
