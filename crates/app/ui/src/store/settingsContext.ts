import { createContext, use } from "react";
import type { SettingsTabId } from "@/components/settings/tabs";

/** Opening Settings, on one tab where the caller has a reason to name it and wherever it was last
 *  left when it does not. */
export type OpenSettings = (tab?: SettingsTabId) => void;

// Opening Settings belongs to the app shell, but a setting that switches a feature on is best
// reached from the feature itself — and those surfaces sit several components below the shell,
// behind ones this change has no business rewriting. So the action travels by context the way the
// attention marks do. The default does nothing, so a surface rendered without the provider renders
// rather than throwing.
export const OpenSettingsContext = createContext<OpenSettings>(() => {});

/** Sends the reader to Settings, on `tab` where one is named. */
export function useOpenSettings(): OpenSettings {
  return use(OpenSettingsContext);
}
