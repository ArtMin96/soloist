import { describe, expect, it } from "vitest";
import { buildOrchestrationTree } from "@/store/orchestrationTree";
import type { AgentNode } from "@/domain";

function node(id: number, label: string, parent: number | null = null): AgentNode {
  return { id, parent, label, kind: "Agent", status: "Running", activity: "Working" };
}

describe("buildOrchestrationTree", () => {
  it("nests a worker under the lead that spawned it", () => {
    const tree = buildOrchestrationTree([node(1, "lead"), node(2, "worker", 1)]);
    expect(tree).toHaveLength(1);
    expect(tree[0].id).toBe(1);
    expect(tree[0].children.map((child) => child.id)).toEqual([2]);
  });

  it("keeps a manually launched agent (no parent) at the root", () => {
    const tree = buildOrchestrationTree([node(1, "lead"), node(2, "solo")]);
    expect(tree.map((root) => root.id)).toEqual([1, 2]);
    expect(tree.every((root) => root.children.length === 0)).toBe(true);
  });

  it("preserves snapshot order at every level", () => {
    const tree = buildOrchestrationTree([
      node(1, "lead"),
      node(2, "worker-a", 1),
      node(3, "worker-b", 1),
    ]);
    expect(tree[0].children.map((child) => child.label)).toEqual(["worker-a", "worker-b"]);
  });

  it("re-roots a node whose parent is absent from the snapshot", () => {
    // The lead (id 1) has left the registry; its worker still references it. The worker must
    // surface as a root rather than vanish into a missing parent.
    const tree = buildOrchestrationTree([node(2, "worker", 1)]);
    expect(tree.map((root) => root.id)).toEqual([2]);
    expect(tree[0].children).toEqual([]);
  });

  it("re-parents a worker to root across the lead's lifecycle", () => {
    // A sequence of successive snapshots: lead alone, then lead with a worker nested, then the
    // lead closed — the worker re-roots without being stranded.
    const leadAlone = buildOrchestrationTree([node(1, "lead")]);
    expect(leadAlone.map((root) => root.id)).toEqual([1]);

    const leadWithWorker = buildOrchestrationTree([node(1, "lead"), node(2, "worker", 1)]);
    expect(leadWithWorker[0].children.map((child) => child.id)).toEqual([2]);

    const leadClosed = buildOrchestrationTree([node(2, "worker", 1)]);
    expect(leadClosed.map((root) => root.id)).toEqual([2]);
  });

  it("renders an empty tree for a project with no agents", () => {
    expect(buildOrchestrationTree([])).toEqual([]);
  });
});
