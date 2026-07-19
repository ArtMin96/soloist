import { useCallback, useEffect, useRef, useState } from "react";
import { useLatestRef } from "@/store/useLatestRef";

// The idle window after the last edit before an autosave fires. Named so a future settings knob can
// promote it without touching call sites.
const DEFAULT_DELAY_MS = 800;

export interface AutosaveController {
  /** Record a new value and schedule a debounced save — unless paused, when it only marks dirty. */
  push: (value: string) => void;
  /** Save the pending value now, cancelling any scheduled save. A no-op when clean or paused. */
  flush: () => void;
  /** True while a save is in flight. */
  saving: boolean;
  /** True when the latest value has not been persisted yet. */
  dirty: boolean;
}

export interface UseAutosaveOptions {
  /** Persists the value; may reject. Surfacing success/failure is the caller's concern. */
  onSave: (value: string) => void | Promise<void>;
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
 * Feature-agnostic autosave: debounces edits into a single save, flushes on demand (blur, Cmd/Ctrl+S,
 * unmount), tracks dirty/saving, and pauses cleanly on conflict. It owns no document knowledge — the
 * caller decides what a value is and how to persist it — so scratchpads, todos, and the template
 * editor all reuse it. The pending value lives in a ref, not state, so a keystroke schedules a save
 * without a re-render; only the visible `saving`/`dirty` flags are state.
 */
export function useAutosave({
  onSave,
  delayMs = DEFAULT_DELAY_MS,
  paused = false,
}: UseAutosaveOptions): AutosaveController {
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  // The latest unsaved value, or null when everything is persisted.
  const pendingRef = useRef<string | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // The scheduled save reads these through refs so a re-render never re-arms the timer, and the
  // timer always sees the current callback and pause flag.
  const onSaveRef = useLatestRef(onSave);
  const pausedRef = useLatestRef(paused);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const commit = useCallback(() => {
    clearTimer();
    if (pausedRef.current) return;
    const value = pendingRef.current;
    if (value === null) return;
    pendingRef.current = null;
    setDirty(false);
    setSaving(true);
    Promise.resolve(onSaveRef.current(value)).finally(() => setSaving(false));
  }, [clearTimer, onSaveRef, pausedRef]);

  const push = useCallback(
    (value: string) => {
      pendingRef.current = value;
      setDirty(true);
      clearTimer();
      if (pausedRef.current) return;
      timerRef.current = setTimeout(commit, delayMs);
    },
    [clearTimer, commit, delayMs, pausedRef],
  );

  const flush = useCallback(() => commit(), [commit]);

  // Persist any pending edit when the editor unmounts — switching documents or closing the panel
  // must not silently drop the last keystrokes (a paused conflict is the exception: commit no-ops).
  useEffect(() => () => commit(), [commit]);

  return { push, flush, saving, dirty };
}
