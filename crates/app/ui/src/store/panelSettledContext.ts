import { createContext, use } from "react";

// Whether the panel a subtree sits in has stopped moving. Read at the leaves — a prose body several
// components below the panel that carries it — so it travels by context rather than drilling
// through every board, detail pane and comment list in between. The default is settled, so content
// rendered outside a sliding track at all (a template preview, a comment thread, a focused test)
// renders at once instead of waiting for a signal nobody will send.
export const PanelSettledContext = createContext(true);

/**
 * Whether the panel this subtree sits in has finished arriving — the moment expensive content can
 * be built without stealing frames from the movement that is bringing it on screen.
 */
export function usePanelSettled(): boolean {
  return use(PanelSettledContext);
}
