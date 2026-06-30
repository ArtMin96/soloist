import { CommandEmpty, CommandGroup, CommandItem, CommandSeparator } from "@/components/ui/command";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { CommandPaletteShell } from "@/components/palette/CommandPaletteShell";
import { ProcessCommandItem } from "@/components/palette/ProcessCommandItem";
import { useCommandAction } from "@/components/palette/useCommandAction";
import type { PaletteHintData } from "@/components/palette/PaletteFooter";
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

const HINTS: PaletteHintData[] = [
  { keys: "↵", label: "jump" },
  { keys: "esc", label: "close" },
];

// Quick-jump palette (Ctrl+E): fuzzy search across all processes and open projects. Processes are
// grouped under their project; the project line itself is also selectable — it opens that project's
// settings. Todos and scratchpads are out of scope here (they need a per-project async fetch not
// pre-loaded at the App level; recorded in plan/05 §12 + KNOWN-DIVERGENCES).
export function QuickJumpPalette({
  open,
  onOpenChange,
  processes,
  projects,
  onSelectProcess,
  onSelectProject,
}: QuickJumpPaletteProps) {
  const run = useCommandAction(onOpenChange);
  const trees = groupByProject(processes, projects, false);

  return (
    <CommandPaletteShell
      open={open}
      onOpenChange={onOpenChange}
      title="Quick Jump"
      description="Jump to any process or project"
      placeholder="Jump to…"
      hints={HINTS}
    >
      {projects.length === 0 && <CommandEmpty>No destinations found.</CommandEmpty>}
      {trees.map((tree, idx) => (
        <div key={tree.project.id}>
          {idx > 0 && <CommandSeparator />}
          <CommandGroup heading={tree.project.name}>
            <CommandItem
              value={`${tree.project.name} project settings ${tree.project.id}`}
              onSelect={run(() => onSelectProject(tree.project.id))}
              className="gap-2"
            >
              <Avatar>
                {tree.project.icon && <AvatarImage src={tree.project.icon} alt="" />}
                <AvatarFallback>{monogram(tree.project.name)}</AvatarFallback>
              </Avatar>
              <span className="flex-1 truncate">{tree.project.name}</span>
              <span className="text-[0.6875rem] text-muted-foreground">Project settings</span>
            </CommandItem>
            {tree.kinds
              .flatMap((kind) => kind.processes)
              .map((process) => (
                <ProcessCommandItem
                  key={process.id}
                  process={process}
                  projectName={tree.project.name}
                  onSelect={run(() => onSelectProcess(process.id))}
                />
              ))}
          </CommandGroup>
        </div>
      ))}
      {projects.length > 0 && processes.length === 0 && (
        <CommandEmpty>No destinations found.</CommandEmpty>
      )}
    </CommandPaletteShell>
  );
}
