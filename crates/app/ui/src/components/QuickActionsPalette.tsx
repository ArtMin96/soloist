import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@/components/ui/command";
import { ProcessIndicator } from "@/components/ProcessIndicator";
import { canRestart, canStart, canStop } from "@/lib/status";
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

// Quick actions palette (Ctrl+P): shows all processes in the active project with their
// status-aware actions. Gap decision: "quick actions" = process control for the current
// project — distinct from the command palette (Ctrl+K) which covers app-wide actions.
// Recorded in plan/05 §12.
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
  function run(fn: () => void) {
    fn();
    onOpenChange(false);
  }

  const trees = groupByProject(processes, projects, false);
  const activeTree = activeProjectId ? trees.find((t) => t.project.id === activeProjectId) : null;

  const hasProject = activeTree != null;
  const activeProcesses = activeTree ? activeTree.kinds.flatMap((k) => k.processes) : [];

  return (
    <CommandDialog
      open={open}
      onOpenChange={onOpenChange}
      title="Quick Actions"
      description="Run an action on any process in the active project"
    >
      <Command>
        <CommandInput placeholder="Search actions…" autoFocus />
        <CommandList>
          {!hasProject && <CommandEmpty>Open a project to see actions.</CommandEmpty>}
          {hasProject && activeProcesses.length === 0 && (
            <CommandEmpty>No processes in this project.</CommandEmpty>
          )}
          {hasProject &&
            activeProcesses.map((process, idx) => {
              const actions: { label: string; fn: () => void }[] = [];
              if (process.requires_trust) {
                actions.push({
                  label: "Trust",
                  fn: () => onTrust(process.project, process.label),
                });
              }
              if (canStart(process.status) && !process.requires_trust) {
                if (process.resumable) {
                  actions.push({ label: "Resume", fn: () => onResume(process.id) });
                }
                actions.push({ label: "Start", fn: () => onStart(process.id) });
              }
              if (canStop(process.status)) {
                actions.push({ label: "Stop", fn: () => onStop(process.id) });
              }
              if (canRestart(process.status)) {
                actions.push({ label: "Restart", fn: () => onRestart(process.id) });
              }

              if (actions.length === 0) return null;

              return (
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
                        key={action.label}
                        value={`${process.id}:${process.label}:${action.label}`}
                        onSelect={() => run(action.fn)}
                      >
                        {action.label}
                      </CommandItem>
                    ))}
                  </CommandGroup>
                </div>
              );
            })}
        </CommandList>
      </Command>
    </CommandDialog>
  );
}
