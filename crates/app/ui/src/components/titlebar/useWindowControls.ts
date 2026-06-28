import { useCallback, useEffect, useState } from "react";
import {
  closeWindow,
  isWindowMaximized,
  minimizeWindow,
  onWindowResized,
  toggleMaximizeWindow,
} from "@/lib/window";

// Tracks the window's maximized state (kept in sync across maximize / restore / WM
// tiling) and exposes the chrome actions. Presentational components consume this; it
// holds no domain logic — only OS-window state.
export function useWindowControls() {
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    let active = true;
    const sync = () => {
      void isWindowMaximized().then((value) => {
        if (active) setIsMaximized(value);
      });
    };
    sync();
    const unlisten = onWindowResized(sync);
    return () => {
      active = false;
      void unlisten.then((off) => off());
    };
  }, []);

  const minimize = useCallback(() => void minimizeWindow(), []);
  const toggleMaximize = useCallback(() => void toggleMaximizeWindow(), []);
  const close = useCallback(() => void closeWindow(), []);

  return { isMaximized, minimize, toggleMaximize, close };
}
