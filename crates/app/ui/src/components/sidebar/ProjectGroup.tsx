import { ChevronRight, MoreHorizontal } from "lucide-react";
import { Collapsible } from "radix-ui";
import { ProcessGroup } from "@/components/sidebar/ProcessGroup";
import { projectActions, type ProjectAction } from "@/components/sidebar/projectActions";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
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
  onResume: (id: number) => void;
  onTrust: (id: number) => void;
  onStartAll: () => void;
  onRestartRunning: () => void;
  onStopAll: () => void;
  onOpenProjectSettings: () => void;
  onOpenOrchestration: () => void;
}

// One project in the sidebar source list: a collapsible header (disclosure + icon + name +
// running count) over its non-empty kind subgroups. The project name is the header's job and
// always stays fully visible; every project action lives in the ••• menu (revealed on
// hover/focus) and the row's right-click menu — both driven by one `projectActions` source,
// so the name never competes with a row of buttons. Empty subgroups are not rendered.
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
  onResume,
  onTrust,
  onStartAll,
  onRestartRunning,
  onStopAll,
  onOpenProjectSettings,
  onOpenOrchestration,
}: ProjectGroupProps) {
  const { project, kinds, count } = tree;
  const actions = projectActions({
    onStartAll,
    onRestartRunning,
    onStopAll,
    onOpenOrchestration,
    onOpenProjectSettings,
  });

  return (
    <Collapsible.Root open={open} onOpenChange={onOpenChange} className="select-none">
      <ContextMenu>
        <ContextMenuTrigger asChild>
          <div className="group/project flex h-8 items-center gap-1.5 rounded-md px-1">
            <Collapsible.Trigger className="group/trigger flex min-w-0 flex-1 items-center gap-1.5 rounded-md py-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-sidebar-ring">
              <ChevronRight
                aria-hidden
                className="size-3 shrink-0 text-muted-foreground transition-transform duration-[var(--dur-control)] ease-spring-settle group-data-[state=open]/trigger:rotate-90"
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
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  aria-label={`${project.name} actions`}
                  className="shrink-0 opacity-0 transition-opacity group-hover/project:opacity-100 group-focus-within/project:opacity-100 focus-visible:opacity-100 data-[state=open]:opacity-100 motion-reduce:transition-none"
                >
                  <MoreHorizontal />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-52">
                {actions.bulk.map((action) => (
                  <DropdownMenuItem key={action.id} onSelect={action.run}>
                    <ActionIcon action={action} />
                    {action.label}
                  </DropdownMenuItem>
                ))}
                <DropdownMenuSeparator />
                {actions.views.map((action) => (
                  <DropdownMenuItem key={action.id} onSelect={action.run}>
                    <ActionIcon action={action} />
                    {action.label}
                  </DropdownMenuItem>
                ))}
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </ContextMenuTrigger>
        <ContextMenuContent className="w-52">
          {actions.bulk.map((action) => (
            <ContextMenuItem key={action.id} onSelect={action.run}>
              <ActionIcon action={action} />
              {action.label}
            </ContextMenuItem>
          ))}
          <ContextMenuSeparator />
          {actions.views.map((action) => (
            <ContextMenuItem key={action.id} onSelect={action.run}>
              <ActionIcon action={action} />
              {action.label}
            </ContextMenuItem>
          ))}
        </ContextMenuContent>
      </ContextMenu>
      <Collapsible.Content className="overflow-hidden data-[state=open]:animate-disclose-down data-[state=closed]:animate-disclose-up">
        <div className="mt-0.5 flex flex-col gap-0.5 pb-0.5 pl-3">
          {count.total === 0 ? (
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
                onResume={onResume}
                onTrust={onTrust}
              />
            ))
          )}
        </div>
      </Collapsible.Content>
    </Collapsible.Root>
  );
}

// Renders a project action's icon; the menu components size and space the svg.
function ActionIcon({ action }: { action: ProjectAction }) {
  const { Icon } = action;
  return <Icon aria-hidden />;
}
