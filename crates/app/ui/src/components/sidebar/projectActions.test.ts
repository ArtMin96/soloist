import { describe, expect, it, vi } from "vitest";
import { projectActions } from "@/components/sidebar/projectActions";

describe("projectActions", () => {
  it("groups bulk commands, project views, and the removal, each wired to its handler", () => {
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
    const { bulk, views, danger } = projectActions(handlers);

    expect(bulk.map((action) => action.label)).toEqual([
      "Start all",
      "Restart running",
      "Stop all",
    ]);
    expect(views.map((action) => action.label)).toEqual(["Orchestration", "Project settings"]);
    // The destructive removal is its own group, so both menus render it last, behind a
    // separator — never adjacent to a routine action.
    expect(danger.map((action) => action.label)).toEqual(["Remove project"]);

    // Each descriptor invokes exactly the handler it names — the contract both menus depend on.
    bulk[0].run();
    expect(handlers.onStartAll).toHaveBeenCalledOnce();
    bulk[1].run();
    expect(handlers.onRestartRunning).toHaveBeenCalledOnce();
    bulk[2].run();
    expect(handlers.onStopAll).toHaveBeenCalledOnce();
    views[0].run();
    expect(handlers.onOpenOrchestration).toHaveBeenCalledOnce();
    views[1].run();
    expect(handlers.onOpenProjectSettings).toHaveBeenCalledOnce();
    danger[0].run();
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

    const { arrange } = projectActions(handlers);

    expect(arrange.map((action) => action.label)).toEqual(["Move up", "Move down"]);
    arrange[0].run();
    expect(onMoveUp).toHaveBeenCalledOnce();
    arrange[1].run();
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

    expect(first.arrange.map((action) => action.label)).toEqual(["Move down"]);
    expect(last.arrange.map((action) => action.label)).toEqual(["Move up"]);
    // A list of one has no arrangement to offer, so the menus render no group at all.
    expect(only.arrange).toEqual([]);
  });
});
