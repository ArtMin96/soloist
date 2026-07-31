import { createContext, useContext } from "react";

/** What the enclosing list lets a consumer do to one of its items. */
export interface SortableListActions {
  /** Moves `id` by `delta` places and commits the result — negative moves it toward the front. */
  moveItemBy: (id: string, delta: number) => void;
  /** Whether `id` has anywhere to go `delta` places away. */
  canMoveItemBy: (id: string, delta: number) => boolean;
}

export const SortableListContext = createContext<SortableListActions | null>(null);

/**
 * The move actions of the enclosing `SortableList`, for building affordances that reorder without
 * a pointer — a menu item, a button, a shortcut. `null` outside a list, and `null` is the answer
 * to build against: an item with no list around it has nowhere to move to, so the control belongs
 * hidden rather than present and dead. Keeping it an answer rather than a throw is what lets an
 * item still be rendered on its own.
 */
export function useSortableList(): SortableListActions | null {
  return useContext(SortableListContext);
}
