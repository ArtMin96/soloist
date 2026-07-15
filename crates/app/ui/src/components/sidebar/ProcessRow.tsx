import { ChevronRight } from "lucide-react";
import { ProcessControls } from "@/components/ProcessControls";
import { ProcessIndicator } from "@/components/ProcessIndicator";
import { ProcessMeta } from "@/components/sidebar/ProcessMeta";
import { PROCESS_CPU_FLOOR, PROCESS_MEM_FLOOR } from "@/lib/sidebar";
import { cn } from "@/lib/utils";
import { useSignal } from "@/store/signalsContext";
import { useSidebarSettings } from "@/store/sidebarSettingsContext";
import type { ProcessView } from "@/domain";

/** The row's base left padding; each lineage level indents one step further. */
const ROW_BASE_PADDING_PX = 10;
const ROW_INDENT_STEP_PX = 16;

interface ProcessRowProps {
  process: ProcessView;
  selected: boolean;
  onSelect: () => void;
  onStart: () => void;
  onStop: () => void;
  onRestart: () => void;
  onResume: () => void;
  onTrust: () => void;
  /** The row's lineage depth within its group; roots sit at 0. */
  depth?: number;
  /** Whether the row's group reserves a disclosure column (some row in it has workers). */
  treeColumn?: boolean;
  /** Whether this row has nested workers of its own — it then owns a disclosure chevron. */
  hasChildren?: boolean;
  expanded?: boolean;
  onToggleExpand?: () => void;
}

// One process in the tree: status dot + name, with per-row controls revealed on hover or
// focus (always shown for the selected row, and for an untrusted command so its trust
// affordance stays visible). The selected row is an azure-tinted rounded fill — the macOS
// source-list selection — while hover stays a quiet neutral; status hues keep their full
// saturation on either, so the heartbeat never loses contrast to the selection. When the
// row's group carries spawn lineage it becomes a tree row: a fixed disclosure column keeps
// the status dots aligned, a lead's chevron collapses its workers, and each level indents
// one step.
export function ProcessRow({
  process,
  selected,
  onSelect,
  onStart,
  onStop,
  onRestart,
  onResume,
  onTrust,
  depth = 0,
  treeColumn = false,
  hasChildren = false,
  expanded = true,
  onToggleExpand,
}: ProcessRowProps) {
  const { metrics, restart, activity } = useSignal(process.id);
  const { sidebar } = useSidebarSettings();
  // Controls are always present for the selected row and for an untrusted command (so its
  // trust affordance stays visible); otherwise they reveal on hover/focus, replacing the
  // at-rest telemetry.
  const showControls = selected || process.requires_trust;
  return (
    <div
      role="treeitem"
      aria-selected={selected}
      aria-level={depth + 1}
      aria-expanded={treeColumn && hasChildren ? expanded : undefined}
      tabIndex={0}
      data-process-id={process.id}
      onClick={onSelect}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onSelect();
        }
      }}
      style={{ paddingLeft: `${ROW_BASE_PADDING_PX + depth * ROW_INDENT_STEP_PX}px` }}
      className={cn(
        "group/row relative flex h-7 cursor-default items-center gap-2 rounded-md pr-1 text-[0.8125rem] outline-none",
        "transition-colors duration-[var(--dur-select)] ease-out-quint",
        "focus-visible:ring-2 focus-visible:ring-sidebar-ring",
        selected
          ? "bg-[var(--sidebar-sel-fill)] font-medium hover:bg-[var(--sidebar-sel-fill-hover)]"
          : "hover:bg-sidebar-accent focus-visible:bg-sidebar-accent",
      )}
    >
      {treeColumn &&
        (hasChildren ? (
          <button
            type="button"
            aria-label={
              expanded ? `Collapse ${process.label}'s workers` : `Expand ${process.label}'s workers`
            }
            onClick={(event) => {
              event.stopPropagation();
              onToggleExpand?.();
            }}
            onKeyDown={(event) => {
              // The button handles its own activation; don't let it bubble into row-select.
              if (event.key === "Enter" || event.key === " ") event.stopPropagation();
            }}
            className="flex size-4 shrink-0 items-center justify-center rounded-sm text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-sidebar-ring"
          >
            <ChevronRight
              aria-hidden
              className={cn(
                "size-3 transition-transform duration-[var(--dur-control)] ease-spring-settle",
                expanded && "rotate-90",
              )}
            />
          </button>
        ) : (
          <span aria-hidden className="size-4 shrink-0" />
        ))}
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
            restart={restart}
            cpuFloor={PROCESS_CPU_FLOOR[sidebar.process_cpu_threshold]}
            memFloor={PROCESS_MEM_FLOOR[sidebar.process_mem_threshold]}
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
