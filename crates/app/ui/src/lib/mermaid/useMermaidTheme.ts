import { useEffect, useState } from "react";
import { themeSignature } from "./theme";

/**
 * Tracks the root's applied-palette signature so a mounted diagram re-renders for both appearance
 * changes and same-appearance custom theme switches.
 */
export function useMermaidTheme(): string {
  const [signature, setSignature] = useState(themeSignature);

  useEffect(() => {
    const root = document.documentElement;
    const observer = new MutationObserver(() => setSignature(themeSignature()));
    observer.observe(root, {
      attributes: true,
      attributeFilter: ["class", "data-theme-signature"],
    });
    // Reconcile against any flip that landed between the initial render and this effect.
    setSignature(themeSignature());
    return () => observer.disconnect();
  }, []);

  return signature;
}
