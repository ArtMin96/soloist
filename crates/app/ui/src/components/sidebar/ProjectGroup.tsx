import { Fragment, useState, type ComponentType, type ReactNode } from "react";
import { ChevronRight, MoreHorizontal } from "lucide-react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { ProcessGroup } from "@/components/sidebar/ProcessGroup";
import { projectActions, type ProjectActionSection } from "@/components/sidebar/projectActions";
import { RemoveProjectDialog } from "@/components/sidebar/RemoveProjectDialog";
import { WatchLimitNotice } from "@/components/sidebar/WatchLimitNotice";
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
import { useSortableList } from "@/components/useSortableList";
import { ATTENTION_LABEL } from "@/lib/attention";
import type { ProcessActionHandlers } from "@/lib/processActions";
import { cn } from "@/lib/utils";
import { useUnreadProject } from "@/store/attentionContext";
import { useWatchLimit } from "@/store/watchContext";
import { monogram, type ProjectTree } from "@/store/projects";
import type { ToggleSet } from "@/store/useToggleSet";
import type { ProcessKind } from "@/domain";

/** One place toward the top of the list, and one place toward its bottom. */
const MOVE_TOWARD_TOP = -1;
const MOVE_TOWARD_BOTTOM = 1;

/** The group/item/separator primitives one project menu needs, whichever family renders it. */
interface MenuParts {
  Group: ComponentType<{ children: ReactNode }>;
  Item: ComponentType<{
    variant?: "default" | "destructive";
    onSelect: () => void;
    children: ReactNode;
  }>;
  Separator: ComponentType;
}

const DROPDOWN_PARTS: MenuParts = {
  Group: DropdownMenuGroup,
  Item: DropdownMenuItem,
  Separator: DropdownMenuSeparator,
};

const CONTEXT_PARTS: MenuParts = {
  Group: ContextMenuGroup,
  Item: ContextMenuItem,
  Separator: ContextMenuSeparator,
};

interface ProjectGroupProps {
  tree: ProjectTree;
  /** Spread onto the header row so the project is dragged by its whole line, not by a grip. */
  dragHandleProps?: Record<string, unknown>;
  /** True while this project is the one being dragged. */
  dragging?: boolean;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  kindOpen: (kind: ProcessKind) => boolean;
  onKindOpenChange: (kind: ProcessKind, open: boolean) => void;
  collapsedLeads: ToggleSet;
  selectedId: number | null;
  onSelect: (id: number) => void;
  handlers: ProcessActionHandlers;
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
  dragHandleProps,
  dragging,
  open,
  onOpenChange,
  kindOpen,
  onKindOpenChange,
  collapsedLeads,
  selectedId,
  onSelect,
  handlers,
  onStartAll,
  onRestartRunning,
  onStopAll,
  onOpenProjectSettings,
  onOpenOrchestration,
  onRemoveProject,
}: ProjectGroupProps) {
  const { project, kinds, count } = tree;
  const unread = useUnreadProject(project.id);
  const watchLimits = useWatchLimit(project.id);
  // The menus only *open* the confirm; the removal itself runs solely from the dialog's
  // destructive action, so a destructive menu click can never remove anything by itself.
  const [confirmRemove, setConfirmRemove] = useState(false);
  // Outside an arrangeable list — the design harness stands a row up on its own — there is nowhere
  // to move to, so the row simply offers no move.
  const list = useSortableList();
  const id = String(project.id);
  const move = (delta: number) =>
    list?.canMoveItemBy(id, delta) ? () => list.moveItemBy(id, delta) : null;
  const sections = projectActions({
    onStartAll,
    onRestartRunning,
    onStopAll,
    onOpenOrchestration,
    onOpenProjectSettings,
    onRemoveProject: () => setConfirmRemove(true),
    onMoveUp: move(MOVE_TOWARD_TOP),
    onMoveDown: move(MOVE_TOWARD_BOTTOM),
  });

  return (
    <Collapsible open={open} onOpenChange={onOpenChange} className="select-none">
      <ContextMenu>
        <ContextMenuTrigger asChild>
          {/* The whole line is the drag handle — there is no grip to find, and no part of the
              row that refuses to move. The press only becomes a drag once it travels, so the
              disclosure and the ••• menu inside it keep taking their clicks. */}
          <div
            {...dragHandleProps}
            className={cn(
              "group/project flex h-8 items-center gap-1.5 rounded-md px-1",
              dragHandleProps && "cursor-grab active:cursor-grabbing",
              dragging && "cursor-grabbing",
            )}
          >
            <CollapsibleTrigger className="group/trigger flex min-w-0 flex-1 items-center gap-1.5 rounded-md py-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-sidebar-ring">
              <ChevronRight
                aria-hidden
                className="size-3 shrink-0 text-muted-foreground transition-transform duration-[var(--dur-control)] ease-spring-settle group-data-[state=open]/trigger:rotate-90"
              />
              {/* The dot badges the project's own icon rather than riding the trailing cell,
                  which swaps the running count for the ••• menu on hover — a marker there would
                  vanish exactly when the pointer arrived. */}
              <span className="relative shrink-0">
                <Avatar>
                  {project.icon && <AvatarImage src={project.icon} alt="" />}
                  <AvatarFallback>{monogram(project.name)}</AvatarFallback>
                </Avatar>
                {unread && (
                  <span
                    role="img"
                    aria-label={ATTENTION_LABEL}
                    className="absolute -top-0.5 -right-0.5 size-[7px] rounded-full bg-status-attention ring-2 ring-sidebar"
                  />
                )}
              </span>
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
                  <ProjectActionSections sections={sections} parts={DROPDOWN_PARTS} />
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          </div>
        </ContextMenuTrigger>
        <ContextMenuContent className="w-52">
          <ContextMenuLabel>{project.name}</ContextMenuLabel>
          <ProjectActionSections sections={sections} parts={CONTEXT_PARTS} />
        </ContextMenuContent>
      </ContextMenu>
      {/* Outside the collapsible content on purpose: a project whose watches are limited must say so
          whether or not its processes are unfolded, since the loss is one nothing else reveals. */}
      {watchLimits && <WatchLimitNotice limits={watchLimits} className="mt-1 ml-3" />}
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
                handlers={handlers}
              />
            ))
          )}
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}

// One project menu's sections, rendered behind the group/item/separator primitives of whichever
// family (dropdown or context menu) is asking — the single body both menus share, so they can
// never drift apart. A separator precedes every section, including the first, since the label
// above it is the caller's, not this renderer's.
function ProjectActionSections({
  sections,
  parts,
}: {
  sections: ProjectActionSection[];
  parts: MenuParts;
}) {
  const { Group, Item, Separator } = parts;
  return (
    <>
      {sections.map((section) => (
        <Fragment key={section.id}>
          <Separator />
          <Group>
            {section.actions.map((action) => (
              <Item
                key={action.id}
                variant={section.destructive ? "destructive" : "default"}
                onSelect={action.run}
              >
                <action.Icon aria-hidden />
                {action.label}
              </Item>
            ))}
          </Group>
        </Fragment>
      ))}
    </>
  );
}
