import { describe, expect, it } from "vitest";
import { filterSidebar, groupByProject, runningCount } from "@/store/projects/tree";
import type { ProcessView, ProjectView } from "@/domain";

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

  it("keeps every subtype group when empty sections are not hidden", () => {
    const processes = [withProject(process(1, "Command", "web"), 1, "Running")];
    // hideEmptyKinds = false (the "hide empty sections" setting off): a command-only project
    // still shows Agents and Terminals, in the fixed order.
    const trees = groupByProject(processes, [projectView(1, "app")], false);
    expect(trees[0].kinds.map((group) => group.kind)).toEqual(["Agent", "Terminal", "Command"]);
    expect(trees[0].kinds.map((group) => group.processes.length)).toEqual([0, 0, 1]);
  });

  it("shows an opened project with no processes as an empty node", () => {
    const processes = [withProject(process(1, "Command", "web"), 1, "Running")];
    // Project 2 is opened but owns no live process yet — it still appears (so the user sees
    // what they opened), with no subgroups and a zero count.
    const trees = groupByProject(processes, [projectView(1, "app"), projectView(2, "empty")]);
    expect(trees.map((tree) => tree.project.name)).toEqual(["app", "empty"]);
    const empty = trees[1];
    expect(empty.kinds).toEqual([]);
    expect(empty.count).toEqual({ running: 0, total: 0 });
  });
});

describe("filterSidebar", () => {
  const PROCESSES = [
    withProject(process(1, "Command", "web-server"), 1),
    withProject(process(2, "Agent", "claude"), 1),
    withProject(process(3, "Command", "web-worker"), 2),
  ];
  const PROJECTS = [projectView(1, "app"), projectView(2, "infra")];

  it("keeps everything for a blank query", () => {
    const result = filterSidebar(PROCESSES, PROJECTS, "  ");
    expect(result.processes).toBe(PROCESSES);
    expect(result.projects).toBe(PROJECTS);
  });

  it("matches processes by label, case-insensitively, and hides projects with no match", () => {
    const result = filterSidebar(PROCESSES, PROJECTS, "WEB");
    expect(result.processes.map((p) => p.label)).toEqual(["web-server", "web-worker"]);
    // Both projects hold a matching "web-*" process, so both remain.
    expect(result.projects.map((p) => p.name)).toEqual(["app", "infra"]);
  });

  it("drops a project whose only processes do not match", () => {
    const result = filterSidebar(PROCESSES, PROJECTS, "claude");
    expect(result.processes.map((p) => p.label)).toEqual(["claude"]);
    expect(result.projects.map((p) => p.name)).toEqual(["app"]);
  });

  it("keeps all of a project's processes when the project name matches", () => {
    const result = filterSidebar(PROCESSES, PROJECTS, "infra");
    expect(result.processes.map((p) => p.label)).toEqual(["web-worker"]);
    expect(result.projects.map((p) => p.name)).toEqual(["infra"]);
  });
});
