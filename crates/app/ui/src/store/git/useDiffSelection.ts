import { useCallback, useState } from "react";

/** What a selected row asked to be shown: a change to a path, or the path itself. */
export const CHANGE = "change" as const;
export const FILE = "file" as const;

export interface DiffSelection {
  kind: typeof CHANGE | typeof FILE;
  path: string;
}

export interface DiffSelectionStore {
  /** What the split is showing, or null when it is closed. */
  selection: DiffSelection | null;
  open: (selection: DiffSelection) => void;
  close: () => void;
}

/**
 * What the diff split is showing, if anything.
 *
 * A selection belongs to the project it was made in: switching project closes the split rather
 * than leaving another repository's file on screen under a path that may not even exist here.
 */
export function useDiffSelection(project: number | null): DiffSelectionStore {
  const [held, setHeld] = useState<{ project: number | null; selection: DiffSelection } | null>(
    null,
  );
  const open = useCallback(
    (selection: DiffSelection) => setHeld({ project, selection }),
    [project],
  );
  const close = useCallback(() => setHeld(null), []);
  return {
    selection: held !== null && held.project === project ? held.selection : null,
    open,
    close,
  };
}
