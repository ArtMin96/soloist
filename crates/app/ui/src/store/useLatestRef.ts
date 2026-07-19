import { useLayoutEffect, useRef, type RefObject } from "react";

/**
 * Tracks the newest `value` in a ref so long-lived callbacks — event listeners, timers, and editor
 * plugins installed once per mount — can read it without being re-created on every change.
 *
 * The write lands in a layout effect rather than the render body: React may start a render and then
 * discard it, and a mutation from a pass that never committed would leave the ref pointing at props
 * the mounted UI never saw. Layout timing (not passive) closes the commit-to-paint window too, so a
 * listener firing immediately after commit already observes the current value.
 *
 * Only read the result from code that runs after commit; during render it is a pass behind.
 */
export function useLatestRef<T>(value: T): RefObject<T> {
  const ref = useRef(value);
  useLayoutEffect(() => {
    ref.current = value;
  }, [value]);
  return ref;
}
