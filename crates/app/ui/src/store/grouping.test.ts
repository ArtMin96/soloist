import { describe, expect, it } from "vitest";
import { groupByKind } from "@/store/grouping";
import type { ProcessView } from "@/domain";

function process(id: number, kind: ProcessView["kind"], label: string): ProcessView {
  return {
    id,
    project: 1,
    kind,
    label,
    status: "Stopped",
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated",
  };
}

describe("groupByKind", () => {
  it("buckets processes into the three groups in fixed order", () => {
    const groups = groupByKind([
      process(1, "Command", "web"),
      process(2, "Agent", "assistant"),
      process(3, "Terminal", "shell"),
    ]);
    expect(groups.map((group) => group.kind)).toEqual(["Agent", "Terminal", "Command"]);
    expect(groups.map((group) => group.label)).toEqual(["Agents", "Terminals", "Commands"]);
  });

  it("preserves registry order within a group", () => {
    const groups = groupByKind([process(1, "Command", "web"), process(2, "Command", "build")]);
    const commands = groups.find((group) => group.kind === "Command");
    expect(commands?.processes.map((process) => process.label)).toEqual(["web", "build"]);
  });

  it("returns empty groups when a subtype has no processes", () => {
    const groups = groupByKind([process(1, "Terminal", "shell")]);
    expect(groups.find((group) => group.kind === "Agent")?.processes).toEqual([]);
  });

  it("keeps every row a root when no lineage is supplied", () => {
    const groups = groupByKind([process(1, "Agent", "lead"), process(2, "Agent", "worker")]);
    const agents = groups.find((group) => group.kind === "Agent");
    expect(agents?.roots.map((root) => root.process.label)).toEqual(["lead", "worker"]);
    expect(agents?.roots.every((root) => root.children.length === 0)).toBe(true);
  });

  it("nests a worker under its lead within the same group", () => {
    const groups = groupByKind(
      [process(1, "Agent", "lead"), process(2, "Agent", "worker")],
      new Map([[2, 1]]),
    );
    const agents = groups.find((group) => group.kind === "Agent");
    expect(agents?.roots.map((root) => root.process.label)).toEqual(["lead"]);
    expect(agents?.roots[0]?.children.map((child) => child.process.label)).toEqual(["worker"]);
  });

  it("lists rows depth-first so counts and nav follow the visual order", () => {
    const groups = groupByKind(
      [process(1, "Agent", "lead"), process(3, "Agent", "solo"), process(2, "Agent", "worker")],
      new Map([[2, 1]]),
    );
    const agents = groups.find((group) => group.kind === "Agent");
    expect(agents?.processes.map((row) => row.label)).toEqual(["lead", "worker", "solo"]);
  });

  it("re-roots a worker whose parent is absent from the group", () => {
    const groups = groupByKind([process(2, "Agent", "worker")], new Map([[2, 1]]));
    const agents = groups.find((group) => group.kind === "Agent");
    expect(agents?.roots.map((root) => root.process.label)).toEqual(["worker"]);
  });

  // A lead is always an agent, so a parent of another kind is data that should not exist. It must
  // still leave every group describing itself: the worker stays in the group that renders it, and
  // no group counts or holds a row of another kind.
  it("keeps a worker in its own group when its recorded parent is of another kind", () => {
    const groups = groupByKind(
      [process(1, "Terminal", "shell"), process(2, "Agent", "worker")],
      new Map([[2, 1]]),
    );
    const terminals = groups.find((group) => group.kind === "Terminal");
    expect(terminals?.roots.map((root) => root.process.label)).toEqual(["shell"]);
    expect(terminals?.roots[0]?.children).toEqual([]);
    expect(terminals?.processes.map((row) => row.label)).toEqual(["shell"]);
    const agents = groups.find((group) => group.kind === "Agent");
    expect(agents?.roots.map((root) => root.process.label)).toEqual(["worker"]);
    expect(agents?.processes.map((row) => row.label)).toEqual(["worker"]);
  });

  it("lists only its own kind in every group, however deep the lineage runs", () => {
    const groups = groupByKind(
      [
        process(1, "Terminal", "shell"),
        process(2, "Agent", "lead"),
        process(3, "Agent", "worker"),
        process(4, "Command", "web"),
      ],
      new Map([
        [2, 1],
        [3, 2],
      ]),
    );
    expect(groups.map((group) => group.processes.map((row) => row.kind))).toEqual([
      ["Agent", "Agent"],
      ["Terminal"],
      ["Command"],
    ]);
    // The lead's own worker still nests; only the edge that leaves the group is dropped.
    const agents = groups.find((group) => group.kind === "Agent");
    expect(agents?.roots.map((root) => root.process.label)).toEqual(["lead"]);
    expect(agents?.roots[0]?.children.map((child) => child.process.label)).toEqual(["worker"]);
  });

  it("treats a self-referential edge as a root", () => {
    const groups = groupByKind([process(1, "Agent", "loop")], new Map([[1, 1]]));
    const agents = groups.find((group) => group.kind === "Agent");
    expect(agents?.roots.map((root) => root.process.label)).toEqual(["loop"]);
  });
});
