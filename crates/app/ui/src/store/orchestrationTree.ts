import type { AgentNode } from "@/domain";

// One node in the rendered orchestration tree: an agent-lineage node with the workers it spawned
// nested beneath it. The core re-roots a node whose parent has left the registry, so a `parent`
// here normally resolves to a sibling — the builder stays defensive anyway: an unresolved or
// self-referential parent is treated as a root, so a node can never be stranded.
export interface OrchestrationTreeNode extends AgentNode {
  children: OrchestrationTreeNode[];
}

// Builds the nested lead→worker tree from the flat snapshot, preserving the snapshot's order at
// every level (the core emits agents in stable registry order, so the tree never reshuffles
// between renders). A node whose `parent` is null — or names a process absent from the snapshot —
// is a root.
export function buildOrchestrationTree(agents: AgentNode[]): OrchestrationTreeNode[] {
  const byId = new Map<number, OrchestrationTreeNode>();
  for (const agent of agents) {
    byId.set(agent.id, { ...agent, children: [] });
  }
  const roots: OrchestrationTreeNode[] = [];
  for (const agent of agents) {
    const node = byId.get(agent.id)!;
    const parent = agent.parent != null ? byId.get(agent.parent) : undefined;
    if (parent && parent.id !== node.id) {
      parent.children.push(node);
    } else {
      roots.push(node);
    }
  }
  return roots;
}
