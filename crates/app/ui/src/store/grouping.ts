import type { ProcessKind, ProcessView, ProjectView } from "@/domain";

// The sidebar's fixed group order: Agents first (most attention-worthy), then Terminals,
// then Commands — matching Solo's process tree.
export const GROUP_ORDER: ProcessKind[] = ["Agent", "Terminal", "Command"];

export const GROUP_LABELS: Record<ProcessKind, string> = {
  Agent: "Agents",
  Terminal: "Terminals",
  Command: "Commands",
};

export interface ProcessGroup {
  kind: ProcessKind;
  label: string;
  processes: ProcessView[];
}

// Buckets processes into the three subtype groups, preserving registry order within each
// group and the fixed group order. Pure — no view concerns, unit-testable.
export function groupByKind(processes: ProcessView[]): ProcessGroup[] {
  return GROUP_ORDER.map((kind) => ({
    kind,
    label: GROUP_LABELS[kind],
    processes: processes.filter((process) => process.kind === kind),
  }));
}

/** A project's running-vs-total process count, shown as the header's "X/Y" badge. */
export interface RunningCount {
  running: number;
  total: number;
}

/** Running and total counts over a project's processes. */
export function runningCount(processes: ProcessView[]): RunningCount {
  return {
    running: processes.filter((process) => process.status === "Running").length,
    total: processes.length,
  };
}

/** One project node in the sidebar: its identity, its non-empty kind subgroups, count. */
export interface ProjectTree {
  project: ProjectView;
  kinds: ProcessGroup[];
  count: RunningCount;
}

// The sidebar's two-level tree: each project that owns a live process, with its processes
// bucketed into the subtype groups (empty subgroups dropped, so a command-only project
// shows just "Commands") and its running count. A project with no registered processes is
// omitted — the durable registry persists projects across runs, but the sidebar shows only
// the ones with a live stack this session, never an empty placeholder node. Pure — the
// project read model and the process read model joined by id.
export function groupByProject(processes: ProcessView[], projects: ProjectView[]): ProjectTree[] {
  const trees: ProjectTree[] = [];
  for (const project of projects) {
    const own = processes.filter((process) => process.project === project.id);
    if (own.length === 0) continue;
    trees.push({
      project,
      kinds: groupByKind(own).filter((group) => group.processes.length > 0),
      count: runningCount(own),
    });
  }
  return trees;
}
