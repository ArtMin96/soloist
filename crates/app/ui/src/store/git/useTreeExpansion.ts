import { useCallback, useMemo, useState } from "react";
import { folderPaths, type Tree } from "@/store/git/tree";

export interface TreeExpansion {
  /** The folders currently open, for the tree to render. */
  expanded: string[];
  /** Takes the tree's own answer for which folders are open — a list, or an update of one. */
  setExpanded: (paths: string[] | ((open: string[]) => string[])) => void;
  /** Whether every folder there is is open, which is what a header control shows and reverses. */
  allExpanded: boolean;
  /** Opens every folder, or closes every one, whichever the tree is not already. */
  toggleAll: () => void;
}

/**
 * Which of a repository tree's folders are open.
 *
 * Held as **the folders whose state differs from the default** rather than as the open ones, which
 * is what makes the whole thing a derivation. A changed-files list opens itself because it is
 * short and read for its files; a whole project does not. Either way a folder somebody opened or
 * closed keeps that decision, a folder that appears takes the default, and a folder that goes away
 * is dropped the next time anything is toggled — with nothing recomputing state in reaction to a
 * read landing.
 *
 * It lives beside the control that reverses it rather than inside the tree, so the two never have
 * to be kept in step: one owner, and both the tree and the header read from it.
 */
export function useTreeExpansion(tree: Tree | null, openByDefault: boolean): TreeExpansion {
  const [toggled, setToggled] = useState<ReadonlySet<string>>(new Set());
  const folders = useMemo(() => (tree === null ? [] : folderPaths(tree)), [tree]);
  const expanded = useMemo(
    () => folders.filter((path) => toggled.has(path) !== openByDefault),
    [folders, openByDefault, toggled],
  );
  const allExpanded = folders.length > 0 && expanded.length === folders.length;

  const setExpanded = useCallback(
    (paths: string[] | ((open: string[]) => string[])) => {
      const open = new Set(typeof paths === "function" ? paths(expanded) : paths);
      setToggled(new Set(folders.filter((path) => open.has(path) !== openByDefault)));
    },
    [expanded, folders, openByDefault],
  );

  const toggleAll = useCallback(
    () => setToggled(!allExpanded !== openByDefault ? new Set(folders) : new Set()),
    [allExpanded, folders, openByDefault],
  );

  return { expanded, setExpanded, allExpanded, toggleAll };
}
