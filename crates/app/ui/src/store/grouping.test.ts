import { describe, expect, it } from "vitest";
import { groupByKind, groupByProject, runningCount } from "@/store/grouping";
import type { ProcessView, ProjectView } from "@/domain";

function process(id: number, kind: ProcessView["kind"], label: string): ProcessView {
  return { id, project: 1, kind, label, status: "Stopped", exit_code: null, requires_trust: false };
}

function withProject(
  base: ProcessView,
  project: number,
  status?: ProcessView["status"],
): ProcessView {
  return { ...base, project, status: status ?? base.status };
}

function projectView(id: number, name: string): ProjectView {
  return { id, name, root: `/p/${name}`, icon: null };
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

describe("runningCount", () => {
  it("counts running over total", () => {
    const count = runningCount([
      withProject(process(1, "Command", "web"), 1, "Running"),
      withProject(process(2, "Command", "api"), 1, "Stopped"),
      withProject(process(3, "Command", "worker"), 1, "Crashed"),
    ]);
    expect(count).toEqual({ running: 1, total: 3 });
  });
});

describe("groupByProject", () => {
  it("nests each project's processes and drops its empty subgroups", () => {
    const processes = [
      withProject(process(1, "Command", "web"), 1, "Running"),
      withProject(process(2, "Command", "api"), 1, "Stopped"),
      withProject(process(3, "Terminal", "shell"), 2, "Running"),
    ];
    const trees = groupByProject(processes, [projectView(1, "app"), projectView(2, "infra")]);

    // One tree per project, in the projects' order.
    expect(trees.map((tree) => tree.project.name)).toEqual(["app", "infra"]);

    // The command-only project shows just "Commands" — no empty Agents/Terminals noise.
    const app = trees[0];
    expect(app.kinds.map((group) => group.kind)).toEqual(["Command"]);
    expect(app.kinds[0].processes.map((p) => p.label)).toEqual(["web", "api"]);
    expect(app.count).toEqual({ running: 1, total: 2 });

    // The second project only owns its own process.
    const infra = trees[1];
    expect(infra.kinds.map((group) => group.kind)).toEqual(["Terminal"]);
    expect(infra.count).toEqual({ running: 1, total: 1 });
  });

  it("omits a project that owns no processes (a stale durable project on launch)", () => {
    const processes = [withProject(process(1, "Command", "web"), 1, "Running")];
    // Project 2 is known (durable) but has no live process this session — it must not
    // render as an empty placeholder node.
    const trees = groupByProject(processes, [projectView(1, "app"), projectView(2, "stale")]);
    expect(trees.map((tree) => tree.project.name)).toEqual(["app"]);
  });
});
