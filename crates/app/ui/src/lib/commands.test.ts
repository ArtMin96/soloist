import { describe, expect, it, vi } from "vitest";
import { buildCommands, type Command, type CommandContext } from "@/lib/commands";
import type { ProcessView, ProjectView } from "@/domain";

const STOREFRONT: ProjectView = { id: 1, name: "Storefront", root: "/p/storefront", icon: null };

function proc(overrides: Partial<ProcessView> = {}): ProcessView {
  return {
    id: 10,
    project: 1,
    kind: "Command",
    label: "Web",
    status: "Running",
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated",
    ...overrides,
  };
}

function context(overrides: Partial<CommandContext> = {}): CommandContext {
  return {
    processes: [],
    projects: [],
    theme: "system",
    newAgentOrTerminal: vi.fn(),
    openProject: vi.fn(),
    openSettings: vi.fn(),
    setTheme: vi.fn(),
    selectProcess: vi.fn(),
    openProjectSettings: vi.fn(),
    openOrchestration: vi.fn(),
    startAll: vi.fn(),
    stopAll: vi.fn(),
    restartRunning: vi.fn(),
    process: {
      onTrust: vi.fn(),
      onResume: vi.fn(),
      onStart: vi.fn(),
      onStop: vi.fn(),
      onRestart: vi.fn(),
    },
    ...overrides,
  };
}

function flat(ctx: CommandContext): Command[] {
  return buildCommands(ctx).flatMap((group) => group.commands);
}

function byLabel(ctx: CommandContext, label: string): Command {
  const command = flat(ctx).find((c) => c.label === label);
  if (!command) throw new Error(`no command labelled "${label}"`);
  return command;
}

describe("buildCommands", () => {
  it("always offers the app-wide actions and the three theme commands", () => {
    const labels = flat(context()).map((c) => c.label);
    expect(labels).toContain("New agent or terminal");
    expect(labels).toContain("Open project…");
    expect(labels).toContain("Open settings");
    expect(labels).toContain("Theme: Light");
    expect(labels).toContain("Theme: Dark");
    expect(labels).toContain("Theme: System");
  });

  it("setting a theme runs setTheme with that theme", () => {
    const setTheme = vi.fn();
    byLabel(context({ setTheme }), "Theme: Dark").run();
    expect(setTheme).toHaveBeenCalledWith("dark");
  });

  it("offers each open project's bulk and navigation commands", () => {
    const labels = flat(context({ projects: [STOREFRONT] })).map((c) => c.label);
    expect(labels).toContain("Start all — Storefront");
    expect(labels).toContain("Stop all — Storefront");
    expect(labels).toContain("Restart running — Storefront");
    expect(labels).toContain("Open settings — Storefront");
    expect(labels).toContain("Open orchestration — Storefront");
  });

  it("a bulk command targets the project id", () => {
    const startAll = vi.fn();
    byLabel(context({ projects: [STOREFRONT], startAll }), "Start all — Storefront").run();
    expect(startAll).toHaveBeenCalledWith(1);
  });

  it("offers focus plus only the status-valid actions for a process", () => {
    const labels = flat(
      context({ projects: [STOREFRONT], processes: [proc({ status: "Running" })] }),
    ).map((c) => c.label);
    expect(labels).toContain("Focus Web");
    expect(labels).toContain("Stop Web");
    expect(labels).toContain("Restart Web");
    // Running, so Start is not offered (single-sourced from processActions).
    expect(labels).not.toContain("Start Web");
  });

  it("focus runs selectProcess with the process id", () => {
    const selectProcess = vi.fn();
    byLabel(
      context({ projects: [STOREFRONT], processes: [proc({ id: 42 })], selectProcess }),
      "Focus Web",
    ).run();
    expect(selectProcess).toHaveBeenCalledWith(42);
  });

  it("omits the Processes group when there are no processes", () => {
    const groups = buildCommands(context({ projects: [STOREFRONT] }));
    expect(groups.find((g) => g.heading === "Processes")).toBeUndefined();
  });

  it("gives every command a unique id (stable React key / search identity)", () => {
    const ids = flat(context({ projects: [STOREFRONT], processes: [proc()] })).map((c) => c.id);
    expect(new Set(ids).size).toBe(ids.length);
  });
});
