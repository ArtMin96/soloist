import { useCallback, useState } from "react";
import { persistThenReconcile } from "@/store/persist";
import { useLoadOnce } from "@/store/useLoadOnce";

// Loads a settings document once from the core, then auto-saves changes optimistically: the
// local value updates immediately and the facade-echoed stored value reconciles it (the same
// load-once + optimistic-save shape AppearanceProvider uses, factored out so the overlay-only
// panels — Tools, Agents, Integrations — don't each re-roll it). `fallback` is the pre-load
// placeholder the facade's stored value supersedes. The always-applied Appearance and Sidebar
// documents keep their own root providers because surfaces outside the Settings overlay read them.
export function useSettingsResource<T>(
  load: () => Promise<T>,
  save: (next: T) => Promise<T>,
  fallback: T,
): { value: T; update: (next: T) => void } {
  const [value, setValue] = useState<T>(fallback);

  useLoadOnce(load, setValue);

  const update = useCallback(
    (next: T) => {
      setValue(next);
      persistThenReconcile(save(next), load, setValue);
    },
    [save, load],
  );

  return { value, update };
}
