import * as React from "react";
import type { ItemInstance, TreeInstance } from "@headless-tree/core";
import { ChevronRightIcon } from "lucide-react";

import { cn } from "@/lib/utils";

/** How far one level is indented, in pixels — passed to the tree and used to draw the row. */
export const TREE_INDENT = 12;

/**
 * The container: a `role="tree"` element whose props (roving focus, typeahead, arrow keys) come
 * from the tree instance, so keyboard behaviour is the library's rather than re-derived here.
 */
function Tree<T>({
  tree,
  className,
  ...props
}: React.ComponentProps<"div"> & { tree: TreeInstance<T> }) {
  "use no memo"; // the tree instance is stable-identity but mutates internally, so the compiler
  // must not cache its getter output across renders
  return (
    <div
      {...tree.getContainerProps()}
      data-slot="tree"
      // A selection scope, so the selected row wears the azure tint only while the keyboard is
      // in this tree and the neutral fill otherwise — the AppKit first-responder distinction the
      // app's other lists already read.
      data-selection-scope
      className={cn("flex w-max min-w-full flex-col outline-none", className)}
      {...props}
    />
  );
}

/**
 * One row. The tree instance supplies `role="treeitem"`, the tab index that carries roving
 * focus, and the level and expansion state a screen reader reads.
 *
 * A row is a `div` rather than a button because a row carries controls of its own — staging a
 * change, throwing one away — and interactive content inside a button is neither valid nor
 * operable. It is the same shape the process source list uses for the same reason, and the
 * keyboard contract is unaffected: it comes from the tree, not from the element.
 */
function TreeItem<T>({
  item,
  className,
  children,
  ...props
}: React.ComponentProps<"div"> & { item: ItemInstance<T> }) {
  "use no memo"; // the item instance is stable-identity but mutates internally, so the compiler
  // must not cache its getter output across renders
  return (
    <div
      {...item.getProps()}
      data-slot="tree-item"
      data-folder={item.isFolder() ? "" : undefined}
      data-selected={item.isSelected() ? "" : undefined}
      style={{ paddingInlineStart: `${item.getItemMeta().level * TREE_INDENT}px` }}
      className={cn(
        "group/tree-item relative flex h-8 w-max min-w-full cursor-default items-center gap-2 rounded-sm bg-sidebar text-left text-[0.8125rem] tracking-[var(--tracking-body)] outline-none",
        "transition-colors duration-[var(--dur-select)] ease-out-quint",
        "focus-visible:ring-2 focus-visible:ring-sidebar-ring",
        "data-folder:font-medium data-selected:bg-[var(--sel-fill)] data-selected:hover:bg-[var(--sel-fill-hover)]",
        "not-data-selected:hover:bg-sidebar-accent/75",
        className,
      )}
      {...props}
    >
      {children}
    </div>
  );
}

/**
 * The disclosure a folder row leads with. A file gets the same box with nothing in it, so the
 * names in a mixed list stay on one vertical line instead of stepping in and out.
 */
function TreeItemChevron<T>({ item }: { item: ItemInstance<T> }) {
  "use no memo"; // the item instance is stable-identity but mutates internally, so the compiler
  // must not cache its getter output across renders
  if (!item.isFolder()) return <span className="w-3.5 shrink-0" aria-hidden />;
  return (
    <ChevronRightIcon
      aria-hidden
      className={cn(
        "size-3.5 shrink-0 text-muted-foreground/80",
        "transition-transform duration-[var(--dur-control)] ease-spring-settle motion-reduce:transition-none",
        item.isExpanded() && "rotate-90",
      )}
    />
  );
}

/** The row's name, kept on one line so the tree can reveal it by scrolling. */
function TreeItemLabel({ className, ...props }: React.ComponentProps<"span">) {
  return (
    <span
      data-slot="tree-item-label"
      className={cn("w-max flex-none whitespace-nowrap", className)}
      {...props}
    />
  );
}

export { Tree, TreeItem, TreeItemChevron, TreeItemLabel };
