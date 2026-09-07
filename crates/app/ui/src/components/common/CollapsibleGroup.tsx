import type { ReactNode } from "react";
import { ChevronRight } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { buttonVariants } from "@/components/ui/button";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { cn } from "@/lib/utils";

/** The DOM handle a group's trigger carries, valued with the group's label. */
export const GROUP_ATTRIBUTE = "data-group";

interface CollapsibleGroupProps {
  label: string;
  count: number;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  children: ReactNode;
}

/**
 * One collapsible section of a list. The header is the same small sentence-case label with a
 * monospace count the sidebar's process groups wear — deliberately not a tracked-uppercase eyebrow,
 * and deliberately the same vocabulary, so a grouped list reads identically wherever it appears in
 * the app. Every group looks alike, including the residual one: a row that belongs to no named
 * group is ordinary, so its section carries no warning colour and no nudge to fill it in.
 *
 * Purely a wrapper: the rows inside are unchanged, which is what keeps a collapsed row the same
 * element the board, the keyboard, and the end-to-end walks already address. Open state belongs to
 * the board, so a group's collapse can persist without this holding any.
 */
export function CollapsibleGroup({
  label,
  count,
  open,
  onOpenChange,
  children,
}: CollapsibleGroupProps) {
  return (
    <Collapsible open={open} onOpenChange={onOpenChange} className="select-none">
      <CollapsibleTrigger
        {...{ [GROUP_ATTRIBUTE]: label }}
        aria-label={`${label}, ${count} ${count === 1 ? "item" : "items"}`}
        className={cn(
          buttonVariants({ variant: "ghost", size: "sm" }),
          "group/collapsible-group h-8 w-full min-w-0 justify-start gap-2 px-2 text-left",
        )}
      >
        <ChevronRight
          aria-hidden
          className="size-3.5 text-muted-foreground transition-transform duration-[var(--dur-control)] ease-spring-settle group-data-[state=open]/collapsible-group:rotate-90 motion-reduce:transition-none"
        />
        <span className="type-label min-w-0 truncate font-[550] tracking-[var(--tracking-label)] text-foreground">
          {label}
        </span>
        <span aria-hidden className="h-px min-w-3 flex-1 bg-border" />
        <Badge
          aria-hidden
          variant="muted"
          className="h-5 min-w-6 rounded-md px-1.5 font-mono tabular-nums"
        >
          {count}
        </Badge>
      </CollapsibleTrigger>
      <CollapsibleContent className="overflow-hidden data-[state=open]:animate-disclose-down data-[state=closed]:animate-disclose-up">
        {children}
      </CollapsibleContent>
    </Collapsible>
  );
}
