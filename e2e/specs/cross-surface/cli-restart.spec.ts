import type { ProcStatus } from "@domain";
import { openProject } from "../../src/flows/openProject.js";
import { soloist } from "../../src/harness/cli.js";
import { sidebar } from "../../src/screens/Sidebar.js";

// The fixture's stub that binds a fresh ephemeral port per spawn — the same evidence the
// supervision walk restarts on, here for a restart that arrives from outside the window.
const LISTENER = "Listener";

const RUNNING: ProcStatus = "Running";

// One behavior, many frontends: the `soloist` CLI, the desktop UI, and MCP all route to the same
// core command, so a restart typed at a shell must land in the window like a clicked one. Every
// leg of that path is unit-tested in isolation and the assembly is tested nowhere else — the CLI
// is a separate binary that finds the app only through the runtime file the app's HTTP server
// records, and no headless test runs the two real binaries against each other.
describe("driving the stack from the command line", () => {
  before(async () => {
    await openProject("basic");
  });

  after(async () => {
    await sidebar.stopIfRunning(LISTENER);
  });

  it("a CLI restart replaces the process, and the window shows the new one", async () => {
    await sidebar.select(LISTENER);
    await sidebar.trust(LISTENER, "./bin/listener.sh");
    await sidebar.start(LISTENER);
    await sidebar.waitForRowStatus(LISTENER, RUNNING);

    const port = await sidebar.waitForPort(LISTENER);

    // The one action, and the only one not driven through the window: a real second binary,
    // reaching the running app over its loopback API the way a shell does.
    await soloist("restart", LISTENER);

    // The window's own telemetry, showing a port only a genuinely reborn process can have bound.
    // A CLI that never reached the core, a core that never replaced the process, or a window that
    // ignored the change all keep the old port and cannot pass.
    const reborn = await sidebar.waitForPort(LISTENER, port);

    expect(reborn).not.toBe(port);
  });
});
