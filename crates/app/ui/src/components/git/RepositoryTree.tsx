import { useEffect, useState, type ReactNode } from "react";
import { hotkeysCoreFeature, selectionFeature, syncDataLoaderFeature } from "@headless-tree/core";
import { useTree } from "@headless-tree/react";
import { FileTreeIcon } from "@/components/git/FileTreeIcon";
import { Tree, TreeItem, TreeItemChevron, TREE_INDENT } from "@/components/ui/tree";
import { type Tree as RepositoryTreeData, type TreeNode } from "@/store/git/tree";

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
  /** Which folders are open. Owned by the caller, so the control that opens or closes them all
   *  reads the same fact this renders rather than a copy of it. */
  expanded: string[];
  onExpandedChange: (paths: string[] | ((open: string[]) => string[])) => void;
  /** The row's content after the disclosure: the path's name and whatever trails it. */
  row: (node: TreeNode) => ReactNode;
  /** A path was chosen — clicked, or focused and confirmed. Folders open instead, so this only
   *  ever names a file. */
  onOpen?: (node: TreeNode) => void;
}

/**
 * Renders a built repository tree as an accessible `role="tree"`. Arrow-key movement, typeahead,
 * roving focus, and the ARIA level/expansion state all come from the tree instance, so the
 * keyboard contract is the library's rather than re-derived here — the reason this replaces the
 * hand-rolled trees elsewhere in the app rather than copying them.
 *
 * Presentational: it renders the tree it is handed and reports nothing back beyond what the reader
 * did. What a row *means* is the caller's `row`; which folders are open is the caller's too.
 */
export function RepositoryTree({
  data,
  label,
  expanded,
  onExpandedChange,
  row,
  onOpen,
}: RepositoryTreeProps) {
  const [selectedItems, setSelectedItems] = useState<string[]>([]);

  const tree = useTree<TreeNode>({
    rootItemId: ROOT,
    state: { expandedItems: expanded, selectedItems },
    setExpandedItems: onExpandedChange,
    setSelectedItems,
    indent: TREE_INDENT,
    getItemName: (item) => item.getItemData().name,
    isItemFolder: (item) => item.getItemData().folder,
    dataLoader: {
      getItem: (path) => data.nodes[path] ?? MISSING,
      getChildren: (path) => (path === ROOT ? data.roots : (data.nodes[path]?.children ?? [])),
    },
    onPrimaryAction: (item) => {
      const node = item.getItemData();
      // A folder's own primary action is to open or close, which the tree already does.
      if (!node.folder) onOpen?.(node);
    },
    features: [syncDataLoaderFeature, selectionFeature, hotkeysCoreFeature],
  });

  useEffect(() => {
    // The tree caches the shape it last walked, so a repository that changed under it has to be
    // walked again — otherwise a path that appeared would not show until something else moved.
    tree.rebuildTree();

    // A path that is gone can still be named by the retained selection; dropping it keeps the tree
    // from holding a growing list of paths that no longer exist.
    const present = new Set(Object.keys(data.nodes));
    setSelectedItems((paths) => {
      const kept = paths.filter((path) => present.has(path));
      return kept.length === paths.length ? paths : kept;
    });
  }, [tree, data]);

  return (
    <Tree tree={tree} aria-label={label} className="px-1.5 py-1.5">
      {tree.getItems().map((item) => (
        <TreeItem key={item.getId()} item={item}>
          <TreeItemChevron item={item} />
          <FileTreeIcon node={item.getItemData()} expanded={item.isExpanded()} />
          {row(item.getItemData())}
        </TreeItem>
      ))}
    </Tree>
  );
}
