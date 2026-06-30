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
import { Kbd } from "@/components/ui/kbd";
import { ProcessIndicator } from "@/components/ProcessIndicator";
import { groupByProject, monogram } from "@/store/projects";
import type { ProcessView, ProjectView } from "@/domain";

interface QuickJumpPaletteProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  processes: ProcessView[];
  projects: ProjectView[];
  onSelectProcess: (id: number) => void;
  onSelectProject: (id: number) => void;
}

// Kind labels kept short for the dense palette item.
const KIND_LABEL: Record<string, string> = {
  Agent: "Agent",
  Terminal: "Terminal",
  Command: "Command",
};

// Quick-jump palette (Ctrl+E): fuzzy search across all processes and open projects.
// Processes are grouped under their project; the project line itself is also selectable —
// it opens that project's settings. Todos and scratchpads are out of scope here
// (they require a per-project async fetch not pre-loaded at the App level).
export function QuickJumpPalette({
  open,
  onOpenChange,
  processes,
  projects,
  onSelectProcess,
  onSelectProject,
}: QuickJumpPaletteProps) {
  function jump(fn: () => void) {
    fn();
    onOpenChange(false);
  }

  const trees = groupByProject(processes, projects, false);
  const hasContent = projects.length > 0;

  return (
    <CommandDialog
      open={open}
      onOpenChange={onOpenChange}
      title="Quick Jump"
      description="Jump to any process or project"
    >
      <Command>
        <CommandInput placeholder="Jump to…" autoFocus />
        <CommandList>
          {!hasContent && <CommandEmpty>No destinations found.</CommandEmpty>}
          {trees.map((tree, idx) => {
            const allProcesses = tree.kinds.flatMap((k) => k.processes);
            return (
              <div key={tree.project.id}>
                {idx > 0 && <CommandSeparator />}
                <CommandGroup heading={tree.project.name}>
                  {/* Project row — opens project settings */}
                  <CommandItem
                    value={`project:${tree.project.id}:${tree.project.name}`}
                    onSelect={() => jump(() => onSelectProject(tree.project.id))}
                    className="gap-2"
                  >
                    <span
                      className="flex size-4 shrink-0 items-center justify-center rounded text-[0.625rem] font-semibold bg-muted text-muted-foreground"
                      aria-hidden
                    >
                      {tree.project.icon ? (
                        <img
                          src={tree.project.icon}
                          alt=""
                          className="size-4 rounded object-cover"
                        />
                      ) : (
                        monogram(tree.project.name)
                      )}
                    </span>
                    <span className="flex-1 truncate">{tree.project.name}</span>
                    <span className="text-[0.6875rem] text-muted-foreground">Project settings</span>
                  </CommandItem>
                  {/* Process rows */}
                  {allProcesses.map((process) => (
                    <CommandItem
                      key={process.id}
                      value={`process:${process.id}:${process.label}:${tree.project.name}`}
                      onSelect={() => jump(() => onSelectProcess(process.id))}
                      className="gap-2"
                    >
                      <ProcessIndicator status={process.status} showLabel={false} />
                      <span className="flex-1 truncate">{process.label}</span>
                      <span className="rounded-full bg-muted px-1.5 py-0.5 text-[0.625rem] text-muted-foreground">
                        {KIND_LABEL[process.kind]}
                      </span>
                    </CommandItem>
                  ))}
                </CommandGroup>
              </div>
            );
          })}
          {hasContent && processes.length === 0 && (
            <CommandEmpty>No destinations found.</CommandEmpty>
          )}
        </CommandList>
        <div className="flex items-center gap-3 border-t px-3 py-2 text-xs text-muted-foreground">
          <span className="flex items-center gap-1.5">
            <Kbd>↵</Kbd>
            jump
          </span>
          <span className="flex items-center gap-1.5">
            <Kbd>esc</Kbd>
            close
          </span>
        </div>
      </Command>
    </CommandDialog>
  );
}
