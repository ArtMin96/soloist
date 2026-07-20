import type { ProcStatus } from "@domain";
import { openProject } from "../../src/flows/openProject.js";
import { openLaunchPicker, openTerminal, TERMINAL_ENTRY } from "../../src/flows/launch.js";
import { launchPicker } from "../../src/screens/LaunchPicker.js";
import { sidebar } from "../../src/screens/Sidebar.js";
import { terminalPane } from "../../src/screens/TerminalPane.js";
import { browser } from "@wdio/globals";

// Where an opened terminal must settle: a login shell runs until it is told to stop, so anything
// short of Running means the shell exited on spawn — the failure a fixed command line invites, and
// the one a row painted by the UI alone would hide.
const OPENED: ProcStatus = "Running";

// The second terminal's label, numbered by the core so two open shells stay tellable apart.
const SECOND_TERMINAL = "Terminal 2";

describe("opening a terminal in a project", () => {
  before(async () => {
    await openProject("basic");
  });

  after(async () => {
    // Leave nothing running: a shell that outlives its app session is a leftover the next
    // session's app would (rightly) raise its orphan dialog over.
    await sidebar.stopIfRunning(TERMINAL_ENTRY);
    await sidebar.stopIfRunning(SECOND_TERMINAL);
  });

  it("offers a terminal in the same picker as the agent tools", async () => {
    await openLaunchPicker();

    expect(await launchPicker.entries()).toContain(TERMINAL_ENTRY);

    await browser.keys("Escape");
    await launchPicker.waitUntilClosed();
  });

  it("renders the terminal in the sidebar once opened", async () => {
    const row = await openTerminal();

    expect(row.label).toBe(TERMINAL_ENTRY);
    expect(row.selected).toBe(true);
    // It lands under Terminals, the group that has always rendered but never had anything in it.
    expect(await sidebar.hasGroup("Terminals")).toBe(true);
  });

  it("actually starts the shell", async () => {
    await sidebar.waitForRowStatus(TERMINAL_ENTRY, OPENED);
  });

  it("opens a laid-out terminal pane for it", async () => {
    expect(await terminalPane.title()).toContain(TERMINAL_ENTRY);
    expect(await terminalPane.isMounted()).toBe(true);

    // jsdom cannot prove this and the unit suites do not try: a terminal that mounts but measures
    // zero renders nothing. Real layout is the reason this spec runs in a real window.
    const { width, height } = await terminalPane.size();
    expect(width).toBeGreaterThan(0);
    expect(height).toBeGreaterThan(0);
  });

  it("numbers a second terminal rather than repeating the first's name", async () => {
    // Two rows reading "Terminal" would be indistinguishable in the sidebar, so the core numbers
    // them. This is the walk that proves the numbering a user actually sees.
    const row = await openTerminal(SECOND_TERMINAL);

    expect(row.label).toBe(SECOND_TERMINAL);
    await sidebar.waitForRowStatus(SECOND_TERMINAL, OPENED);
  });
});
