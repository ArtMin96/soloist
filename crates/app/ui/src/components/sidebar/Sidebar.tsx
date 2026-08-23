import { useState } from "react";
import { House, Settings } from "lucide-react";
import { ProjectGroup } from "@/components/sidebar/ProjectGroup";
import { useSidebarHotkeys } from "@/components/sidebar/useSidebarHotkeys";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { SortableItem, SortableList } from "@/components/SortableList";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import {
  filterSidebar,
  groupByProject,
  kindCollapseKey,
  projectCollapseKey,
} from "@/store/projects";
import { useCollapseState } from "@/store/useCollapseState";
import { useSidebarSettings } from "@/store/sidebarSettingsContext";
import { useToggleSet } from "@/store/useToggleSet";
import type { ProcessActionHandlers } from "@/lib/processActions";
import type { ProjectTree } from "@/store/projects";
import type { ProcessView, ProjectView } from "@/domain";

// What a rearranged project is called when a move is announced.
function nameOfProject(trees: ProjectTree[], id: string): string {
  return trees.find((tree) => String(tree.project.id) === id)?.project.name ?? id;
}

interface SidebarProps {
  projects: ProjectView[];
  processes: ProcessView[];
  /** The live spawn-lineage map (worker id → lead id); workers nest under their leads. */
  lineage: ReadonlyMap<number, number>;
  selectedId: number | null;
  onSelect: (id: number) => void;
  handlers: ProcessActionHandlers;
  onStartAll: (project: number) => void;
  onRestartRunning: (project: number) => void;
  onStopAll: (project: number) => void;
  onOpenStart: () => void;
  startActive: boolean;
  onOpenSettings: () => void;
  onOpenProjectSettings: (projectId: number) => void;
  onOpenOrchestration: (projectId: number) => void;
  onRemoveProject: (projectId: number) => void;
  onReorderProjects: (order: number[]) => void;
}

// The process tree, grouped by project: each opened project is a collapsible node over its
// subtype subgroups, with spawned workers nested under their lead inside a subgroup. It renders
// the read model and raises intent; the store owns the data and the core owns the behaviour.
// Collapse state persists per project and per subgroup; a lead's collapse is in-session only
// (per-run ids must never persist).
export function Sidebar({
  projects,
  processes,
  lineage,
  selectedId,
  onSelect,
  handlers,
  onStartAll,
  onRestartRunning,
  onStopAll,
  onOpenStart,
  startActive,
  onOpenSettings,
  onOpenProjectSettings,
  onOpenOrchestration,
  onRemoveProject,
  onReorderProjects,
}: SidebarProps) {
  const { sidebar } = useSidebarSettings();
  const [filter, setFilter] = useState("");
  // The filter only narrows the tree while its input is shown; hiding the input restores the full
  // list (there is then no way to change the query).
  const query = sidebar.show_filter_input ? filter : "";
  const filtering = query.trim().length > 0;
  const visible = filterSidebar(processes, projects, query);
  const trees = groupByProject(
    visible.processes,
    visible.projects,
    sidebar.hide_empty_sections,
    lineage,
  );
  const [collapsed, setCollapsed] = useCollapseState();
  const collapsedLeads = useToggleSet();
  const handleNavKeyDown = useSidebarHotkeys({
    trees,
    selectedId,
    setCollapsed,
    onSelect,
    onRestart: handlers.onRestart,
  });

  return (
    <div className="@container/sidebar flex w-64 shrink-0 flex-col border-r border-sidebar-border bg-sidebar text-sidebar-foreground [--muted-foreground:var(--theme-sidebar-muted-foreground)]">
      {sidebar.show_filter_input && (
        <div className="border-b border-sidebar-border p-2">
          <Input
            type="search"
            value={filter}
            onChange={(event) => setFilter(event.target.value)}
            placeholder="Filter processes…"
            aria-label="Filter processes"
            className="h-7 border-sidebar-border bg-sidebar-control-surface text-[0.8125rem] placeholder:text-sidebar-muted-foreground"
          />
        </div>
      )}
      <nav
        aria-label="Projects"
        className="min-h-0 flex-1 overflow-y-auto p-2"
        tabIndex={0}
        onKeyDown={handleNavKeyDown}
      >
        <SortableList
          ids={trees.map((tree) => String(tree.project.id))}
          onReorder={(order) => onReorderProjects(order.map(Number))}
          nameOf={(id) => nameOfProject(trees, id)}
          // A filtered tree is only part of the list, and an order arranged from part of a list
          // is not the user's answer for the whole of it.
          disabled={filtering}
        >
          {trees.map((tree, index) => (
            <SortableItem
              key={tree.project.id}
              id={String(tree.project.id)}
              className={index > 0 ? "mt-1 border-t pt-1" : undefined}
            >
              {({ handleProps, isDragging }) => (
                <ProjectGroup
                  tree={tree}
                  dragHandleProps={filtering ? undefined : handleProps}
                  dragging={isDragging}
                  open={!collapsed[projectCollapseKey(tree.project.id)]}
                  onOpenChange={(open) => setCollapsed(projectCollapseKey(tree.project.id), !open)}
                  kindOpen={(kind) => !collapsed[kindCollapseKey(tree.project.id, kind)]}
                  onKindOpenChange={(kind, open) =>
                    setCollapsed(kindCollapseKey(tree.project.id, kind), !open)
                  }
                  collapsedLeads={collapsedLeads}
                  selectedId={selectedId}
                  onSelect={onSelect}
                  handlers={handlers}
                  onStartAll={() => onStartAll(tree.project.id)}
                  onRestartRunning={() => onRestartRunning(tree.project.id)}
                  onStopAll={() => onStopAll(tree.project.id)}
                  onOpenProjectSettings={() => onOpenProjectSettings(tree.project.id)}
                  onOpenOrchestration={() => onOpenOrchestration(tree.project.id)}
                  onRemoveProject={() => onRemoveProject(tree.project.id)}
                />
              )}
            </SortableItem>
          ))}
        </SortableList>
      </nav>
      <div className="flex items-center gap-1 border-t border-sidebar-border p-2">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon-sm"
              className={cn(
                "text-muted-foreground hover:text-foreground",
                startActive && "bg-sidebar-accent text-foreground",
              )}
              aria-label="Start page"
              aria-current={startActive ? "page" : undefined}
              onClick={onOpenStart}
            >
              <House />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="right">Start</TooltipContent>
        </Tooltip>
        {sidebar.show_settings_footer && (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon-sm"
                className="text-muted-foreground hover:text-foreground"
                aria-label="Settings"
                onClick={onOpenSettings}
              >
                <Settings />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="right">Settings</TooltipContent>
          </Tooltip>
        )}
      </div>
    </div>
  );
}
