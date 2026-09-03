import { useEffect, useState, type ReactNode } from "react";
import { hotkeysCoreFeature, selectionFeature, syncDataLoaderFeature } from "@headless-tree/core";
// The item instances `useTree` hands out are stable-identity but mutate internally, so the
// compiler-safe entrypoint returns a getter instead of the instance itself — every access below
// calls `tree()` fresh rather than memoizing against an identity that never changes.
import { useTree } from "@headless-tree/react/react-compiler";
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

  // A path that is gone can still be named by the retained selection; dropping it here, the render
  // `data` itself changed in, keeps the tree from holding a growing list of paths that no longer
  // exist. Tracked against `data`'s own identity rather than an effect so a repository that changed
  // out from under the tree never paints a stale selection first.
  const [prunedFor, setPrunedFor] = useState(data);
  if (prunedFor !== data) {
    setPrunedFor(data);
    const present = new Set(Object.keys(data.nodes));
    setSelectedItems((paths) => {
      const kept = paths.filter((path) => present.has(path));
      return kept.length === paths.length ? paths : kept;
    });
  }

  const getTree = useTree<TreeNode>({
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

  // `getTree` is a fresh closure every render — only what it resolves to is stable — so the
  // rebuild effect closes over the resolved instance rather than the closure, or it would refire
  // (and push a new state object) on every render instead of only when `data` changes.
  const treeInstance = getTree();
  useEffect(() => {
    // The tree caches the shape it last walked, so a repository that changed under it has to be
    // walked again — otherwise a path that appeared would not show until something else moved.
    treeInstance.rebuildTree();
  }, [treeInstance, data]);

  return (
    <Tree tree={getTree()} aria-label={label} className="px-1.5 py-1.5">
      {getTree()
        .getItems()
        .map((item) => (
          <TreeItem key={item.getId()} item={item}>
            <TreeItemChevron item={item} />
            <FileTreeIcon node={item.getItemData()} expanded={item.isExpanded()} />
            {row(item.getItemData())}
          </TreeItem>
        ))}
    </Tree>
  );
}
