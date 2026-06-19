import { ProjectGroup } from "@/components/sidebar/ProjectGroup";
import { groupByProject } from "@/store/grouping";
import { useCollapseState } from "@/store/useCollapseState";
import type { ProcessKind, ProcessView, ProjectView } from "@/domain";

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
}: SidebarProps) {
  const trees = groupByProject(processes, projects);
  const [collapsed, setCollapsed] = useCollapseState();
  const projectKey = (id: number) => `project:${id}`;
  const kindKey = (id: number, kind: ProcessKind) => `kind:${id}:${kind}`;

  return (
    <nav
      aria-label="Projects"
      className="flex w-60 shrink-0 flex-col overflow-y-auto border-r bg-sidebar p-2"
    >
      {trees.map((tree, index) => (
        <div key={tree.project.id} className={index > 0 ? "mt-1 border-t pt-1" : undefined}>
          <ProjectGroup
            tree={tree}
            open={!collapsed[projectKey(tree.project.id)]}
            onOpenChange={(open) => setCollapsed(projectKey(tree.project.id), !open)}
            kindOpen={(kind) => !collapsed[kindKey(tree.project.id, kind)]}
            onKindOpenChange={(kind, open) => setCollapsed(kindKey(tree.project.id, kind), !open)}
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
  );
}
