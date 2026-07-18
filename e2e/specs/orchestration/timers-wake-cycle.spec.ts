import type { ProcStatus, ProjectView } from "@domain";
import { LEAD_AGENT, TIMER, releaseWorkerToIdle, requestLeadTimer } from "../../src/harness/leadAgent.js";
import { launchAgent } from "../../src/flows/launchAgent.js";
import { openProject } from "../../src/flows/openProject.js";
import { orchestrationPane } from "../../src/screens/OrchestrationPane.js";
import { sidebar } from "../../src/screens/Sidebar.js";
import { terminalPane } from "../../src/screens/TerminalPane.js";
import { timersPanel } from "../../src/screens/TimersPanel.js";

// The lead the walk launches and the worker its stub spawns. The lead arms a fire-when-idle-all timer
// watching the worker over the real MCP/IPC wire; the worker is held Working until the walk drives it
// idle, so the timer fires on cue.
const LEAD = LEAD_AGENT.lead;
const WORKER = LEAD_AGENT.worker;
const RUNNING: ProcStatus = "Running";

// The wake-reason header the core prepends to a delivered timer body (see the core's
// `wake_reason_header`): the timer-id marker every wake carries, and the all-idle reason that tells
// the woken agent it fired because its watched peers went idle — not because the max-wait backstop
// elapsed. Asserted as the user-visible text that lands in the lead's terminal.
const WAKE_PREFIX = "Soloist timer #";
const WAKE_REASON_ALL_IDLE = "watched agents are idle";

// Timers and the wake cycle, driven in the real window against the real core. A lead arms a
// fire-when-idle-all timer over its worker; the panel shows it waiting on that worker with a live
// countdown. Driving the worker idle fires the timer — it leaves the panel, and its body arrives in
// the lead's terminal prefixed with the wake-reason header. That delivered terminal text is the
// unfakeable evidence: it crossed a real PTY from the core scheduler, so no repaint or dropped event
// can fake it. The whole cycle runs through the existing scheduler delivery, with no UI-side
// injection.
describe("timers and the wake cycle", () => {
  let project: ProjectView;

  before(async () => {
    project = await openProject("orchestration");

    // Put the lead into its timers arm and hold its worker Working, both before it launches: the
    // lead spawns the worker and arms the timer over the real wire, and the held worker keeps the
    // timer waiting (never firing on its own) until the walk drives it idle.
    await requestLeadTimer();
    await launchAgent(LEAD);
    await sidebar.waitForRowStatus(LEAD, RUNNING);

    await sidebar.openOrchestration(project.name);
    await orchestrationPane.showView("timers");
  });

  after(async () => {
    // Release the held worker even if a test failed before doing so, so it never sits outputting; then
    // stop the lead and worker so the file leaves nothing running.
    await releaseWorkerToIdle();
    await sidebar.stopIfRunning(WORKER);
    await sidebar.stopIfRunning(LEAD);
  });

  it("shows an armed fire-when-idle timer waiting on its worker with a live countdown", async () => {
    // The real core armed this timer in the lead's bound session and surfaced it: it is waiting on
    // the worker — the watched process the core reports not yet idle — with a live countdown to the
    // max-wait backstop. A panel wired to nothing could not name the worker the core is watching.
    expect(await timersPanel.waitForWaitingOn()).toContain(WORKER);
    expect(await timersPanel.waitForCountdown()).toMatch(/\d+\s*[hms]/);
  });

  it("fires when the worker goes idle: clears the timer and delivers its body with the wake-reason prefix to the lead's terminal", async () => {
    // Drive the held worker idle. The real idle sampler classifies its settled terminal Idle, the
    // real scheduler's idle-all quorum is met, and the timer fires — leaving the panel.
    await releaseWorkerToIdle();
    await timersPanel.waitForNoTimers();

    // The fire delivered the body to the lead as a fresh turn: the scheduler wrote it to the lead's
    // real PTY prefixed with the wake-reason header, and the lead echoed it, so it is in the lead's
    // terminal. Switch to the lead's terminal and read it there.
    await sidebar.select(LEAD);
    const terminal = await terminalPane.waitForText(TIMER.body);

    // The body arrived, with the prefix that tells the agent it woke, and the reason that says its
    // peers went idle — not that the backstop elapsed (which would read "backstop elapsed").
    expect(terminal).toContain(WAKE_PREFIX);
    expect(terminal).toContain(WAKE_REASON_ALL_IDLE);
  });
});
