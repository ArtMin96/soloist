import type { ReactNode } from "react";
import { Alert, AlertAction, AlertDescription } from "@/components/ui/alert";
import { cn } from "@/lib/utils";

// An inline advisory strip: something about the surface needs the user's attention, but nothing is
// broken and nothing was lost. It wears the transition tone — the same amber that marks a reversible
// in-flight process state — because that is exactly its severity: a gap the user can close, not a
// failure. What separates it from the destructive-text line is whose act was rejected: an action the
// user asked for and did not get is destructive, while a capability that quietly reduced — a save to
// reload, a filesystem watch the OS turned down — is this.
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
    <Alert
      // Alert sets `role="alert"` itself; spreading ours after it is what lets a strip ask for the
      // quieter `status` instead.
      role={urgency}
      // A strip stays identifiable as one whatever politeness it asks for. Addressing it by role
      // instead would make every urgency decision a change to whoever reads the strip back.
      data-advisory-notice
      className={cn("border-status-transition/40 bg-status-transition/10", className)}
    >
      <AlertDescription className="text-foreground">{children}</AlertDescription>
      {action && <AlertAction className="top-1/2 -translate-y-1/2">{action}</AlertAction>}
    </Alert>
  );
}
