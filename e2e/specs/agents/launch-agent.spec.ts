import type { ProcStatus } from "@domain";
import { browser } from "@wdio/globals";
import { openProject } from "../../src/flows/openProject.js";
import { launchAgent } from "../../src/flows/launchAgent.js";
import { agentPicker } from "../../src/screens/AgentPicker.js";
import { sidebar } from "../../src/screens/Sidebar.js";
import { terminalPane } from "../../src/screens/TerminalPane.js";
import { titlebar } from "../../src/screens/Titlebar.js";

// The tool as the app's built-in registry names it: "Claude" is the display name and registry key,
// "claude" is the command it spawns.
const CLAUDE = { name: "Claude", command: "claude" };

// A registered process that was never started rests here, so leaving it is what proves a launch
// actually reached the supervisor rather than only painting a row.
const NEVER_STARTED: ProcStatus = "Stopped";

describe("launching an agent into a project", () => {
  let projectName: string;

  before(async () => {
    ({ name: projectName } = await openProject("basic"));
  });

  it("offers Claude for the open project", async () => {
    await titlebar.launchAgent();
    await agentPicker.waitUntilOpen();

    expect(await agentPicker.targetProject()).toBe(projectName);
    expect(await agentPicker.tools()).toContain(CLAUDE.name);
    expect(await agentPicker.commandFor(CLAUDE.name)).toBe(CLAUDE.command);

    await browser.keys("Escape");
    await agentPicker.waitUntilClosed();
  });

  it("renders the agent in the sidebar once launched", async () => {
    const row = await launchAgent(CLAUDE.name);

    expect(row.label).toBe(CLAUDE.name);
    expect(row.selected).toBe(true);
    expect(await sidebar.hasGroup("Agents")).toBe(true);
  });

  it("actually starts the agent's process", async () => {
    // Whether it settles Running or Crashed depends on Claude Code being installed on the machine —
    // true on a developer's box, false in CI — so asserting either would make this pass or fail for
    // reasons that have nothing to do with the app. What holds everywhere is that the app really
    // tried: the supervisor moved it off Stopped and spawned something. Without this, the two specs
    // above would pass against a row that had merely been drawn.
    const row = await sidebar.waitForRow(CLAUDE.name);
    expect(row.status).not.toBe(NEVER_STARTED);
  });

  it("opens a laid-out terminal for the launched agent", async () => {
    // Not an exact match, and not laziness: the header shows the process's label only until the
    // process sets its own title over OSC. A real Claude Code renames it to "✳ Claude Code" within
    // a second of starting, so pinning the exact string would assert Claude Code's branding rather
    // than this app's behavior, and would flip the moment the tool is absent and the label stands.
    expect(await terminalPane.title()).toContain(CLAUDE.name);
    expect(await terminalPane.isMounted()).toBe(true);

    // jsdom cannot prove this and the unit suites do not try: a terminal that mounts but measures
    // zero renders nothing. Real layout is the reason this spec runs in a real window.
    const { width, height } = await terminalPane.size();
    expect(width).toBeGreaterThan(0);
    expect(height).toBeGreaterThan(0);
  });
});
