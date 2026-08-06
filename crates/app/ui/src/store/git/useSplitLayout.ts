import { clampSize, useStoredLayout } from "@/store/useStoredLayout";

/** Where the split's shape is remembered between launches. */
const STORAGE_KEY = "soloist.git.split";

/** How tall the diff opens the first time, and the bounds a drag may take it between. */
export const SPLIT_DEFAULT_HEIGHT = 420;
export const SPLIT_MIN_HEIGHT = 140;
export const SPLIT_MAX_HEIGHT = 1400;

/** How far one arrow-key press moves the divider. */
export const SPLIT_RESIZE_STEP = 24;

export interface SplitLayout {
  height: number;
  maximized: boolean;
}

function sanitize(stored: Partial<SplitLayout>): SplitLayout {
  return {
    height: clampSize(
      typeof stored.height === "number" ? stored.height : SPLIT_DEFAULT_HEIGHT,
      SPLIT_MIN_HEIGHT,
      SPLIT_MAX_HEIGHT,
    ),
    maximized: stored.maximized === true,
  };
}

/**
 * The diff split's remembered shape: how tall it opens and whether it is filling the area. The
 * upper bound is deliberately generous — the pane's own style caps it against the window, so a
 * height remembered from a taller screen simply gives the largest split this one can show
 * rather than one that pushes the terminal off the bottom.
 */
export function useSplitLayout(): [SplitLayout, (next: Partial<SplitLayout>) => void] {
  return useStoredLayout(STORAGE_KEY, sanitize);
}
