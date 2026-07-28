import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { PhysicalPosition } from "@tauri-apps/api/dpi";
import { onFileDrop } from "@/lib/fileDrop";
import { FileDropContext, type FileDropTarget } from "@/store/fileDropContext";

// Whether a point in CSS pixels lands inside an element's box.
//
// The box is half-open — its left and top edges belong to it, its right and bottom edges do not —
// so no point is ever in two boxes at once and, more importantly here, a zero-size box contains
// nothing at all. That is what keeps a drop out of the hidden panes of the keep-alive terminal
// pool: `display: none` reports exactly such a box, so only the pane the user can see can be hit.
function contains(host: HTMLElement, x: number, y: number): boolean {
  const rect = host.getBoundingClientRect();
  return x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom;
}

// The registered target a drag position falls in, or null when it falls outside all of them.
//
// The position arrives in *physical* pixels while a client rect is in *CSS* pixels, so on any
// display whose scale factor is not 1 the two disagree and an unconverted comparison silently picks
// the wrong element — or none. Their origins do agree: the app's title bar is drawn inside the
// webview and the shell fills it with no page scroll, so the webview's top-left is the viewport's.
function targetAt(
  targets: Iterable<FileDropTarget>,
  position: PhysicalPosition,
): FileDropTarget | null {
  const { x, y } = position.toLogical(window.devicePixelRatio);
  for (const target of targets) {
    const host = target.host();
    if (host && contains(host, x, y)) return target;
  }
  return null;
}

/**
 * Holds the app's one subscription to the OS drag-and-drop stream and routes each event to the
 * surface under the pointer.
 *
 * The stream is window-wide, so subscribing per drop target would mean one listener per pane for
 * events they would each have to filter anyway. Mounted at the app root, above every surface that
 * accepts a drop.
 */
export function FileDropProvider({ children }: { children: ReactNode }) {
  const targets = useRef(new Set<FileDropTarget>());
  const [hovered, setHovered] = useState<FileDropTarget | null>(null);

  const clearHover = useCallback((target: FileDropTarget) => {
    setHovered((current) => (current === target ? null : current));
  }, []);

  const register = useCallback(
    (target: FileDropTarget) => {
      targets.current.add(target);
      return () => {
        targets.current.delete(target);
        // A target that unmounts mid-drag is never going to report that the drag left it.
        clearHover(target);
      };
    },
    [clearHover],
  );

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let stopped = false;
    void onFileDrop((event) => {
      switch (event.type) {
        case "enter":
        case "over":
          setHovered(targetAt(targets.current, event.position));
          break;
        case "drop":
          setHovered(null);
          targetAt(targets.current, event.position)?.onDrop(event.paths);
          break;
        case "leave":
          setHovered(null);
          break;
      }
    })
      .then((stop) => {
        // The subscription can resolve after this effect tore down; end it straight away in that
        // case, or it is a listener nothing holds a handle to for the life of the app.
        if (stopped) stop();
        else unlisten = stop;
      })
      .catch(() => {
        // No drag-and-drop from the platform; the app simply does not accept drops.
      });
    return () => {
      stopped = true;
      unlisten?.();
    };
  }, []);

  const state = useMemo(() => ({ register, clearHover, hovered }), [register, clearHover, hovered]);
  return <FileDropContext value={state}>{children}</FileDropContext>;
}
