import { useCallback } from "react";

// Wraps a palette action so selecting it runs the action and then closes the palette. Returns a
// factory used at the call site: `const run = useCommandAction(onOpenChange);` then
// `onSelect={run(() => doThing())}`. One place owns the run-then-dismiss behaviour every palette
// item shares.
export function useCommandAction(onOpenChange: (open: boolean) => void) {
  return useCallback(
    (action: () => void) => () => {
      action();
      onOpenChange(false);
    },
    [onOpenChange],
  );
}
