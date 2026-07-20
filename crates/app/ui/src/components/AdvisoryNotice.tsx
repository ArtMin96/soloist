import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

// An inline advisory strip: something about the surface needs the user's attention, but nothing is
// broken and nothing was lost. It wears the transition tone — the same amber that marks a reversible
// in-flight process state — because that is exactly its severity: a gap the user can close, not a
// failure. A refusal the core reported is a destructive-text line instead; this is not that.
//
// `action` carries the one control that resolves the notice (a Reload for a stale revision), and is
// omitted when the notice is purely something to know.
//
// `urgency` decides how a screen reader announces it. `alert` interrupts, which is right for
// something that just happened to the user's work — a save refused because the document moved on.
// `status` waits for a pause, which is what an advisory that re-renders while the user types needs:
// announcing the unfilled-placeholder notice assertively would re-interrupt on every keystroke.
export type AdvisoryUrgency = "alert" | "status";

export function AdvisoryNotice({
  children,
  action,
  className,
  urgency = "alert",
}: {
  children: ReactNode;
  action?: ReactNode;
  className?: string;
  urgency?: AdvisoryUrgency;
}) {
  return (
    <div
      role={urgency}
      className={cn(
        "flex items-center gap-3 rounded-md border border-status-transition/40 bg-status-transition/10 px-3 py-2 text-[0.8125rem]",
        className,
      )}
    >
      <span className="min-w-0 flex-1">{children}</span>
      {action}
    </div>
  );
}
