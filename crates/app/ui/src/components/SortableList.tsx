import { useCallback, useMemo, useState } from "react";
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
import { SortableListContext, type SortableListActions } from "@/components/useSortableList";
import { canMoveBy, moveBy, moveItem } from "@/lib/sortable";
import { cn } from "@/lib/utils";
import { prefersReducedMotion } from "@/store/useScrollSpy";

/**
 * How far a press travels before it becomes a drag. An item is dragged by its own body rather
 * than by a grip, so the controls it carries have to keep taking clicks: below this, the press
 * belongs to whatever was under it.
 */
const DRAG_ACTIVATION_DISTANCE = 4;

/**
 * The displaced items' settle, in ms. dnd-kit composes the transition in script and takes a
 * number, so this side of the boundary cannot say `var(--dur-control)` — it mirrors that token
 * instead, and `SortableList.test.tsx` fails if the two ever part.
 */
export const SORT_DURATION = 220;

/** Marks the live region a move made without a drag is announced through. */
export const ANNOUNCEMENT_SLOT = "sortable-announcement";

/** Reordering is one axis, so a drag that wanders sideways still reads as a clean list move. */
const verticalOnly: Modifier = ({ transform }) => ({ ...transform, x: 0 });

export interface SortableListProps {
  /** The item ids, in the order they are rendered. */
  ids: string[];
  /** The full new order, once a move commits. */
  onReorder: (ids: string[]) => void;
  /** The item's name, for what assistive tech is told about a move. Defaults to the id. */
  nameOf?: (id: string) => string;
  /**
   * Stops the list being rearranged at all — by drag or by any control built on
   * [`useSortableList`]. Set it when `ids` is only part of the list, so an order arranged from
   * what is on screen is never filed as the answer for the whole of it.
   */
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
      if (disabled || !canMoveBy(ids, id, delta)) return;
      const next = moveBy(ids, id, delta);
      onReorder(next);
      setAnnouncement(`${name(id)} moved to ${next.indexOf(id) + 1} of ${next.length}.`);
    },
    [disabled, ids, onReorder, name],
  );

  // `disabled` is gated here rather than at each call site, so it holds for every way of moving an
  // item and not just the drag. A caller disables the list because the ids it handed over are not
  // the whole list; an order arranged from part of one is not the user's answer for all of it, by
  // whichever affordance it was arranged.
  const context = useMemo<SortableListActions>(
    () => ({
      moveItemBy,
      canMoveItemBy: (id, delta) => !disabled && canMoveBy(ids, id, delta),
    }),
    [disabled, ids, moveItemBy],
  );

  const announcements: Announcements = {
    onDragStart: ({ active }) =>
      `Picked up ${name(String(active.id))}, ${place(String(active.id))}.`,
    onDragOver: ({ active, over }) =>
      over ? `${name(String(active.id))} is over ${place(String(over.id))}.` : undefined,
    onDragEnd: ({ active, over }) =>
      over
        ? `${name(String(active.id))} dropped at ${place(String(over.id))}.`
        : `${name(String(active.id))} was left where it was.`,
    onDragCancel: ({ active }) =>
      `Move cancelled. ${name(String(active.id))} was left where it was.`,
  };

  function handleDragEnd({ active, over }: DragEndEvent) {
    if (!over || active.id === over.id) return;
    onReorder(moveItem(ids, ids.indexOf(String(active.id)), ids.indexOf(String(over.id))));
  }

  return (
    <SortableListContext.Provider value={context}>
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
      <span data-slot={ANNOUNCEMENT_SLOT} aria-live="polite" className="sr-only">
        {announcement}
      </span>
    </SortableListContext.Provider>
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
    useSortable({
      id,
      transition: { duration: SORT_DURATION, easing: "var(--ease-spring-settle)" },
    });

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
