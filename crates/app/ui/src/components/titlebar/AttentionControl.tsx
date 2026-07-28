import { useState } from "react";
import { Bell } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ATTENTION_LABEL, attentionCountLabel, attentionEntries } from "@/lib/attention";
import { TOAST } from "@/lib/notifications";
import { cn } from "@/lib/utils";
import type { AttentionSnapshot, ProcessView } from "@/domain";

interface AttentionControlProps {
  snapshot: AttentionSnapshot;
  processes: ProcessView[];
  onSelect: (id: number) => void;
  onClearAll: () => void;
}

// The unread count in the window chrome, and the list behind it. This is the indicator that always
// works: the dock badge silently no-ops on some desktops, so the count here is what a user can rely
// on. It is absent — not a zero — when nothing is waiting, because a control reading zero is a
// standing claim that something might not be.
//
// It carries no drag-region attribute: the title bar starts a window drag on mousedown over any
// element that has one, which would make this a drag handle instead of a button.
export function AttentionControl({
  snapshot,
  processes,
  onSelect,
  onClearAll,
}: AttentionControlProps) {
  const [open, setOpen] = useState(false);
  const entries = attentionEntries(snapshot, processes);

  if (snapshot.total === 0) return null;

  const select = (id: number) => {
    setOpen(false);
    onSelect(id);
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <Tooltip>
        <TooltipTrigger asChild>
          <PopoverTrigger asChild>
            <Button
              variant="ghost"
              size="sm"
              aria-label={ATTENTION_LABEL}
              className="h-7 gap-1.5 px-2 text-muted-foreground hover:text-foreground"
            >
              <Bell aria-hidden className="text-status-attention" />
              <span className="font-mono text-[0.6875rem] tabular-nums text-foreground">
                {attentionCountLabel(snapshot.total)}
              </span>
            </Button>
          </PopoverTrigger>
        </TooltipTrigger>
        <TooltipContent>{ATTENTION_LABEL}</TooltipContent>
      </Tooltip>
      <PopoverContent align="end" className="w-60 gap-1.5">
        <div className="flex items-center justify-between gap-2">
          <span className="text-[0.6875rem] font-[550] tracking-[0.01em] text-muted-foreground">
            {ATTENTION_LABEL}
          </span>
          <Button
            variant="ghost"
            size="sm"
            className="-mr-1 h-6 px-1.5 text-[0.6875rem] font-normal text-muted-foreground hover:text-foreground"
            onClick={onClearAll}
          >
            Clear all
          </Button>
        </div>
        <ul className="flex flex-col">
          {entries.map((entry) => (
            <li key={entry.process}>
              <button
                type="button"
                onClick={() => select(entry.process)}
                className="flex w-full cursor-default items-center gap-2 rounded-md px-1.5 py-1 text-left text-[0.8125rem] outline-none transition-colors duration-[var(--dur-fast)] hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring"
              >
                <span
                  aria-hidden
                  className={cn("text-[0.7rem] leading-none", TOAST[entry.kind].toneClass)}
                >
                  {TOAST[entry.kind].glyph}
                </span>
                <span className="min-w-0 flex-1 truncate">{entry.label}</span>
                {/* The count totals alerts, not processes, so a short list can sit under a much
                    larger number; naming each process's share reconciles the two. */}
                {entry.alerts > 1 && (
                  <span className="font-mono text-[0.6875rem] tabular-nums text-muted-foreground">
                    {entry.alerts}
                  </span>
                )}
              </button>
            </li>
          ))}
        </ul>
      </PopoverContent>
    </Popover>
  );
}
