import { useCallback, useState, type ReactNode } from "react";
import {
  disableHotkey,
  hotkeys as readHotkeys,
  remapHotkey,
  resetAllHotkeys,
  resetHotkey,
} from "@/api";
import { HotkeysContext } from "@/store/hotkeysContext";
import { persistThenReconcile } from "@/store/persist";
import { useLoadOnce } from "@/store/useLoadOnce";
import type { Binding, HotkeyAction, HotkeyBindingView } from "@/domain";

// Loads the keymap once and provides it to the whole app, so the global keyboard handler and the
// Hotkeys settings panel share one source — a remap, disable, or reset takes effect live. Each
// mutator persists through its facade method and reconciles from the returned keymap (falling back
// to the stored keymap if the write fails). Mounted at the app root alongside the other settings
// providers.
export function HotkeysProvider({ children }: { children: ReactNode }) {
  const [bindings, setBindings] = useState<HotkeyBindingView[]>([]);

  useLoadOnce(readHotkeys, setBindings);

  const applyWrite = useCallback((write: Promise<HotkeyBindingView[]>) => {
    persistThenReconcile(write, readHotkeys, setBindings);
  }, []);
  const remap = useCallback(
    (action: HotkeyAction, binding: Binding) => applyWrite(remapHotkey(action, binding)),
    [applyWrite],
  );
  const disable = useCallback(
    (action: HotkeyAction) => applyWrite(disableHotkey(action)),
    [applyWrite],
  );
  const reset = useCallback(
    (action: HotkeyAction) => applyWrite(resetHotkey(action)),
    [applyWrite],
  );
  const resetAll = useCallback(() => applyWrite(resetAllHotkeys()), [applyWrite]);

  return (
    <HotkeysContext value={{ bindings, remap, disable, reset, resetAll }}>
      {children}
    </HotkeysContext>
  );
}
