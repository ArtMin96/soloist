import { useState } from "react";
import { ChevronRight, MoreHorizontal } from "lucide-react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { ProcessGroup } from "@/components/sidebar/ProcessGroup";
import { projectActions, type ProjectAction } from "@/components/sidebar/projectActions";
import { RemoveProjectDialog } from "@/components/sidebar/RemoveProjectDialog";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuGroup,
  ContextMenuItem,
  ContextMenuLabel,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { monogram, type ProjectTree } from "@/store/projects";
import type { ToggleSet } from "@/store/useToggleSet";
import type { ProcessKind } from "@/domain";

interface ProjectGroupProps {
  tree: ProjectTree;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  kindOpen: (kind: ProcessKind) => boolean;
  onKindOpenChange: (kind: ProcessKind, open: boolean) => void;
  collapsedLeads: ToggleSet;
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
  onRemoveProject: () => void;
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
  collapsedLeads,
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
  onRemoveProject,
}: ProjectGroupProps) {
  const { project, kinds, count } = tree;
  // The menus only *open* the confirm; the removal itself runs solely from the dialog's
  // destructive action, so a destructive menu click can never remove anything by itself.
  const [confirmRemove, setConfirmRemove] = useState(false);
  const actions = projectActions({
    onStartAll,
    onRestartRunning,
    onStopAll,
    onOpenOrchestration,
    onOpenProjectSettings,
    onRemoveProject: () => setConfirmRemove(true),
  });

  return (
    <Collapsible open={open} onOpenChange={onOpenChange} className="select-none">
      <ContextMenu>
        <ContextMenuTrigger asChild>
          <div className="group/project flex h-8 items-center gap-1.5 rounded-md px-1">
            <CollapsibleTrigger className="group/trigger flex min-w-0 flex-1 items-center gap-1.5 rounded-md py-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-sidebar-ring">
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
            </CollapsibleTrigger>
            {/* Count and menu share one trailing cell. The transparent menu trigger therefore
                never reserves a second button-width or makes the count look inset. */}
            <div
              className="relative grid h-6 min-w-6 shrink-0 place-items-center"
              style={{ gridTemplateAreas: "'trailing'" }}
            >
              <span
                style={{ gridArea: "trailing" }}
                className="justify-self-end font-mono text-[0.6875rem] tabular-nums text-muted-foreground transition-opacity group-hover/project:opacity-0 group-focus-within/project:opacity-0"
                aria-label={`${count.running} of ${count.total} processes running`}
                title={`${count.running} of ${count.total} processes running`}
              >
                {count.running}/{count.total}
              </span>
              <DropdownMenu>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <DropdownMenuTrigger asChild>
                      <Button
                        variant="ghost"
                        size="icon-xs"
                        aria-label={`Actions for ${project.name}`}
                        className="pointer-events-none opacity-0 transition-opacity group-hover/project:pointer-events-auto group-hover/project:opacity-100 group-focus-within/project:pointer-events-auto group-focus-within/project:opacity-100 focus-visible:pointer-events-auto focus-visible:opacity-100 data-[state=open]:pointer-events-auto data-[state=open]:opacity-100 motion-reduce:transition-none"
                        style={{ gridArea: "trailing" }}
                      >
                        <MoreHorizontal data-icon="inline-start" />
                      </Button>
                    </DropdownMenuTrigger>
                  </TooltipTrigger>
                  <TooltipContent>Actions for {project.name}</TooltipContent>
                </Tooltip>
                <DropdownMenuContent align="end" className="w-52">
                  <DropdownMenuLabel>{project.name}</DropdownMenuLabel>
                  <DropdownMenuSeparator />
                  <DropdownMenuGroup>
                    {actions.bulk.map((action) => (
                      <DropdownMenuItem key={action.id} onSelect={action.run}>
                        <ActionIcon action={action} />
                        {action.label}
                      </DropdownMenuItem>
                    ))}
                  </DropdownMenuGroup>
                  <DropdownMenuSeparator />
                  <DropdownMenuGroup>
                    {actions.views.map((action) => (
                      <DropdownMenuItem key={action.id} onSelect={action.run}>
                        <ActionIcon action={action} />
                        {action.label}
                      </DropdownMenuItem>
                    ))}
                  </DropdownMenuGroup>
                  <DropdownMenuSeparator />
                  <DropdownMenuGroup>
                    {actions.danger.map((action) => (
                      <DropdownMenuItem key={action.id} variant="destructive" onSelect={action.run}>
                        <ActionIcon action={action} />
                        {action.label}
                      </DropdownMenuItem>
                    ))}
                  </DropdownMenuGroup>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          </div>
        </ContextMenuTrigger>
        <ContextMenuContent className="w-52">
          <ContextMenuLabel>{project.name}</ContextMenuLabel>
          <ContextMenuSeparator />
          <ContextMenuGroup>
            {actions.bulk.map((action) => (
              <ContextMenuItem key={action.id} onSelect={action.run}>
                <ActionIcon action={action} />
                {action.label}
              </ContextMenuItem>
            ))}
          </ContextMenuGroup>
          <ContextMenuSeparator />
          <ContextMenuGroup>
            {actions.views.map((action) => (
              <ContextMenuItem key={action.id} onSelect={action.run}>
                <ActionIcon action={action} />
                {action.label}
              </ContextMenuItem>
            ))}
          </ContextMenuGroup>
          <ContextMenuSeparator />
          <ContextMenuGroup>
            {actions.danger.map((action) => (
              <ContextMenuItem key={action.id} variant="destructive" onSelect={action.run}>
                <ActionIcon action={action} />
                {action.label}
              </ContextMenuItem>
            ))}
          </ContextMenuGroup>
        </ContextMenuContent>
      </ContextMenu>
      <RemoveProjectDialog
        open={confirmRemove}
        onOpenChange={setConfirmRemove}
        projectName={project.name}
        runningCount={count.running}
        onConfirm={onRemoveProject}
      />
      <CollapsibleContent className="overflow-hidden data-[state=open]:animate-disclose-down data-[state=closed]:animate-disclose-up">
        <div className="mt-0.5 flex flex-col gap-0.5 pb-0.5 pl-3">
          {count.total === 0 ? (
            <p className="px-1 py-1 text-[0.6875rem] text-muted-foreground">No processes yet</p>
          ) : (
            kinds.map((group) => (
              <ProcessGroup
                key={group.kind}
                group={group}
                open={kindOpen(group.kind)}
                onOpenChange={(value) => onKindOpenChange(group.kind, value)}
                collapsedLeads={collapsedLeads}
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
      </CollapsibleContent>
    </Collapsible>
  );
}

// Renders a project action's icon; the menu components size and space the svg.
function ActionIcon({ action }: { action: ProjectAction }) {
  const { Icon } = action;
  return <Icon aria-hidden />;
}
