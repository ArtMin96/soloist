import { clampSize, useStoredLayout } from "@/store/useStoredLayout";

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

function sanitize(stored: Partial<RailLayout>): RailLayout {
  return {
    width: clampSize(
      typeof stored.width === "number" ? stored.width : RAIL_DEFAULT_WIDTH,
      RAIL_MIN_WIDTH,
      RAIL_MAX_WIDTH,
    ),
    collapsed: stored.collapsed === true,
  };
}

/**
 * The rail's remembered shape: how wide it is and whether it is closed. Both survive a restart,
 * the way a docked panel is expected to — a user who narrowed the rail yesterday should not
 * have to narrow it again today.
 */
export function useRailLayout(): [RailLayout, (next: Partial<RailLayout>) => void] {
  return useStoredLayout(STORAGE_KEY, sanitize);
}
