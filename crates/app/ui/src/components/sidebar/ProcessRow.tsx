import { ProcessControls } from "@/components/ProcessControls";
import { StatusIndicator } from "@/components/StatusIndicator";
import { cn } from "@/lib/utils";
import type { ProcessView } from "@/domain";

interface ProcessRowProps {
  process: ProcessView;
  selected: boolean;
  onSelect: () => void;
  onStart: () => void;
  onStop: () => void;
  onRestart: () => void;
}

// One process in the tree: status dot + name, with per-row controls revealed on hover or
// focus (always shown for the selected row). The selected row carries a full-height azure
// marker — a selection affordance, not a decorative side-stripe.
export function ProcessRow({
  process,
  selected,
  onSelect,
  onStart,
  onStop,
  onRestart,
}: ProcessRowProps) {
  return (
    <div
      role="option"
      aria-selected={selected}
      tabIndex={0}
      data-process-id={process.id}
      onClick={onSelect}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onSelect();
        }
      }}
      className={cn(
        "group/row relative flex h-7 cursor-default items-center gap-2 rounded-sm pr-1 pl-2.5 text-[0.8125rem] outline-none",
        "hover:bg-sidebar-accent focus-visible:bg-sidebar-accent focus-visible:ring-2 focus-visible:ring-sidebar-ring",
        selected && "bg-sidebar-accent",
      )}
    >
      {selected && (
        <span
          aria-hidden
          className="absolute top-1 bottom-1 left-0 w-0.5 rounded-full bg-sidebar-primary"
        />
      )}
      <StatusIndicator status={process.status} showLabel={false} />
      <span className="min-w-0 flex-1 truncate">{process.label}</span>
      <div
        className={cn(
          "shrink-0 opacity-0 transition-opacity",
          "group-hover/row:opacity-100 group-focus-within/row:opacity-100",
          selected && "opacity-100",
        )}
      >
        <ProcessControls
          status={process.status}
          size="icon-xs"
          onStart={onStart}
          onStop={onStop}
          onRestart={onRestart}
        />
      </div>
    </div>
  );
}
