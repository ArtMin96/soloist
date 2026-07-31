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

/** The ProcessKind of the selected process in the given tree, or null. */
export function selectedKind(tree: ProjectTree, selectedId: number): ProcessKind | null {
  return tree.kinds.find((k) => k.processes.some((p) => p.id === selectedId))?.kind ?? null;
}

/** First process across all non-empty kind groups, or null. */
export function firstProcessInTree(tree: ProjectTree): ProcessView | null {
  for (const group of tree.kinds) {
    if (group.processes.length > 0) return group.processes[0];
  }
  return null;
}

/** First process of the given kind in the tree, or null. */
export function firstOfKind(tree: ProjectTree, kind: ProcessKind): ProcessView | null {
  return tree.kinds.find((k) => k.kind === kind)?.processes[0] ?? null;
}
