import type { ProcStatus } from "@domain";
import { browser } from "@wdio/globals";
import { openProject } from "../../src/flows/openProject.js";
import { launchAgent, openLaunchPicker } from "../../src/flows/launch.js";
import { launchPicker } from "../../src/screens/LaunchPicker.js";
import { sidebar } from "../../src/screens/Sidebar.js";
import { terminalPane } from "../../src/screens/TerminalPane.js";

// The tool as the app's built-in registry names it: "Claude" is the display name and registry key,
// "claude" is the command it spawns. What actually runs is the fixture stub the harness put first
// on PATH (wdio.conf.ts) — detection probes it, the supervisor spawns it, and it stays alive like
// a real agent — so the journey is identical on any machine and never opens a real Claude session.
const CLAUDE = { name: "Claude", command: "claude" };

// Where the launch must settle: the stub runs indefinitely, so anything short of Running means the
// supervisor never actually spawned it — a launch that had merely painted a row would rest Stopped.
const LAUNCHED: ProcStatus = "Running";

describe("launching an agent into a project", () => {
  let projectName: string;

  before(async () => {
    ({ name: projectName } = await openProject("basic"));
  });

  after(async () => {
    // Leave nothing running: an agent that outlives its app session is a leftover the next
    // session's app would (rightly) raise its orphan dialog over.
    await sidebar.stopIfRunning(CLAUDE.name);
  });

  it("offers Claude for the open project", async () => {
    await openLaunchPicker();

    expect(await launchPicker.targetProject()).toBe(projectName);
    expect(await launchPicker.entries()).toContain(CLAUDE.name);
    expect(await launchPicker.commandFor(CLAUDE.name)).toBe(CLAUDE.command);

    await browser.keys("Escape");
    await launchPicker.waitUntilClosed();
  });

  it("renders the agent in the sidebar once launched", async () => {
    const row = await launchAgent(CLAUDE.name);

    expect(row.label).toBe(CLAUDE.name);
    expect(row.selected).toBe(true);
    expect(await sidebar.hasGroup("Agents")).toBe(true);
  });

  it("actually starts the agent's process", async () => {
    await sidebar.waitForRowStatus(CLAUDE.name, LAUNCHED);
  });

  it("opens a laid-out terminal for the launched agent", async () => {
    // The header shows the process's label until the process retitles itself over OSC, which the
    // stub never does — so the label stands. A containment match asserts the pane identifies the
    // process without pinning the header's surrounding layout.
    expect(await terminalPane.title()).toContain(CLAUDE.name);
    expect(await terminalPane.isMounted()).toBe(true);

    // jsdom cannot prove this and the unit suites do not try: a terminal that mounts but measures
    // zero renders nothing. Real layout is the reason this spec runs in a real window.
    const { width, height } = await terminalPane.size();
    expect(width).toBeGreaterThan(0);
    expect(height).toBeGreaterThan(0);
  });
});
