import { Settings } from "lucide-react";
import { ProjectGroup } from "@/components/sidebar/ProjectGroup";
import { useSidebarHotkeys } from "@/components/sidebar/useSidebarHotkeys";
import { Button } from "@/components/ui/button";
import { groupByProject, kindCollapseKey, projectCollapseKey } from "@/store/projects";
import { useCollapseState } from "@/store/useCollapseState";
import { useSidebarSettings } from "@/store/sidebarSettingsContext";
import type { ProcessView, ProjectView } from "@/domain";

interface SidebarProps {
  projects: ProjectView[];
  processes: ProcessView[];
  selectedId: number | null;
  onSelect: (id: number) => void;
  onStart: (id: number) => void;
  onStop: (id: number) => void;
  onRestart: (id: number) => void;
  onResume: (id: number) => void;
  onTrust: (id: number) => void;
  onStartAll: (project: number) => void;
  onRestartRunning: (project: number) => void;
  onStopAll: (project: number) => void;
  onOpenSettings: () => void;
  onOpenProjectSettings: (projectId: number) => void;
  onOpenOrchestration: (projectId: number) => void;
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
  onResume,
  onTrust,
  onStartAll,
  onRestartRunning,
  onStopAll,
  onOpenSettings,
  onOpenProjectSettings,
  onOpenOrchestration,
}: SidebarProps) {
  const { sidebar } = useSidebarSettings();
  const trees = groupByProject(processes, projects, sidebar.hide_empty_sections);
  const [collapsed, setCollapsed] = useCollapseState();
  const handleNavKeyDown = useSidebarHotkeys({
    trees,
    selectedId,
    setCollapsed,
    onSelect,
    onRestart,
  });

  return (
    <div className="flex w-64 shrink-0 flex-col border-r bg-sidebar">
      <nav
        aria-label="Projects"
        className="min-h-0 flex-1 overflow-y-auto p-2"
        tabIndex={0}
        onKeyDown={handleNavKeyDown}
      >
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
              onResume={onResume}
              onTrust={onTrust}
              onStartAll={() => onStartAll(tree.project.id)}
              onRestartRunning={() => onRestartRunning(tree.project.id)}
              onStopAll={() => onStopAll(tree.project.id)}
              onOpenProjectSettings={() => onOpenProjectSettings(tree.project.id)}
              onOpenOrchestration={() => onOpenOrchestration(tree.project.id)}
            />
          </div>
        ))}
      </nav>
      {sidebar.show_settings_footer && (
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
      )}
    </div>
  );
}
