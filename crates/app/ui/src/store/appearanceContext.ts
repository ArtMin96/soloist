import { createContext, use } from "react";
import type { Appearance } from "@/domain";
import { DEFAULT_APPEARANCE, resolveDark, systemPrefersDark } from "@/lib/appearance";

// The live appearance read model: the document, the dark/light it resolves to (theme against
// the OS preference), and the auto-saving setter. Read at the leaves that restyle — the
// Appearance panel and the terminal — so it travels by context. The default is the documented
// defaults so a component rendered without the provider (a focused test) still works.
export interface AppearanceState {
  appearance: Appearance;
  dark: boolean;
  setAppearance: (next: Appearance) => void;
}

const DEFAULT_STATE: AppearanceState = {
  appearance: DEFAULT_APPEARANCE,
  dark: resolveDark(DEFAULT_APPEARANCE.theme, systemPrefersDark()),
  setAppearance: () => {},
};

export const AppearanceContext = createContext<AppearanceState>(DEFAULT_STATE);

/** The current appearance, the resolved dark/light, and the auto-saving setter. */
export function useAppearance(): AppearanceState {
  return use(AppearanceContext);
}
