import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { TOAST } from "@/lib/notifications";
import { cn } from "@/lib/utils";
import type { AttentionKind } from "@/domain";

export interface ProcessToastProps {
  kind: AttentionKind;
  /** The alert's first line, as the core wrote it. */
  title: string;
  /** The alert's second line, as the core wrote it. */
  body: string;
  /** Show the process this alert came from. */
  onOpen: () => void;
  onDismiss: () => void;
}

// One in-app alert. The words arrive already written — the same sentence the desktop would have
// shown — so this renders them and adds only what a toast needs: the kind's glyph, a way to reach
// the process, and a way to be rid of it.
//
// The card is the whole surface: nothing inside it is boxed again.
export function ProcessToast({ kind, title, body, onOpen, onDismiss }: ProcessToastProps) {
  const display = TOAST[kind];

  return (
    <div className="flex w-full items-start gap-2 rounded-lg border border-border bg-popover p-2.5 text-popover-foreground shadow-overlay">
      <span
        aria-hidden
        className={cn("mt-[0.3125rem] text-[0.625rem] leading-none", display.toneClass)}
      >
        {display.glyph}
      </span>
      <button
        type="button"
        onClick={onOpen}
        className="-my-0.5 min-w-0 flex-1 cursor-default rounded-md px-1.5 py-1 text-left transition-colors duration-[var(--dur-fast)] outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring/50 dark:hover:bg-muted/50"
      >
        <span className="block text-[0.8125rem] leading-[1.35] font-[550]">{title}</span>
        <span className="mt-0.5 block text-[0.8125rem] leading-[1.45] text-muted-foreground">
          {body}
        </span>
      </button>
      <Button
        variant="ghost"
        size="icon-xs"
        aria-label="Dismiss"
        onClick={onDismiss}
        className="-mt-0.5 shrink-0"
      >
        <X />
      </Button>
    </div>
  );
}
