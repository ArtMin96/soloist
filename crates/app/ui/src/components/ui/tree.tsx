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
  return (
    <div
      {...tree.getContainerProps()}
      data-slot="tree"
      // A selection scope, so the selected row wears the azure tint only while the keyboard is
      // in this tree and the neutral fill otherwise — the AppKit first-responder distinction the
      // app's other lists already read.
      data-selection-scope
      className={cn("flex flex-col outline-none", className)}
      {...props}
    />
  );
}

/**
 * One row. Rendered as a button so it is focusable and pressable by construction; the tree
 * instance supplies `role="treeitem"` and the level/expansion state that screen readers read.
 */
function TreeItem<T>({
  item,
  className,
  children,
  ...props
}: Omit<React.ComponentProps<"button">, "type"> & { item: ItemInstance<T> }) {
  return (
    <button
      {...item.getProps()}
      type="button"
      data-slot="tree-item"
      data-folder={item.isFolder() ? "" : undefined}
      data-selected={item.isSelected() ? "" : undefined}
      style={{ paddingInlineStart: `${item.getItemMeta().level * TREE_INDENT}px` }}
      className={cn(
        "relative flex h-8 w-full min-w-0 cursor-default items-center gap-2 rounded-sm pe-2 text-left text-[0.8125rem] tracking-[var(--tracking-body)] outline-none",
        "transition-colors duration-[var(--dur-select)] ease-out-quint",
        "focus-visible:ring-2 focus-visible:ring-sidebar-ring",
        "data-folder:font-medium data-selected:bg-[var(--sel-fill)] data-selected:hover:bg-[var(--sel-fill-hover)]",
        "not-data-selected:hover:bg-sidebar-accent/75",
        className,
      )}
      {...props}
    >
      {children}
    </button>
  );
}

/**
 * The disclosure a folder row leads with. A file gets the same box with nothing in it, so the
 * names in a mixed list stay on one vertical line instead of stepping in and out.
 */
function TreeItemChevron<T>({ item }: { item: ItemInstance<T> }) {
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

/** The row's name, truncating rather than wrapping — a rail is narrow by design. */
function TreeItemLabel({ className, ...props }: React.ComponentProps<"span">) {
  return <span data-slot="tree-item-label" className={cn("min-w-0 truncate", className)} {...props} />;
}

export { Tree, TreeItem, TreeItemChevron, TreeItemLabel };
