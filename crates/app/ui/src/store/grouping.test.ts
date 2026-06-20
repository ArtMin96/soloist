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
    ports: [],
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
});
