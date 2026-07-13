import { useEffect } from "react";
import { onResync } from "@/api";

// A self-healing backstop for the snapshot-then-deltas read models. It re-runs `refresh` whenever
// the backend signals its delta stream fell behind (`onResync`) and whenever the window regains
// focus. Either path recovers a store that missed a delta — a lost delta is otherwise permanent, so
// without this a stale or missing row stays wrong until an unrelated event happens to touch it. The
// focus path also covers a subscription that failed to attach, since it never depends on the delta
// stream. `refresh` must be stable (a `useCallback`) so this attaches once per mount.
export function useReconcile(refresh: () => void): void {
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    onResync(() => refresh())
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(() => {});
    const onFocus = () => refresh();
    window.addEventListener("focus", onFocus);
    return () => {
      cancelled = true;
      unlisten?.();
      window.removeEventListener("focus", onFocus);
    };
  }, [refresh]);
}
