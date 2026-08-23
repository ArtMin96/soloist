import { describe, expect, it, vi } from "vitest";
import { projectActions, type ProjectActionSection } from "@/components/sidebar/projectActions";

function sectionFor(sections: ProjectActionSection[], id: ProjectActionSection["id"]) {
  return sections.find((section) => section.id === id);
}

describe("projectActions", () => {
  it("groups bulk commands, project views, and the removal in canonical order, each wired to its handler", () => {
    const handlers = {
      onStartAll: vi.fn(),
      onRestartRunning: vi.fn(),
      onStopAll: vi.fn(),
      onOpenOrchestration: vi.fn(),
      onOpenProjectSettings: vi.fn(),
      onRemoveProject: vi.fn(),
      onMoveUp: vi.fn(),
      onMoveDown: vi.fn(),
    };
    const sections = projectActions(handlers);

    expect(sections.map((section) => section.id)).toEqual(["bulk", "views", "arrange", "danger"]);

    const bulk = sectionFor(sections, "bulk")!;
    const views = sectionFor(sections, "views")!;
    const danger = sectionFor(sections, "danger")!;

    expect(bulk.actions.map((action) => action.label)).toEqual([
      "Start all",
      "Restart running",
      "Stop all",
    ]);
    expect(views.actions.map((action) => action.label)).toEqual([
      "Orchestration",
      "Project settings",
    ]);
    // The destructive removal is its own section, marked for the menus' destructive treatment —
    // both menus render it last, behind a separator, never adjacent to a routine action.
    expect(danger.actions.map((action) => action.label)).toEqual(["Remove project"]);
    expect(danger.destructive).toBe(true);
    expect(bulk.destructive).toBeFalsy();

    // Each descriptor invokes exactly the handler it names — the contract both menus depend on.
    bulk.actions[0].run();
    expect(handlers.onStartAll).toHaveBeenCalledOnce();
    bulk.actions[1].run();
    expect(handlers.onRestartRunning).toHaveBeenCalledOnce();
    bulk.actions[2].run();
    expect(handlers.onStopAll).toHaveBeenCalledOnce();
    views.actions[0].run();
    expect(handlers.onOpenOrchestration).toHaveBeenCalledOnce();
    views.actions[1].run();
    expect(handlers.onOpenProjectSettings).toHaveBeenCalledOnce();
    danger.actions[0].run();
    expect(handlers.onRemoveProject).toHaveBeenCalledOnce();
  });

  it("offers a move in each direction the project can actually go", () => {
    const onMoveUp = vi.fn();
    const onMoveDown = vi.fn();
    const handlers = {
      onStartAll: vi.fn(),
      onRestartRunning: vi.fn(),
      onStopAll: vi.fn(),
      onOpenOrchestration: vi.fn(),
      onOpenProjectSettings: vi.fn(),
      onRemoveProject: vi.fn(),
      onMoveUp,
      onMoveDown,
    };

    const arrange = sectionFor(projectActions(handlers), "arrange")!;

    expect(arrange.actions.map((action) => action.label)).toEqual(["Move up", "Move down"]);
    arrange.actions[0].run();
    expect(onMoveUp).toHaveBeenCalledOnce();
    arrange.actions[1].run();
    expect(onMoveDown).toHaveBeenCalledOnce();
  });

  it("withholds the move a project at an end of the list cannot make", () => {
    const base = {
      onStartAll: vi.fn(),
      onRestartRunning: vi.fn(),
      onStopAll: vi.fn(),
      onOpenOrchestration: vi.fn(),
      onOpenProjectSettings: vi.fn(),
      onRemoveProject: vi.fn(),
    };

    const first = projectActions({ ...base, onMoveUp: null, onMoveDown: vi.fn() });
    const last = projectActions({ ...base, onMoveUp: vi.fn(), onMoveDown: null });
    const only = projectActions({ ...base, onMoveUp: null, onMoveDown: null });

    expect(sectionFor(first, "arrange")?.actions.map((action) => action.label)).toEqual([
      "Move down",
    ]);
    expect(sectionFor(last, "arrange")?.actions.map((action) => action.label)).toEqual(["Move up"]);
    // A list of one has no arrangement to offer, so the section is absent rather than empty —
    // the menus render no group and no separator for it at all.
    expect(sectionFor(only, "arrange")).toBeUndefined();
    expect(only.map((section) => section.id)).toEqual(["bulk", "views", "danger"]);
  });
});
