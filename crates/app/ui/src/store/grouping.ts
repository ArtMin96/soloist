import type { ProcessKind, ProcessView } from "@/domain";

// The sidebar's fixed group order: Agents first (most attention-worthy), then Terminals,
// then Commands — matching Solo's process tree.
const GROUP_ORDER: ProcessKind[] = ["Agent", "Terminal", "Command"];

// The plural heading for a group of one kind — the sidebar's section headings, and the launch
// picker's, so the two surfaces name the same group identically.
export const GROUP_LABELS: Record<ProcessKind, string> = {
  Agent: "Agents",
  Terminal: "Terminals",
  Command: "Commands",
};

// The singular noun for one process's kind — used where a single process is labelled (e.g. a
// palette row badge), as opposed to the plural section headings above. One source so no surface
// emits a bare `ProcessKind` token.
export const KIND_LABELS: Record<ProcessKind, string> = {
  Agent: "Agent",
  Terminal: "Terminal",
  Command: "Command",
};

/** One process in a group's lineage tree: its row plus the workers nested beneath it. */
export interface ProcessNode {
  process: ProcessView;
  children: ProcessNode[];
}

export interface ProcessGroup {
  kind: ProcessKind;
  label: string;
  /** The group's rows in visual (depth-first) order — counts and keyboard nav read this. */
  processes: ProcessView[];
  /**
   * The same rows nested by spawn lineage. A root is a process with no live lead; it carries its
   * whole subtree, so a root's workers can be of another kind than the group.
   */
  roots: ProcessNode[];
}

// Nests processes by the child→parent lineage map, across kinds: a worker nests under the lead
// that spawned it whatever kind that lead is. A child whose parent is absent from the list, or
// self-referential, re-roots rather than disappearing. Order is preserved at every level.
function nestByLineage(
  processes: ProcessView[],
  parents: ReadonlyMap<number, number>,
): ProcessNode[] {
  const byId = new Map<number, ProcessNode>(
    processes.map((process) => [process.id, { process, children: [] }]),
  );
  const roots: ProcessNode[] = [];
  for (const process of processes) {
    const node = byId.get(process.id);
    if (!node) continue;
    const parentId = parents.get(process.id);
    const parent = parentId != null && parentId !== process.id ? byId.get(parentId) : undefined;
    if (parent) parent.children.push(node);
    else roots.push(node);
  }
  return roots;
}

/** The tree's rows in visual order — what a flat consumer (counts, nav) iterates. */
function flatten(roots: ProcessNode[]): ProcessView[] {
  const rows: ProcessView[] = [];
  const walk = (nodes: ProcessNode[]) => {
    for (const node of nodes) {
      rows.push(node.process);
      walk(node.children);
    }
  };
  walk(roots);
  return rows;
}

// Nests every process by spawn lineage, then files each root subtree under the subtype group its
// root belongs to, in the fixed group order and preserving registry order within each. A subtree
// renders whole, so an agent spawned from a terminal nests under that terminal rather than sitting
// flat — the same shape the orchestration tree shows. A group's `processes` is exactly the rows it
// renders, so its count and keyboard nav describe what is on screen. With no lineage every node is
// a root in its own kind's group and `processes` keeps the flat registry order. Pure — no view
// concerns, unit-testable. The project tier (which project owns which processes) is the projects
// module's concern; this is purely the process-kind grouping used within a project node.
export function groupByKind(
  processes: ProcessView[],
  parents: ReadonlyMap<number, number> = new Map(),
): ProcessGroup[] {
  const roots = nestByLineage(processes, parents);
  return GROUP_ORDER.map((kind) => {
    const owned = roots.filter((root) => root.process.kind === kind);
    return {
      kind,
      label: GROUP_LABELS[kind],
      processes: flatten(owned),
      roots: owned,
    };
  });
}
