import { Settings } from "lucide-react";
import { ProjectGroup } from "@/components/sidebar/ProjectGroup";
import { Button } from "@/components/ui/button";
import { groupByProject, kindCollapseKey, projectCollapseKey } from "@/store/projects";
import { useCollapseState } from "@/store/useCollapseState";
import type { ProcessView, ProjectView } from "@/domain";

interface SidebarProps {
  projects: ProjectView[];
  processes: ProcessView[];
  selectedId: number | null;
  onSelect: (id: number) => void;
  onStart: (id: number) => void;
  onStop: (id: number) => void;
  onRestart: (id: number) => void;
  onTrust: (id: number) => void;
  onStartAll: (project: number) => void;
  onRestartRunning: (project: number) => void;
  onStopAll: (project: number) => void;
  onOpenSettings: () => void;
}

// The process tree, grouped by project: each opened project is a collapsible node over its
// subtype subgroups. It renders the read model and raises intent; the store owns the data
// and the core owns the behaviour. Collapse state persists per project and per subgroup.
export function Sidebar({
  projects,
  processes,
  selectedId,
  onSelect,
  onStart,
  onStop,
  onRestart,
  onTrust,
  onStartAll,
  onRestartRunning,
  onStopAll,
  onOpenSettings,
}: SidebarProps) {
  const trees = groupByProject(processes, projects);
  const [collapsed, setCollapsed] = useCollapseState();

  return (
    <div className="flex w-60 shrink-0 flex-col border-r bg-sidebar">
      <nav aria-label="Projects" className="min-h-0 flex-1 overflow-y-auto p-2">
        {trees.map((tree, index) => (
          <div key={tree.project.id} className={index > 0 ? "mt-1 border-t pt-1" : undefined}>
            <ProjectGroup
              tree={tree}
              open={!collapsed[projectCollapseKey(tree.project.id)]}
              onOpenChange={(open) => setCollapsed(projectCollapseKey(tree.project.id), !open)}
              kindOpen={(kind) => !collapsed[kindCollapseKey(tree.project.id, kind)]}
              onKindOpenChange={(kind, open) =>
                setCollapsed(kindCollapseKey(tree.project.id, kind), !open)
              }
              selectedId={selectedId}
              onSelect={onSelect}
              onStart={onStart}
              onStop={onStop}
              onRestart={onRestart}
              onTrust={onTrust}
              onStartAll={() => onStartAll(tree.project.id)}
              onRestartRunning={() => onRestartRunning(tree.project.id)}
              onStopAll={() => onStopAll(tree.project.id)}
            />
          </div>
        ))}
      </nav>
      <div className="border-t border-sidebar-border p-2">
        <Button
          variant="ghost"
          size="sm"
          className="w-full justify-start gap-2 px-2 text-muted-foreground hover:text-foreground"
          onClick={onOpenSettings}
        >
          <Settings />
          Settings
        </Button>
      </div>
    </div>
  );
}
