import { describe, expect, it, vi } from "vitest";
import { projectActions } from "@/components/sidebar/projectActions";

describe("projectActions", () => {
  it("groups bulk supervisor commands and project views, each wired to its handler", () => {
    const handlers = {
      onStartAll: vi.fn(),
      onRestartRunning: vi.fn(),
      onStopAll: vi.fn(),
      onOpenOrchestration: vi.fn(),
      onOpenProjectSettings: vi.fn(),
    };
    const { bulk, views } = projectActions(handlers);

    expect(bulk.map((action) => action.label)).toEqual([
      "Start all",
      "Restart running",
      "Stop all",
    ]);
    expect(views.map((action) => action.label)).toEqual(["Orchestration", "Project settings"]);

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
  });
});
