import { useCallback, useEffect, useState } from "react";

// Loads a settings document once from the core, then auto-saves changes optimistically: the
// local value updates immediately and the facade-echoed stored value reconciles it (the same
// load-once + optimistic-save shape AppearanceProvider uses, factored out so the overlay-only
// panels — Tools, Agents, Integrations — don't each re-roll it). `load` and `save` must be
// stable references (the module-level api functions); `fallback` is the pre-load placeholder
// the facade's stored value supersedes. The always-applied Appearance and Sidebar documents
// keep their own root providers because surfaces outside the Settings overlay read them.
export function useSettingsResource<T>(
  load: () => Promise<T>,
  save: (next: T) => Promise<T>,
  fallback: T,
): { value: T; update: (next: T) => void } {
  const [value, setValue] = useState<T>(fallback);

  useEffect(() => {
    let cancelled = false;
    load()
      .then((loaded) => {
        if (!cancelled) setValue(loaded);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [load]);

  const update = useCallback(
    (next: T) => {
      setValue(next);
      void save(next)
        .then(setValue)
        .catch(() => {});
    },
    [save],
  );

  return { value, update };
}
