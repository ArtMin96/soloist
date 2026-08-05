import { useEffect, useRef, useState, type ReactNode } from "react";
import { hotkeysCoreFeature, selectionFeature, syncDataLoaderFeature } from "@headless-tree/core";
import { useTree } from "@headless-tree/react";
import { Tree, TreeItem, TreeItemChevron, TREE_INDENT } from "@/components/ui/tree";
import { folderPaths, type Tree as RepositoryTreeData, type TreeNode } from "@/store/git/tree";

/** The tree's own root. It holds the top-level paths and is never itself rendered. */
const ROOT = "";

/** Stands in for a path the loader is asked about after it has gone — never rendered. */
const MISSING: TreeNode = {
  path: ROOT,
  name: "",
  children: [],
  folder: false,
  change: null,
  ignored: false,
};

interface RepositoryTreeProps {
  data: RepositoryTreeData;
  /** Names the tree for a screen reader, e.g. "Changed files". */
  label: string;
  /**
   * Open each folder the first time it appears. A changed-files list is short and is read for
   * the files, so it opens itself; a whole project is not, so it does not. Either way a folder
   * the user has since closed stays closed — it has been seen.
   */
  autoExpand: boolean;
  /** The row's content after the disclosure: the path's name and whatever trails it. */
  row: (node: TreeNode) => ReactNode;
}

/**
 * Renders a built repository tree as an accessible `role="tree"`. Arrow-key movement, typeahead,
 * roving focus, and the ARIA level/expansion state all come from the tree instance, so the
 * keyboard contract is the library's rather than re-derived here — the reason this replaces the
 * hand-rolled trees elsewhere in the app rather than copying them.
 *
 * Presentational: it renders the tree it is handed and reports nothing back. What a row *means*
 * is the caller's `row`.
 */
export function RepositoryTree({ data, label, autoExpand, row }: RepositoryTreeProps) {
  const [expandedItems, setExpandedItems] = useState<string[]>([]);
  const [selectedItems, setSelectedItems] = useState<string[]>([]);
  const seenFolders = useRef(new Set<string>());

  const tree = useTree<TreeNode>({
    rootItemId: ROOT,
    state: { expandedItems, selectedItems },
    setExpandedItems,
    setSelectedItems,
    indent: TREE_INDENT,
    getItemName: (item) => item.getItemData().name,
    isItemFolder: (item) => item.getItemData().folder,
    dataLoader: {
      getItem: (path) => data.nodes[path] ?? MISSING,
      getChildren: (path) => (path === ROOT ? data.roots : (data.nodes[path]?.children ?? [])),
    },
    features: [syncDataLoaderFeature, selectionFeature, hotkeysCoreFeature],
  });

  useEffect(() => {
    // The tree caches the shape it last walked, so a repository that changed under it has to be
    // walked again — otherwise a path that appeared would not show until something else moved.
    tree.rebuildTree();

    const folders = folderPaths(data);
    const fresh = autoExpand ? folders.filter((path) => !seenFolders.current.has(path)) : [];
    seenFolders.current = new Set(folders);

    // A path that is gone can still be named by the retained expansion or selection; dropping
    // those keeps the tree from holding a growing list of paths that no longer exist.
    const present = new Set(Object.keys(data.nodes));
    setExpandedItems((paths) => {
      const kept = paths.filter((path) => present.has(path));
      if (fresh.length === 0) return kept.length === paths.length ? paths : kept;
      return [...kept, ...fresh];
    });
    setSelectedItems((paths) => {
      const kept = paths.filter((path) => present.has(path));
      return kept.length === paths.length ? paths : kept;
    });
  }, [tree, data, autoExpand]);

  return (
    <Tree tree={tree} aria-label={label} className="px-1 py-1">
      {tree.getItems().map((item) => (
        <TreeItem key={item.getId()} item={item}>
          <TreeItemChevron item={item} />
          {row(item.getItemData())}
        </TreeItem>
      ))}
    </Tree>
  );
}
