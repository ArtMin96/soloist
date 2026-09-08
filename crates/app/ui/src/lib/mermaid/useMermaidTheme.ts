import { useSyncExternalStore } from "react";
import { themeSignature } from "./theme";

// Subscribes a MutationObserver to the root's palette attributes and hands the callback to
// `useSyncExternalStore`, which is what closes the gap a manual effect+state pairing has here: it
// always re-reads `themeSignature()` for the render that follows a subscribe, so a flip landing
// between mount and the observer attaching is never missed.
function subscribe(onChange: () => void): () => void {
  const observer = new MutationObserver(onChange);
  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["class", "data-theme-signature"],
  });
  return () => observer.disconnect();
}

/**
 * Tracks the root's applied-palette signature so a mounted diagram re-renders for both appearance
 * changes and same-appearance custom theme switches.
 */
export function useMermaidTheme(): string {
  return useSyncExternalStore(subscribe, themeSignature);
}
