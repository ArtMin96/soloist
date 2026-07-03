import type { ProcessKind, ProcessView } from "@/domain";

// The sidebar's fixed group order: Agents first (most attention-worthy), then Terminals,
// then Commands — matching Solo's process tree.
const GROUP_ORDER: ProcessKind[] = ["Agent", "Terminal", "Command"];

const GROUP_LABELS: Record<ProcessKind, string> = {
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
  /** The same rows nested by spawn lineage; a node whose parent is outside this group is a root. */
  roots: ProcessNode[];
}

// Nests one group's members by the child→parent lineage map. Scoped to the group: an edge
// resolves only when both ends are in the same group, so a child whose parent is absent,
// self-referential, or in another subtype group re-roots flat rather than disappearing.
// Order is preserved at every level.
function nestByLineage(
  members: ProcessView[],
  parents: ReadonlyMap<number, number>,
): ProcessNode[] {
  const byId = new Map<number, ProcessNode>(
    members.map((process) => [process.id, { process, children: [] }]),
  );
  const roots: ProcessNode[] = [];
  for (const process of members) {
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

// Buckets processes into the three subtype groups, preserving registry order within each
// group and the fixed group order, and nests each group's rows by the spawn-lineage map
// (worker under its lead — in practice only Agents ever nest, since both ends of a lineage
// edge are agents). With no lineage every node is a root and `processes` keeps today's flat
// order. Pure — no view concerns, unit-testable. The project tier (which project owns which
// processes) is the projects module's concern; this is purely the process-kind grouping used
// within a project node.
export function groupByKind(
  processes: ProcessView[],
  parents: ReadonlyMap<number, number> = new Map(),
): ProcessGroup[] {
  return GROUP_ORDER.map((kind) => {
    const roots = nestByLineage(
      processes.filter((process) => process.kind === kind),
      parents,
    );
    return {
      kind,
      label: GROUP_LABELS[kind],
      processes: flatten(roots),
      roots,
    };
  });
}
