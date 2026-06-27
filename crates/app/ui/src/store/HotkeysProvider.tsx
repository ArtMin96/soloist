import { useCallback, useState, type ReactNode } from "react";
import {
  disableHotkey,
  hotkeys as readHotkeys,
  remapHotkey,
  resetAllHotkeys,
  resetHotkey,
} from "@/api";
import { HotkeysContext } from "@/store/hotkeysContext";
import { useLoadOnce } from "@/store/useLoadOnce";
import type { Binding, HotkeyAction, HotkeyBindingView } from "@/domain";

// Loads the keymap once and provides it to the whole app, so the global keyboard handler and the
// Hotkeys settings panel share one source — a remap, disable, or reset takes effect live. Each
// mutator persists through its facade method and reconciles from the returned keymap. Mounted at
// the app root alongside the other settings providers.
export function HotkeysProvider({ children }: { children: ReactNode }) {
  const [bindings, setBindings] = useState<HotkeyBindingView[]>([]);

  useLoadOnce(readHotkeys, setBindings);

  const remap = useCallback((action: HotkeyAction, binding: Binding) => {
    void remapHotkey(action, binding)
      .then(setBindings)
      .catch(() => {});
  }, []);
  const disable = useCallback((action: HotkeyAction) => {
    void disableHotkey(action)
      .then(setBindings)
      .catch(() => {});
  }, []);
  const reset = useCallback((action: HotkeyAction) => {
    void resetHotkey(action)
      .then(setBindings)
      .catch(() => {});
  }, []);
  const resetAll = useCallback(() => {
    void resetAllHotkeys()
      .then(setBindings)
      .catch(() => {});
  }, []);

  return (
    <HotkeysContext value={{ bindings, remap, disable, reset, resetAll }}>
      {children}
    </HotkeysContext>
  );
}
