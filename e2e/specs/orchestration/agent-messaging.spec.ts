import type { ProcStatus } from "@domain";
import { captureProof } from "../../src/harness/artifacts.js";
import {
  LEAD_AGENT,
  MAILBOX,
  requestLeadMailbox,
} from "../../src/harness/leadAgent.js";
import { launchAgent } from "../../src/flows/launch.js";
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
describe("addressed work between spawned agents", () => {
  before(async () => {
    await openProject("orchestration");
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
    const primary = await terminalPane.waitForText(MAILBOX.completionReported);
    expect(primary).toContain(MAILBOX.submitted);
    expect(primary).toContain(MAILBOX.instructions);
    expect(primary).toContain(MAILBOX.instructionsReceived);
    expect(primary).toContain(MAILBOX.taskAcknowledged);
    expect(primary).toContain(MAILBOX.primaryLeaseAcquired);
    expect(primary).toContain(MAILBOX.primaryLeaseReleased);

    await sidebar.select(MAILBOX.peer);
    const peer = await terminalPane.waitForText(MAILBOX.peerExchanged);
    expect(peer).toContain(MAILBOX.submitted);
    expect(peer).toContain(MAILBOX.instructionsSuppressed);
    expect(peer).not.toContain(MAILBOX.instructions);
    expect(peer).toContain(MAILBOX.peerLeaseHeld);
    expect(peer).toContain(MAILBOX.peerLeaseAcquired);
    expect(peer).toContain(MAILBOX.peerLeaseReleased);
    expect(canonicalMessageBody(peer)).toContain(canonicalMessageBody(MAILBOX.broadcast));
    expect(canonicalMessageBody(peer)).toContain(canonicalMessageBody(MAILBOX.direct));

    await sidebar.select(LEAD);
    const lead = await terminalPane.waitForText(MAILBOX.proof);
    expect(lead).toContain("lead retrieved and acknowledged Completion");
    expect(canonicalMessageBody(lead)).toContain(canonicalMessageBody(MAILBOX.completion));

    await captureProof("agent-messaging-complete", { primary, peer, lead });
  });
});
