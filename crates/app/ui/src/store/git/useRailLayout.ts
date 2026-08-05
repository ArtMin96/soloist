import { useCallback, useRef, useState } from "react";

/** Where the rail's shape is remembered between launches. */
const STORAGE_KEY = "soloist.git.rail";

/** How wide the rail opens the first time, and the bounds a drag may take it between. */
export const RAIL_DEFAULT_WIDTH = 280;
export const RAIL_MIN_WIDTH = 200;
export const RAIL_MAX_WIDTH = 560;

/** How far one arrow-key press moves the divider. */
export const RAIL_RESIZE_STEP = 16;

export interface RailLayout {
  width: number;
  collapsed: boolean;
}

const DEFAULT: RailLayout = { width: RAIL_DEFAULT_WIDTH, collapsed: false };

/** Keeps a remembered width inside the bounds, so a stored value from another build or a
 *  hand-edited file cannot produce a rail that swallows the window or vanishes. */
export function clampWidth(width: number): number {
  return Math.min(RAIL_MAX_WIDTH, Math.max(RAIL_MIN_WIDTH, Math.round(width)));
}

function load(): RailLayout {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === null) return DEFAULT;
    const parsed = JSON.parse(stored) as Partial<RailLayout>;
    return {
      width: clampWidth(typeof parsed.width === "number" ? parsed.width : RAIL_DEFAULT_WIDTH),
      collapsed: parsed.collapsed === true,
    };
  } catch {
    return DEFAULT;
  }
}

/**
 * The rail's remembered shape: how wide it is and whether it is closed. Both survive a restart,
 * the way a docked panel is expected to — a user who narrowed the rail yesterday should not
 * have to narrow it again today.
 */
export function useRailLayout(): [RailLayout, (next: Partial<RailLayout>) => void] {
  const [layout, setLayout] = useState<RailLayout>(load);
  // The latest shape, so an update can merge and save without doing either inside the state
  // updater — which React may run more than once for one change.
  const latest = useRef(layout);
  const update = useCallback((next: Partial<RailLayout>) => {
    const merged = {
      ...latest.current,
      ...next,
      width: clampWidth(next.width ?? latest.current.width),
    };
    latest.current = merged;
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(merged));
    } catch {
      // Storage unavailable; the choice still holds for this session.
    }
    setLayout(merged);
  }, []);
  return [layout, update];
}
