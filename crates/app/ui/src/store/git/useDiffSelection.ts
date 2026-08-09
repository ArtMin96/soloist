import { useCallback, useState } from "react";

/** What a selected row asked to be shown: a change to a path, or the path itself. */
export const CHANGE = "change" as const;
export const FILE = "file" as const;

/** What the branch header asks for: the pull request this branch has, or the form that opens one. */
export const PULL_REQUEST = "pull_request" as const;

export interface DiffSelection {
  kind: typeof CHANGE | typeof FILE;
  path: string;
}

/** What the split is showing: a path, or the pull-request view, which names nothing. */
export type SplitView = DiffSelection | { kind: typeof PULL_REQUEST };

export interface DiffSelectionStore {
  /** What the split is showing, or null when it is closed. */
  selection: SplitView | null;
  open: (selection: SplitView) => void;
  close: () => void;
}

/**
 * What the split is showing, if anything.
 *
 * A selection belongs to the project it was made in: switching project closes the split rather
 * than leaving another repository's file — or another repository's pull request — on screen.
 */
export function useDiffSelection(project: number | null): DiffSelectionStore {
  const [held, setHeld] = useState<{ project: number | null; selection: SplitView } | null>(null);
  const open = useCallback((selection: SplitView) => setHeld({ project, selection }), [project]);
  const close = useCallback(() => setHeld(null), []);
  return {
    selection: held !== null && held.project === project ? held.selection : null,
    open,
    close,
  };
}
