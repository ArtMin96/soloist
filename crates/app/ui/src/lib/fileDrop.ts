import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview, type DragDropEvent } from "@tauri-apps/api/webview";

// The single boundary to the OS drag-and-drop stream.
//
// The webview handles the drop natively, which is what makes the paths below real filesystem paths
// — a file dropped through the DOM's own drag-and-drop arrives as a `File` with no path at all, and
// recovering one would mean reading the bytes back out to a temporary file. Like the window-chrome
// boundary, this is a platform concern rather than a domain command, so it talks to the Tauri
// webview API directly instead of going through the Facade IPC in `api.ts`.

export type { DragDropEvent };

/**
 * Subscribe to the window's drag-and-drop stream. The events are window-wide rather than per
 * element — each one carries the position it happened at, which is what a subscriber routes by — so
 * one subscription serves the whole app.
 *
 * Returns the handle that ends the subscription. It must be called, or the listener outlives
 * whatever installed it.
 */
export function onFileDrop(handler: (event: DragDropEvent) => void): Promise<UnlistenFn> {
  return getCurrentWebview().onDragDropEvent(({ payload }) => handler(payload));
}
