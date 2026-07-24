import { useEffect, useState } from "react";
import { themeSignature } from "./theme";

/**
 * Tracks the diagram theme signature ("light" | "dark") and updates it when the app flips theme, so a
 * mounted diagram can re-render in the new palette. The signal is the `.dark` class on the document
 * root (toggled by `applyDarkClass`); a `MutationObserver` on that one attribute is cheaper than a
 * global theme subscription and keeps this hook independent of the appearance provider.
 */
export function useMermaidTheme(): string {
  const [signature, setSignature] = useState(themeSignature);

  useEffect(() => {
    const root = document.documentElement;
    const observer = new MutationObserver(() => setSignature(themeSignature()));
    observer.observe(root, { attributes: true, attributeFilter: ["class"] });
    // Reconcile against any flip that landed between the initial render and this effect.
    setSignature(themeSignature());
    return () => observer.disconnect();
  }, []);

  return signature;
}
