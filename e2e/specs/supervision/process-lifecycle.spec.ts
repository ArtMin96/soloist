import type { ProcStatus } from "@domain";
import { openProject } from "../../src/flows/openProject.js";
import { sidebar } from "../../src/screens/Sidebar.js";

// The fixture's three stub commands: one that lives until stopped, one that exits nonzero on
// cue, one that binds a fresh ephemeral port per spawn. Deterministic stand-ins, so supervision
// behavior is asserted without waiting on a real process to misbehave.
const ECHO = "Echo";
const CRASHER = "Crasher";
const LISTENER = "Listener";

const RUNNING: ProcStatus = "Running";
const STOPPED: ProcStatus = "Stopped";
const CRASHED: ProcStatus = "Crashed";

// The dashboard-core walk: a command is trust-gated until reviewed, a trusted command really
// starts, a crash is reported as Crashed rather than papered over, a stop really reaps, and a
// restart really replaces the process. All driven through the row's own controls, the way a user
// does it — the trust click is load-bearing, because a fresh session's data dir starts with
// nothing trusted and an untrusted command cannot start.
describe("supervising a project's commands", () => {
  before(async () => {
    await openProject("basic");
  });

  after(async () => {
    await sidebar.stopIfRunning(LISTENER);
    await sidebar.stopIfRunning(ECHO);
  });

  it("starts a trusted command and reports it Running", async () => {
    await sidebar.select(ECHO);
    await sidebar.trust(ECHO);
    await sidebar.start(ECHO);

    await sidebar.waitForRowStatus(ECHO, RUNNING);
  });

  it("stops it back to Stopped", async () => {
    await sidebar.stop(ECHO);

    await sidebar.waitForRowStatus(ECHO, STOPPED);
  });

  it("reports a command that exits nonzero as Crashed", async () => {
    await sidebar.select(CRASHER);
    await sidebar.trust(CRASHER);
    await sidebar.start(CRASHER);

    await sidebar.waitForRowStatus(CRASHER, CRASHED);
  });

  it("restart replaces the process, not just the row", async () => {
    await sidebar.select(LISTENER);
    await sidebar.trust(LISTENER);
    await sidebar.start(LISTENER);
    await sidebar.waitForRowStatus(LISTENER, RUNNING);

    // The stub binds a fresh ephemeral port each spawn, so a real restart must surface a
    // different discovered port. A restart that only repainted the row — or never reached the
    // supervisor — keeps the old one and cannot pass.
    const port = await sidebar.waitForPort(LISTENER);
    await sidebar.restart(LISTENER);
    const reborn = await sidebar.waitForPort(LISTENER, port);

    expect(reborn).not.toBe(port);
  });
});
