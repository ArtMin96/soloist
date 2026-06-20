import { ChevronRight } from "lucide-react";
import { Collapsible } from "radix-ui";
import { ProcessGroup } from "@/components/sidebar/ProcessGroup";
import { ProjectControls } from "@/components/sidebar/ProjectControls";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { monogram, type ProjectTree } from "@/store/projects";
import type { ProcessKind } from "@/domain";

interface ProjectGroupProps {
  tree: ProjectTree;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  kindOpen: (kind: ProcessKind) => boolean;
  onKindOpenChange: (kind: ProcessKind, open: boolean) => void;
  selectedId: number | null;
  onSelect: (id: number) => void;
  onStart: (id: number) => void;
  onStop: (id: number) => void;
  onRestart: (id: number) => void;
  onTrust: (id: number) => void;
  onStartAll: () => void;
  onRestartRunning: () => void;
  onStopAll: () => void;
}

// One project in the sidebar: a collapsible header (icon + name + running count + bulk
// controls) over its non-empty kind subgroups. The project is the top-level context and
// the subtype groups nest under it; empty subgroups are not rendered, so a command-only
// project shows just its commands.
export function ProjectGroup({
  tree,
  open,
  onOpenChange,
  kindOpen,
  onKindOpenChange,
  selectedId,
  onSelect,
  onStart,
  onStop,
  onRestart,
  onTrust,
  onStartAll,
  onRestartRunning,
  onStopAll,
}: ProjectGroupProps) {
  const { project, kinds, count } = tree;

  return (
    <Collapsible.Root open={open} onOpenChange={onOpenChange} className="select-none">
      <div className="group/project flex h-8 items-center gap-1.5 rounded-sm px-1">
        <Collapsible.Trigger className="group/trigger flex min-w-0 flex-1 items-center gap-1.5 rounded-sm py-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-sidebar-ring">
          <ChevronRight
            aria-hidden
            className="size-3 shrink-0 text-muted-foreground transition-transform group-data-[state=open]/trigger:rotate-90"
          />
          <Avatar>
            {project.icon && <AvatarImage src={project.icon} alt="" />}
            <AvatarFallback>{monogram(project.name)}</AvatarFallback>
          </Avatar>
          <span className="min-w-0 flex-1 truncate text-[0.9375rem] font-[550] tracking-[-0.005em] text-foreground">
            {project.name}
          </span>
        </Collapsible.Trigger>
        <span
          className="shrink-0 font-mono text-[0.6875rem] tabular-nums text-muted-foreground/70"
          aria-label={`${count.running} of ${count.total} running`}
        >
          {count.running}/{count.total}
        </span>
        <div className="shrink-0 opacity-0 transition-opacity group-hover/project:opacity-100 group-focus-within/project:opacity-100">
          <ProjectControls
            onStartAll={onStartAll}
            onRestartRunning={onRestartRunning}
            onStopAll={onStopAll}
          />
        </div>
      </div>
      <Collapsible.Content>
        <div className="mt-0.5 flex flex-col gap-0.5 pb-0.5 pl-3">
          {kinds.length === 0 ? (
            <p className="px-1 py-1 text-[0.6875rem] text-muted-foreground/70">No commands yet</p>
          ) : (
            kinds.map((group) => (
              <ProcessGroup
                key={group.kind}
                group={group}
                open={kindOpen(group.kind)}
                onOpenChange={(value) => onKindOpenChange(group.kind, value)}
                selectedId={selectedId}
                onSelect={onSelect}
                onStart={onStart}
                onStop={onStop}
                onRestart={onRestart}
                onTrust={onTrust}
              />
            ))
          )}
        </div>
      </Collapsible.Content>
    </Collapsible.Root>
  );
}
