import type { ProjectTree } from "@/store/projects";
import type { ProcessKind, ProcessView } from "@/domain";

/** The tree that contains the selected process, or null when nothing is selected. */
export function findSelectedTree(
  trees: ProjectTree[],
  selectedId: number | null,
): ProjectTree | null {
  if (selectedId === null) return null;
  return (
    trees.find((tree) => tree.kinds.some((k) => k.processes.some((p) => p.id === selectedId))) ??
    null
  );
}

/**
 * The kind of the subtype group that renders the selected process, or null. A worker nested under
 * a lead of another kind renders in the lead's group, so this is the group section navigation
 * moves from — not necessarily the process's own kind.
 */
export function selectedGroupKind(tree: ProjectTree, selectedId: number): ProcessKind | null {
  return tree.kinds.find((k) => k.processes.some((p) => p.id === selectedId))?.kind ?? null;
}

/** First process across all non-empty kind groups, or null. */
export function firstProcessInTree(tree: ProjectTree): ProcessView | null {
  for (const group of tree.kinds) {
    if (group.processes.length > 0) return group.processes[0];
  }
  return null;
}

/**
 * First rendered row of the given kind in the tree, or null. Scans the rows each group renders
 * rather than the groups themselves, so a worker nested under a lead of another kind is still
 * reachable by a jump to its own kind.
 */
export function firstOfKind(tree: ProjectTree, kind: ProcessKind): ProcessView | null {
  for (const group of tree.kinds) {
    const match = group.processes.find((process) => process.kind === kind);
    if (match) return match;
  }
  return null;
}
