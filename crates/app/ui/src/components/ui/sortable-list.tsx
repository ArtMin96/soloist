import { createContext, useCallback, useContext, useMemo, useState } from "react";
import {
  closestCenter,
  DndContext,
  PointerSensor,
  useSensor,
  useSensors,
  type Announcements,
  type DragEndEvent,
  type Modifier,
} from "@dnd-kit/core";
import { SortableContext, useSortable, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { canMoveBy, moveBy, moveItem } from "@/lib/sortable";
import { cn } from "@/lib/utils";

/**
 * How far a press travels before it becomes a drag. An item is dragged by its own body rather
 * than by a grip, so the controls it carries have to keep taking clicks: below this, the press
 * belongs to whatever was under it.
 */
const DRAG_ACTIVATION_DISTANCE = 4;

/** The displaced items' settle, in ms — the `--dur-control` duration this side of the boundary. */
const SORT_DURATION = 220;

/** Reordering is one axis, so a drag that wanders sideways still reads as a clean list move. */
const verticalOnly: Modifier = ({ transform }) => ({ ...transform, x: 0 });

// Animating a reorder means transforming the items that give up their place, which the OS-level
// preference asks us not to do; there the move simply happens.
function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

interface SortableListContext {
  /** Moves `id` by `delta` places and commits the result — negative moves it toward the front. */
  moveItemBy: (id: string, delta: number) => void;
  /** Whether `id` has anywhere to go `delta` places away. */
  canMoveItemBy: (id: string, delta: number) => boolean;
}

const ListContext = createContext<SortableListContext | null>(null);

/**
 * The move actions of the enclosing [`SortableList`], for building affordances that reorder
 * without a pointer — a menu item, a button, a shortcut. Throws outside a list, so a consumer
 * cannot silently render a control that does nothing.
 */
export function useSortableList(): SortableListContext {
  const context = useContext(ListContext);
  if (!context) throw new Error("useSortableList must be used within a SortableList");
  return context;
}

export interface SortableListProps {
  /** The item ids, in the order they are rendered. */
  ids: string[];
  /** The full new order, once a move commits. */
  onReorder: (ids: string[]) => void;
  /** The item's name, for what assistive tech is told about a move. Defaults to the id. */
  nameOf?: (id: string) => string;
  /** Stops items being dragged — e.g. while only part of the list is on screen. */
  disabled?: boolean;
  children: React.ReactNode;
}

/**
 * A vertical list whose items the user can rearrange by dragging any part of them. Wrap the
 * items in this and render each through [`SortableItem`]; the list reports the whole new order
 * rather than a delta, so the caller stores an order and never a drag.
 *
 * Pointer moves are handled here. A move made without a pointer is the caller's to offer — take
 * [`useSortableList`] and put it on a control your users can reach; announcing it is handled
 * here either way.
 */
export function SortableList({ ids, onReorder, nameOf, disabled, children }: SortableListProps) {
  const [announcement, setAnnouncement] = useState("");
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: DRAG_ACTIVATION_DISTANCE } }),
  );

  const name = useCallback((id: string) => nameOf?.(id) ?? id, [nameOf]);
  const place = useCallback((id: string) => `${ids.indexOf(id) + 1} of ${ids.length}`, [ids]);

  const moveItemBy = useCallback(
    (id: string, delta: number) => {
      if (!canMoveBy(ids, id, delta)) return;
      const next = moveBy(ids, id, delta);
      onReorder(next);
      setAnnouncement(`${name(id)} moved to ${next.indexOf(id) + 1} of ${next.length}.`);
    },
    [ids, onReorder, name],
  );

  const context = useMemo<SortableListContext>(
    () => ({
      moveItemBy,
      canMoveItemBy: (id, delta) => canMoveBy(ids, id, delta),
    }),
    [ids, moveItemBy],
  );

  const announcements: Announcements = {
    onDragStart: ({ active }) => `Picked up ${name(String(active.id))}, ${place(String(active.id))}.`,
    onDragOver: ({ active, over }) =>
      over ? `${name(String(active.id))} is over ${place(String(over.id))}.` : undefined,
    onDragEnd: ({ active, over }) =>
      over
        ? `${name(String(active.id))} dropped at ${place(String(over.id))}.`
        : `${name(String(active.id))} was left where it was.`,
    onDragCancel: ({ active }) => `Move cancelled. ${name(String(active.id))} was left where it was.`,
  };

  function handleDragEnd({ active, over }: DragEndEvent) {
    if (!over || active.id === over.id) return;
    onReorder(moveItem(ids, ids.indexOf(String(active.id)), ids.indexOf(String(over.id))));
  }

  return (
    <ListContext.Provider value={context}>
      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        modifiers={[verticalOnly]}
        accessibility={{ announcements }}
        onDragEnd={handleDragEnd}
      >
        <SortableContext items={ids} strategy={verticalListSortingStrategy} disabled={disabled}>
          {children}
        </SortableContext>
      </DndContext>
      {/* A move made from a control rather than a drag: dnd-kit's own live region covers the
          drag, and never sees this one. */}
      <span data-slot="sortable-announcement" aria-live="polite" className="sr-only">
        {announcement}
      </span>
    </ListContext.Provider>
  );
}

/** What a sortable item hands back, for placing the drag on the part that should carry it. */
export interface SortableHandle {
  /** Spread onto the element the user drags by — often the item's whole body. */
  handleProps: Record<string, unknown>;
  /** True while this item is the one being dragged. */
  isDragging: boolean;
}

export interface SortableItemProps {
  id: string;
  className?: string;
  /**
   * The item. As a function, it is handed the drag handle to place on part of itself; as plain
   * children, the whole item carries the drag.
   */
  children: React.ReactNode | ((handle: SortableHandle) => React.ReactNode);
}

/**
 * One rearrangeable item. It owns the movement — the lift of the item being dragged and the
 * settle of the ones giving up their place — so a caller styles content and never a drag.
 */
export function SortableItem({ id, className, children }: SortableItemProps) {
  const { listeners, setNodeRef, setActivatorNodeRef, transform, transition, isDragging } =
    useSortable({ id, transition: { duration: SORT_DURATION, easing: "var(--ease-spring-settle)" } });

  // Only the listeners, never dnd-kit's `attributes`: those announce a keyboard-draggable button,
  // and an item dragged by its whole body already holds the controls that own those semantics.
  const handle: SortableHandle = {
    handleProps: { ...listeners, ref: setActivatorNodeRef },
    isDragging,
  };

  return (
    <div
      ref={setNodeRef}
      data-dragging={isDragging || undefined}
      style={{
        transform: CSS.Translate.toString(transform),
        transition: prefersReducedMotion() ? undefined : transition,
      }}
      className={cn(
        "touch-none",
        // The dragged item lifts off the list and rides above the ones it is passing; everything
        // else stays flat, as a resting surface should.
        isDragging && "relative z-10 rounded-md bg-sidebar shadow-overlay",
        className,
      )}
    >
      {typeof children === "function" ? children(handle) : children}
    </div>
  );
}
