import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

// An inline advisory strip: something about the surface needs the user's attention, but nothing is
// broken and nothing was lost. It wears the transition tone — the same amber that marks a reversible
// in-flight process state — because that is exactly its severity: a gap the user can close, not a
// failure. A refusal the core reported is a destructive-text line instead; this is not that.
//
// `action` carries the one control that resolves the notice (a Reload for a stale revision), and is
// omitted when the notice is purely something to know.
export function AdvisoryNotice({
  children,
  action,
  className,
}: {
  children: ReactNode;
  action?: ReactNode;
  className?: string;
}) {
  return (
    <div
      role="alert"
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
