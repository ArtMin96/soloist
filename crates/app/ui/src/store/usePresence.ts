import { useEffect } from "react";
import { setPresence } from "@/api";
import { useWindowFocused } from "@/store/useWindowFocused";

// Reports where the user is to the core: whether the window is on screen, and which process it
// shows. The shell only observes — which surface an alert reaches, and what a sighting clears,
// are decided in the core, so nothing here branches on focus or on a notification level.
export function usePresence(viewing: number | null): void {
  const focused = useWindowFocused();

  useEffect(() => {
    // Until focus is known, the honest report is that nobody is looking: an alert then goes to
    // the desktop, where it waits, rather than to a toast in a window that may not be on screen.
    void setPresence({ focused: focused ?? false, viewing }).catch(() => {});
  }, [focused, viewing]);

  useEffect(
    () => () => {
      // The window can close or hide to the tray while the core keeps running, so the last thing
      // this shell says must be that nobody is looking. Left focused, every alert after it would
      // route to a toast in a window nobody can see.
      void setPresence({ focused: false, viewing: null }).catch(() => {});
    },
    [],
  );
}
