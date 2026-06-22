import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ACTIVITY } from "@/lib/activity";
import { STATUS, type StatusDisplay } from "@/lib/status";
import { cn } from "@/lib/utils";
import type { AgentActivity, ProcessView } from "@/domain";

interface ProcessIndicatorProps {
  process: ProcessView;
  /** The agent's current activity, present only while a tracked agent is running. */
  activity?: AgentActivity;
  /** Show the text label beside the glyph (the roomy terminal header). Off in the dense
   *  sidebar, where the label moves into a hover tooltip. */
  showLabel?: boolean;
}

// The product's heartbeat: a process's live state as glyph + colored dot + label. For a
// running agent the state shown is its activity (Working/Thinking/Idle/Permission/Error);
// every other process shows its ProcStatus. One vocabulary either way — the glyph carries the
// state without color, the hue reinforces it, the label names it — so state survives color
// blindness and a grayscale screenshot (DESIGN.md). The `data-status`/`data-activity`
// attribute keys styling and tests off the value, not scraped text.
export function ProcessIndicator({ process, activity, showLabel = true }: ProcessIndicatorProps) {
  const showActivity = process.status === "Running" && activity != null;
  const display: StatusDisplay = showActivity ? ACTIVITY[activity] : STATUS[process.status];
  const dataProps = showActivity
    ? { "data-activity": activity }
    : { "data-status": process.status };

  const glyph = (
    <span
      aria-hidden
      className={cn(
        "text-[0.7rem] leading-none",
        display.toneClass,
        display.transitional && "motion-safe:animate-pulse",
      )}
    >
      {display.glyph}
    </span>
  );

  // Header: glyph + inline label, no tooltip needed (the label is already visible).
  if (showLabel) {
    return (
      <span {...dataProps} className="inline-flex items-center gap-1.5 leading-none">
        {glyph}
        <span className="text-xs text-muted-foreground">{display.label}</span>
      </span>
    );
  }

  // Dense row: glyph only; the label rides a hover tooltip and an always-present sr-only span.
  // `asChild` keeps the trigger a span — no nested button to swallow the row's click or add a
  // tab stop per row.
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span {...dataProps} className="inline-flex items-center leading-none">
          {glyph}
          <span className="sr-only">{display.label}</span>
        </span>
      </TooltipTrigger>
      <TooltipContent>{display.label}</TooltipContent>
    </Tooltip>
  );
}
