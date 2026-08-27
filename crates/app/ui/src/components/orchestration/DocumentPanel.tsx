import type { ReactNode, RefObject } from "react";

interface DocumentPanelProps {
  /** Wires the container to hotkey handling; only a surface that needs one supplies it (the
   *  scratchpad panel's Ctrl+Shift+W archive shortcut) — a panel with no hotkey scope leaves it unset. */
  panelRef?: RefObject<HTMLDivElement | null>;
  /** The subject's own roster — already wired to its summaries, selection and sort. */
  roster: ReactNode;
  /** The right pane: a placeholder while nothing is open or loaded, or the open document's editor. */
  content: ReactNode;
}

// The shared two-column document surface: a fixed-width roster on the left, a flexible content pane
// on the right. Scratchpads and diagrams differ only in their roster, editor and hotkey wiring — all
// built by the caller — so this holds just the shell both already shared verbatim.
export function DocumentPanel({ panelRef, roster, content }: DocumentPanelProps) {
  return (
    <div ref={panelRef} className="flex h-full min-h-0 tracking-[var(--tracking-body)]">
      <div className="w-60 shrink-0 border-r">{roster}</div>
      <div className="min-w-0 flex-1">{content}</div>
    </div>
  );
}

/** The document panel's centered placeholder — shown while nothing is open, still loading, or refused. */
export function DocumentPlaceholder({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full items-center justify-center p-6 text-center text-[0.8125rem] text-muted-foreground">
      {children}
    </div>
  );
}
