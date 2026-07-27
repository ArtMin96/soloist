import { createContext, use, useEffect, useState, type RefObject } from "react";
import { useLatestRef } from "@/store/useLatestRef";

/** A region of the window that accepts dropped files, and what to do with the paths that land in it. */
export interface FileDropTarget {
  /** The element whose box a drop position is hit-tested against, resolved as the event arrives. */
  host: () => HTMLElement | null;
  /** Handed the dropped paths once a drop lands inside the host. */
  onDrop: (paths: string[]) => void;
}

/** The drop registry: how a surface joins it, and which member a drag is currently over. */
export interface FileDropState {
  /** Registers a target for as long as the returned function is uncalled. */
  register: (target: FileDropTarget) => () => void;
  /** Gives up the hover mark if this target holds it. Its registration is untouched. */
  clearHover: (target: FileDropTarget) => void;
  /** The registered target a drag is hovering, or null when a drag is over none of them. */
  hovered: FileDropTarget | null;
}

// A registry that accepts everything and reports nothing hovered, so a surface rendered without the
// provider — a focused test, a future window that has no drop targets — still mounts.
const NO_DROP_TARGETS: FileDropState = {
  register: () => () => {},
  clearHover: () => {},
  hovered: null,
};

export const FileDropContext = createContext<FileDropState>(NO_DROP_TARGETS);

/**
 * Accept files dropped onto `host`, and report whether a drag is currently over it so the surface
 * can show where a drop would land.
 *
 * The registration is one object for the whole mount: hover events arrive at pointer-move rate, and
 * a target whose identity changed per render would turn every one of them into a state change. Both
 * arguments are therefore read at the moment an event arrives rather than captured at registration,
 * so either may be a fresh value each render without the registration going stale.
 *
 * `shown` is what the surface is currently displaying. A surface that stops being displayed while a
 * drag is over it will never be told the drag left — the drag can end anywhere, and the events that
 * would clear the mark are addressed to wherever the pointer went. Without giving the mark up here,
 * the surface would come back still marked for a drag that ended long ago.
 */
export function useFileDropTarget(
  host: RefObject<HTMLElement | null>,
  onDrop: (paths: string[]) => void,
  shown = true,
): boolean {
  const { register, clearHover, hovered } = use(FileDropContext);
  const latest = useLatestRef({ host, onDrop });
  const [target] = useState<FileDropTarget>(() => ({
    host: () => latest.current.host.current,
    onDrop: (paths) => latest.current.onDrop(paths),
  }));
  useEffect(() => register(target), [register, target]);
  useEffect(() => {
    if (!shown) clearHover(target);
  }, [shown, clearHover, target]);
  return hovered === target;
}
