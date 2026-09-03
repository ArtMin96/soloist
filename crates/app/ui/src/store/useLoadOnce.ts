import { useEffect } from "react";
import { useLatestRef } from "@/store/useLatestRef";

// Run an async `load` exactly once on mount and hand its result to `onLoaded`, guarded so a
// result that arrives after unmount is dropped. This is the shared shape behind every "load the
// persisted document once, then render it" provider — written once here instead of re-rolling
// the cancellation dance per store. The callbacks are read from the latest render through refs,
// so passing a fresh closure each render never re-triggers the load.
export function useLoadOnce<T>(
  load: () => Promise<T>,
  onLoaded: (value: T) => void,
  onError?: (reason: unknown) => void,
): void {
  const loadRef = useLatestRef(load);
  const onLoadedRef = useLatestRef(onLoaded);
  const onErrorRef = useLatestRef(onError);

  // The refs are stable across renders (`useLatestRef`), so listing them still runs this once per mount.
  useEffect(() => {
    let cancelled = false;
    loadRef.current().then(
      (value) => {
        if (!cancelled) onLoadedRef.current(value);
      },
      (reason) => {
        if (!cancelled) onErrorRef.current?.(reason);
      },
    );
    return () => {
      cancelled = true;
    };
  }, [loadRef, onLoadedRef, onErrorRef]);
}
