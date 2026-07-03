import { createContext, use } from "react";
import type { Binding, HotkeyAction, HotkeyBindingView } from "@/domain";

// The live keymap read model plus its mutators. Read by the Hotkeys settings panel (to render and
// edit) and by the global keyboard handler (to dispatch a pressed chord to its action), so a remap
// takes effect everywhere at once. It travels by context from the root provider; the default is an
// empty keymap so a component rendered without the provider (a focused test) still works.
export interface HotkeysState {
  bindings: HotkeyBindingView[];
  remap: (action: HotkeyAction, binding: Binding) => void;
  disable: (action: HotkeyAction) => void;
  reset: (action: HotkeyAction) => void;
  resetAll: () => void;
}

const DEFAULT_STATE: HotkeysState = {
  bindings: [],
  remap: () => {},
  disable: () => {},
  reset: () => {},
  resetAll: () => {},
};

export const HotkeysContext = createContext<HotkeysState>(DEFAULT_STATE);

/** The current keymap and its mutators. */
export function useHotkeys(): HotkeysState {
  return use(HotkeysContext);
}
