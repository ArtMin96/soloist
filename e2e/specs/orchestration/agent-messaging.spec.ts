import type { ProcStatus } from "@domain";
import { captureProof } from "../../src/harness/artifacts.js";
import {
  LEAD_AGENT,
  MAILBOX,
  requestLeadMailbox,
} from "../../src/harness/leadAgent.js";
import { launchAgent } from "../../src/flows/launch.js";
import { messagesPanel } from "../../src/screens/MessagesPanel.js";
import { orchestrationPane } from "../../src/screens/OrchestrationPane.js";
import { openProject } from "../../src/flows/openProject.js";
import { sidebar } from "../../src/screens/Sidebar.js";
import { terminalPane } from "../../src/screens/TerminalPane.js";

const LEAD = LEAD_AGENT.lead;
const RUNNING: ProcStatus = "Running";

function canonicalMessageBody(text: string): string {
  return text.replace(/[·\s]+/gu, "");
}

// A real-window handoff through the real app, PTYs, and authenticated IPC sessions. The fixture
// prints each proof marker only after the corresponding mailbox operation succeeds. In particular,
// its line reader cannot print `submitted wake received` for bytes merely pasted into the PTY: it
// needs the semantic return that submits the turn.
//
// The terminal reads below are an agent's *own* output, so what they prove is that the core carried
// the message into the recipient process — the bodies are legible there only because the fixture
// echoes what it received, which a real agent CLI would not. The final case is the one that reads a
// Soloist surface instead: the Messages view renders the retained transcript, so a body found there
// was displayed by the app rather than printed by the fixture.
//
// Every agent in this walk is a fixture the harness puts first on PATH — no real agent CLI runs at
// any point, here or anywhere in this suite. Green says the core's messaging works against agents
// that behave as these stubs do; it is not a live multi-provider walk.
describe("addressed work between spawned agents", () => {
  let projectName = "";

  before(async () => {
    const project = await openProject("orchestration");
    projectName = project.name;
    await requestLeadMailbox();
    await launchAgent(LEAD);
    await sidebar.waitForRowStatus(LEAD, RUNNING);
    await sidebar.waitForRowStatus(MAILBOX.primary, RUNNING);
    await sidebar.waitForRowStatus(MAILBOX.peer, RUNNING);
  });

  after(async () => {
    await sidebar.stopIfRunning(MAILBOX.primary);
    await sidebar.stopIfRunning(MAILBOX.peer);
    await sidebar.stopIfRunning(LEAD);
  });

  it("proves default onboarding, opt-out, addressed exchange, and acknowledged completion", async () => {
    await sidebar.select(MAILBOX.primary);
    const primaryTerminal = await terminalPane.waitForText(
      MAILBOX.completionReported,
    );
    expect(primaryTerminal).toContain(MAILBOX.submitted);
    expect(primaryTerminal).toContain(MAILBOX.instructions);
    expect(primaryTerminal).toContain(MAILBOX.instructionsReceived);
    expect(primaryTerminal).toContain(MAILBOX.taskAcknowledged);
    expect(primaryTerminal).toContain(MAILBOX.primaryLeaseAcquired);
    expect(primaryTerminal).toContain(MAILBOX.primaryLeaseReleased);

    await sidebar.select(MAILBOX.peer);
    const peerTerminal = await terminalPane.waitForText(MAILBOX.peerExchanged);
    expect(peerTerminal).toContain(MAILBOX.submitted);
    expect(peerTerminal).toContain(MAILBOX.instructionsSuppressed);
    expect(peerTerminal).not.toContain(MAILBOX.instructions);
    expect(peerTerminal).toContain(MAILBOX.peerLeaseHeld);
    expect(peerTerminal).toContain(MAILBOX.peerLeaseAcquired);
    expect(peerTerminal).toContain(MAILBOX.peerLeaseReleased);
    expect(canonicalMessageBody(peerTerminal)).toContain(
      canonicalMessageBody(MAILBOX.broadcast),
    );
    expect(canonicalMessageBody(peerTerminal)).toContain(
      canonicalMessageBody(MAILBOX.direct),
    );

    await sidebar.select(LEAD);
    const leadTerminal = await terminalPane.waitForText(MAILBOX.proof);
    expect(leadTerminal).toContain(
      "lead retrieved and acknowledged Completion",
    );
    expect(canonicalMessageBody(leadTerminal)).toContain(
      canonicalMessageBody(MAILBOX.completion),
    );

    await captureProof("agent-messaging-complete", {
      primaryTerminal,
      peerTerminal,
      leadTerminal,
    });
  });

  it("shows the exchanged bodies in the app's own Messages view", async () => {
    await sidebar.openOrchestration(projectName);
    await orchestrationPane.showView("messages");
    const transcript = await messagesPanel.waitForTranscript();

    // The same two bodies the terminals echoed, this time rendered by Soloist from its own retained
    // record — the distinction between a fixture printing its mail and the app showing it.
    expect(canonicalMessageBody(transcript)).toContain(
      canonicalMessageBody(MAILBOX.direct),
    );
    expect(canonicalMessageBody(transcript)).toContain(
      canonicalMessageBody(MAILBOX.broadcast),
    );
    // A closed worker's messages stay readable, so the routing labels survive alongside the bodies.
    expect(transcript).toContain(MAILBOX.peer);

    await captureProof("agent-messaging-transcript", { transcript });
  });
});
