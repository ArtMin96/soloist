import { ProcessControls } from "@/components/ProcessControls";
import { ProcessIndicator } from "@/components/ProcessIndicator";
import { ProcessMeta } from "@/components/sidebar/ProcessMeta";
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
  onResume: () => void;
  onTrust: () => void;
}

// One process in the tree: status dot + name, with per-row controls revealed on hover or
// focus (always shown for the selected row, and for an untrusted command so its trust
// affordance stays visible). The selected row is an azure-tinted rounded fill — the macOS
// source-list selection — while hover stays a quiet neutral; status hues keep their full
// saturation on either, so the heartbeat never loses contrast to the selection.
export function ProcessRow({
  process,
  selected,
  onSelect,
  onStart,
  onStop,
  onRestart,
  onResume,
  onTrust,
}: ProcessRowProps) {
  const { metrics, attempt, activity } = useSignal(process.id);
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
        "group/row relative flex h-7 cursor-default items-center gap-2 rounded-md pr-1 pl-2.5 text-[0.8125rem] outline-none",
        "transition-colors duration-[var(--dur-select)] ease-out-quint",
        "focus-visible:ring-2 focus-visible:ring-sidebar-ring",
        selected
          ? "bg-[var(--sidebar-sel-fill)] font-medium hover:bg-[var(--sidebar-sel-fill-hover)]"
          : "hover:bg-sidebar-accent focus-visible:bg-sidebar-accent",
      )}
    >
      <ProcessIndicator status={process.status} activity={activity} showLabel={false} />
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
            "pointer-events-none transition-opacity duration-[var(--dur-fast)]",
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
            "pointer-events-none translate-x-1 opacity-0 transition-[opacity,transform] duration-[var(--dur-fast)] ease-out-quint",
            "group-hover/row:pointer-events-auto group-hover/row:translate-x-0 group-hover/row:opacity-100",
            "group-focus-within/row:pointer-events-auto group-focus-within/row:translate-x-0 group-focus-within/row:opacity-100",
            showControls && "pointer-events-auto translate-x-0 opacity-100",
          )}
        >
          <ProcessControls
            status={process.status}
            size="icon-xs"
            onStart={onStart}
            onStop={onStop}
            onRestart={onRestart}
            resumable={process.resumable}
            onResume={onResume}
            requiresTrust={process.requires_trust}
            onTrust={onTrust}
          />
        </div>
      </div>
    </div>
  );
}
