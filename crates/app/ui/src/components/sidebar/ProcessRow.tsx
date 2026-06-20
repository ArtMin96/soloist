import { ProcessControls } from "@/components/ProcessControls";
import { ProcessMeta } from "@/components/sidebar/ProcessMeta";
import { StatusIndicator } from "@/components/StatusIndicator";
import { cn } from "@/lib/utils";
import { useSignal } from "@/store/signalsContext";
import type { ProcessView } from "@/domain";

interface ProcessRowProps {
  process: ProcessView;
  selected: boolean;
  onSelect: () => void;
  onStart: () => void;
  onStop: () => void;
  onRestart: () => void;
  onTrust: () => void;
}

// One process in the tree: status dot + name, with per-row controls revealed on hover or
// focus (always shown for the selected row, and for an untrusted command so its trust
// affordance stays visible). The selected row carries a full-height azure marker — a
// selection affordance, not a decorative side-stripe.
export function ProcessRow({
  process,
  selected,
  onSelect,
  onStart,
  onStop,
  onRestart,
  onTrust,
}: ProcessRowProps) {
  const { metrics, attempt } = useSignal(process.id);
  // Controls are always present for the selected row and for an untrusted command (so its
  // trust affordance stays visible); otherwise they reveal on hover/focus, replacing the
  // at-rest telemetry.
  const showControls = selected || process.requires_trust;
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
      {/* The right zone stacks at-rest telemetry under the controls in one grid cell, so the
          cell keeps the width of whichever is wider and the name never reflows on hover. */}
      <div
        className="relative grid shrink-0 items-center justify-items-end"
        style={{ gridTemplateAreas: "'stack'" }}
      >
        <div
          style={{ gridArea: "stack" }}
          className={cn(
            "pointer-events-none transition-opacity",
            "group-hover/row:opacity-0 group-focus-within/row:opacity-0",
            showControls && "opacity-0",
          )}
        >
          <ProcessMeta
            status={process.status}
            ready={process.ready}
            ports={process.ports}
            metrics={metrics}
            attempt={attempt}
          />
        </div>
        <div
          style={{ gridArea: "stack" }}
          className={cn(
            "pointer-events-none opacity-0 transition-opacity",
            "group-hover/row:pointer-events-auto group-hover/row:opacity-100",
            "group-focus-within/row:pointer-events-auto group-focus-within/row:opacity-100",
            showControls && "pointer-events-auto opacity-100",
          )}
        >
          <ProcessControls
            status={process.status}
            size="icon-xs"
            onStart={onStart}
            onStop={onStop}
            onRestart={onRestart}
            requiresTrust={process.requires_trust}
            onTrust={onTrust}
          />
        </div>
      </div>
    </div>
  );
}
