import { useCallback, useRef, useState } from "react";

/**
 * A panel's remembered shape: read once at mount, written back on every change, so a user who
 * sized a panel yesterday does not have to size it again today.
 *
 * `sanitize` is the one place a stored value is trusted or corrected — it receives whatever was
 * read (or nothing at all) and returns a shape the layout can actually take, so a value from
 * another build, a hand-edited file, or a smaller screen degrades to something sane instead of
 * producing a panel that swallows the window or vanishes. Define it at module scope: it is a
 * dependency of the returned setter.
 */
export function useStoredLayout<T extends object>(
  key: string,
  sanitize: (stored: Partial<T>) => T,
): [T, (next: Partial<T>) => void] {
  const [layout, setLayout] = useState<T>(() => sanitize(load<T>(key)));
  // The latest shape, so an update can merge and save without doing either inside the state
  // updater — which React may run more than once for one change.
  const latest = useRef(layout);

  const update = useCallback(
    (next: Partial<T>) => {
      const merged = sanitize({ ...latest.current, ...next });
      latest.current = merged;
      try {
        localStorage.setItem(key, JSON.stringify(merged));
      } catch {
        // Storage unavailable; the choice still holds for this session.
      }
      setLayout(merged);
    },
    [key, sanitize],
  );

  return [layout, update];
}

/** Whatever was stored under `key`, or nothing when there is none or it will not parse. */
function load<T extends object>(key: string): Partial<T> {
  try {
    const stored = localStorage.getItem(key);
    return stored === null ? {} : (JSON.parse(stored) as Partial<T>);
  } catch {
    return {};
  }
}

/** Keeps a remembered size inside the bounds a panel can actually take. */
export function clampSize(size: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, Math.round(size)));
}
