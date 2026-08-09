import { describe, expect, it, vi } from "vitest";
import { buildCommands, type Command, type CommandContext } from "@/lib/commands";
import { onBranchSwitcherRequest, type BranchClusterView } from "@/store/git/branchCluster";
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

/** A trusted repository on a branch that tracks one, with everything the chrome can offer offered. */
function cluster(overrides: Partial<BranchClusterView> = {}): BranchClusterView {
  return {
    branch: { name: "main", upstream: "origin/main", sync: { state: "up_to_date" } },
    branches: null,
    exchanging: false,
    busy: false,
    exchange: { fetch: vi.fn(), pull: vi.fn(), push: vi.fn(), stop: vi.fn() },
    branchActions: {
      switchTo: vi.fn(),
      create: vi.fn(() => Promise.resolve(true)),
      remove: vi.fn(),
      stash: vi.fn(),
      popStash: vi.fn(),
    },
    openPullRequest: vi.fn(),
    onBranchesOpen: vi.fn(),
    ...overrides,
  };
}

function context(overrides: Partial<CommandContext> = {}): CommandContext {
  return {
    processes: [],
    projects: [],
    theme: "system",
    git: null,
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
      onRemove: vi.fn(),
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
    const ids = flat(context({ projects: [STOREFRONT], processes: [proc()], git: cluster() })).map(
      (c) => c.id,
    );
    expect(new Set(ids).size).toBe(ids.length);
  });
});

describe("buildCommands — what is checked out", () => {
  it("offers by keyboard what the title bar's far corner offers by mouse", () => {
    const labels = flat(context({ git: cluster() })).map((c) => c.label);
    expect(labels).toContain("Switch branch…");
    expect(labels).toContain("Fetch from the remote");
    expect(labels).toContain("Pull from the upstream");
    expect(labels).toContain("Push to the upstream");
    expect(labels).toContain("Show this branch's pull request");
  });

  it("says nothing about version control where there is no repository in view", () => {
    const groups = buildCommands(context({ projects: [STOREFRONT] }));
    expect(groups.find((group) => group.heading === "Version control")).toBeUndefined();
  });

  it("runs the very callback the chrome's control runs, rather than reaching the core itself", () => {
    const exchange = { fetch: vi.fn(), pull: vi.fn(), push: vi.fn(), stop: vi.fn() };
    const openPullRequest = vi.fn();
    const ctx = context({ git: cluster({ exchange, openPullRequest }) });

    byLabel(ctx, "Fetch from the remote").run();
    byLabel(ctx, "Pull from the upstream").run();
    byLabel(ctx, "Push to the upstream").run();
    byLabel(ctx, "Show this branch's pull request").run();

    expect(exchange.fetch).toHaveBeenCalled();
    expect(exchange.pull).toHaveBeenCalled();
    expect(exchange.push).toHaveBeenCalled();
    expect(openPullRequest).toHaveBeenCalled();
  });

  it("switching branches asks the chrome to open its switcher, not a second one", () => {
    const opened = vi.fn();
    const stop = onBranchSwitcherRequest(opened);

    byLabel(context({ git: cluster() }), "Switch branch…").run();

    expect(opened).toHaveBeenCalled();
    stop();
  });

  it("offers publishing rather than pushing on a branch that tracks nothing, and no pull", () => {
    const labels = flat(
      context({
        git: cluster({ branch: { name: "spike", upstream: null, sync: { state: "unknown" } } }),
      }),
    ).map((c) => c.label);

    expect(labels).toContain("Publish this branch");
    expect(labels).not.toContain("Push to the upstream");
    expect(labels, "there is nothing to pull from an upstream that does not exist").not.toContain(
      "Pull from the upstream",
    );
  });

  it("offers stopping instead of starting another while the remote is being waited on", () => {
    const labels = flat(context({ git: cluster({ exchanging: true }) })).map((c) => c.label);

    expect(labels).toContain("Stop waiting on the remote");
    expect(labels).not.toContain("Fetch from the remote");
    expect(labels).not.toContain("Pull from the upstream");
  });

  it("offers nothing that changes the repository until the project is trusted", () => {
    const labels = flat(
      context({ git: cluster({ exchange: null, branchActions: null, openPullRequest: null }) }),
    ).map((c) => c.label);

    expect(labels).not.toContain("Switch branch…");
    expect(labels).not.toContain("Fetch from the remote");
    expect(labels).not.toContain("Show this branch's pull request");
  });
});
