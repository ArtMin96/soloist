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
  /** The same rows nested by spawn lineage; every node in the tree is of the group's kind. */
  roots: ProcessNode[];
}

// Nests one group's members by the child→parent lineage map. Resolution is scoped to the members
// handed in, so a child whose parent is absent, self-referential, or of another kind re-roots
// rather than disappearing — a group can never come to hold a row it does not own. Order is
// preserved at every level.
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

// Buckets processes into the three subtype groups, preserving registry order within each group and
// the fixed group order, and nests each group's rows by the spawn-lineage map (a worker under the
// lead that spawned it — a lead is always an agent, so in practice only Agents ever nest). Nesting
// resolves inside a group, so a group's `processes` is exactly the rows it renders and every one of
// them is of its own kind: its count matches what is on screen, and no section can be emptied by a
// row that renders elsewhere. With no lineage every node is a root and `processes` keeps the flat
// registry order. Pure — no view concerns, unit-testable. The project tier (which project owns
// which processes) is the projects module's concern; this is purely the process-kind grouping used
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
