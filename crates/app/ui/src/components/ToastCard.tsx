import { XIcon } from "lucide-react";
import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { GLASS_FLOATING_SURFACE } from "@/components/ui/glass";
import { cn } from "@/lib/utils";

const DISMISS_LABEL = "Dismiss";

/**
 * The card every in-app alert is drawn on: a mark, what it says, and a way to be rid of it.
 *
 * One card rather than one per kind of alert, so a crashed process and a refused git command cannot
 * drift into two different-looking notifications. The card is the whole surface — nothing inside it
 * is boxed again — and what goes in the middle is the caller's, because a process alert leads
 * somewhere and a refusal only reports.
 */
export function ToastCard({
  mark,
  role,
  onDismiss,
  children,
}: {
  /** The leading glyph or icon, in whatever vocabulary the alert belongs to. */
  mark: ReactNode;
  /** `"alert"` where what the card reports is a failure the reader has to be told about now. */
  role?: "alert";
  onDismiss: () => void;
  children: ReactNode;
}) {
  return (
    <div
      role={role}
      className={cn(
        "flex w-full items-start gap-2 rounded-lg border border-border bg-message-surface p-2.5 text-message-foreground",
        GLASS_FLOATING_SURFACE,
      )}
    >
      {mark}
      {children}
      <Button
        variant="ghost"
        size="icon-xs"
        aria-label={DISMISS_LABEL}
        onClick={onDismiss}
        className="-mt-0.5 shrink-0 text-message-action-foreground hover:bg-message-action-hover"
      >
        <XIcon />
      </Button>
    </div>
  );
}

/** An alert's two lines, as whoever raised it wrote them: what happened, then the detail. */
export function ToastLines({ title, body }: { title: string; body: string }) {
  return (
    <>
      <span className="block type-body font-[550]">{title}</span>
      <span className="mt-0.5 block type-body text-message-foreground/80">{body}</span>
    </>
  );
}
