import { groupByKind, type ProcessGroup } from "@/store/grouping";
import { isRunning } from "@/lib/status";
import type { ProcessView, ProjectView } from "@/domain";

/** A project's running-vs-total process count, shown as the header's "X/Y" badge. */
export interface RunningCount {
  running: number;
  total: number;
}

/** Running and total counts over a project's processes. */
export function runningCount(processes: ProcessView[]): RunningCount {
  return {
    running: processes.filter((process) => isRunning(process.status)).length,
    total: processes.length,
  };
}

/** One project node in the sidebar: its identity, its non-empty kind subgroups, count. */
export interface ProjectTree {
  project: ProjectView;
  kinds: ProcessGroup[];
  count: RunningCount;
}

// The sidebar's two-level tree: one node per opened project, its processes bucketed into the
// subtype groups with its running count, each group nested by the spawn-lineage map (a worker
// under its lead). Every opened project appears — including one with no processes yet (an empty
// node) — so the user always sees what they opened, not a sidebar that silently stays blank.
// `hideEmptyKinds` (the Sidebar "hide empty sections" setting) drops the subtype groups with no
// processes when set, so a command-only project shows just "Commands"; when clear, all three
// subtype groups always show. Pure: the project read model and the process read model joined
// by id.
export function groupByProject(
  processes: ProcessView[],
  projects: ProjectView[],
  hideEmptyKinds = true,
  parents: ReadonlyMap<number, number> = new Map(),
): ProjectTree[] {
  return projects.map((project) => {
    const own = processes.filter((process) => process.project === project.id);
    const kinds = groupByKind(own, parents);
    return {
      project,
      kinds: hideEmptyKinds ? kinds.filter((group) => group.processes.length > 0) : kinds,
      count: runningCount(own),
    };
  });
}
