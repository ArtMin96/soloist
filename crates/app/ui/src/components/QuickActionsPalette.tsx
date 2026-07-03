import { CommandEmpty, CommandGroup, CommandItem, CommandSeparator } from "@/components/ui/command";
import { CommandPaletteShell } from "@/components/palette/CommandPaletteShell";
import type { PaletteHintData } from "@/components/palette/PaletteFooter";
import { useCommandAction } from "@/components/palette/useCommandAction";
import { ProcessIndicator } from "@/components/ProcessIndicator";
import { runnableProcessActions, type ProcessActionHandlers } from "@/lib/processActions";
import { groupByProject } from "@/store/projects";
import type { ProcessView, ProjectView } from "@/domain";

interface QuickActionsPaletteProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  processes: ProcessView[];
  projects: ProjectView[];
  /** The project currently focused in the app (selected process's project, or last opened). */
  activeProjectId: number | null;
  onStart: (id: number) => void;
  onStop: (id: number) => void;
  onRestart: (id: number) => void;
  onResume: (id: number) => void;
  onTrust: (project: number, name: string) => void;
}

const HINTS: PaletteHintData[] = [
  { keys: "↵", label: "run" },
  { keys: "esc", label: "close" },
];

// Quick actions palette (Ctrl+P): every process in the active project with its status-aware
// actions. By design, "quick actions" is process control for the current project — distinct from
// the command palette (Ctrl+K), which covers app-wide actions. The available actions per process
// come from the single `runnableProcessActions` source, so the gating never diverges
// from the per-process control cluster.
export function QuickActionsPalette({
  open,
  onOpenChange,
  processes,
  projects,
  activeProjectId,
  onStart,
  onStop,
  onRestart,
  onResume,
  onTrust,
}: QuickActionsPaletteProps) {
  const run = useCommandAction(onOpenChange);
  const handlers: ProcessActionHandlers = { onTrust, onResume, onStart, onStop, onRestart };

  const trees = groupByProject(processes, projects, false);
  const activeTree = activeProjectId
    ? trees.find((tree) => tree.project.id === activeProjectId)
    : null;
  const actionable = (activeTree ? activeTree.kinds.flatMap((kind) => kind.processes) : []).flatMap(
    (process) => {
      const actions = runnableProcessActions(process, handlers);
      return actions.length > 0 ? [{ process, actions }] : [];
    },
  );

  return (
    <CommandPaletteShell
      open={open}
      onOpenChange={onOpenChange}
      title="Quick Actions"
      description="Run an action on any process in the active project"
      placeholder="Search actions…"
      hints={HINTS}
      target={activeTree?.project.name}
    >
      {!activeTree && <CommandEmpty>Open a project to see actions.</CommandEmpty>}
      {activeTree && actionable.length === 0 && (
        <CommandEmpty>No actions available in this project.</CommandEmpty>
      )}
      {actionable.map(({ process, actions }, idx) => (
        <div key={process.id}>
          {idx > 0 && <CommandSeparator />}
          <CommandGroup
            heading={
              <span className="flex items-center gap-1.5">
                <ProcessIndicator status={process.status} showLabel={false} />
                {process.label}
              </span>
            }
          >
            {actions.map((action) => (
              <CommandItem
                key={action.kind}
                value={`${process.label} ${action.label} ${process.id}`}
                onSelect={run(action.run)}
              >
                {action.label}
              </CommandItem>
            ))}
          </CommandGroup>
        </div>
      ))}
    </CommandPaletteShell>
  );
}
