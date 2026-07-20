import type { ProcStatus, ProjectView, TodoStatus } from "@domain";
import {
  COORDINATION,
  LEAD_AGENT,
  requestLeadCoordination,
  triggerScratchpadRewrite,
} from "../../src/harness/leadAgent.js";
import { launchAgent } from "../../src/flows/launch.js";
import { openProject } from "../../src/flows/openProject.js";
import { orchestrationPane } from "../../src/screens/OrchestrationPane.js";
import { scratchpadPanel } from "../../src/screens/ScratchpadPanel.js";
import { sidebar } from "../../src/screens/Sidebar.js";
import { todoBoard } from "../../src/screens/TodoBoard.js";

// The lead the walk launches: its stub binds its own MCP session and, over the real IPC wire, seeds
// the shared documents a bound agent produces — a scratchpad, a blocker chain, and a comment stamped
// with its own identity. Its label is what the board shows as the comment's author.
const LEAD = LEAD_AGENT.lead;
const RUNNING: ProcStatus = "Running";
const DONE: TodoStatus = "done";

// The scratchpad and to-do coordination panels, driven in the real window against the real core: a
// stale scratchpad save loses to a concurrent write (the optimistic-concurrency guard, end to end); a
// blocked todo cannot complete until its blocker does (the gate, and its live clearing); and a
// comment carries the author the core stamped from its creator's bound session. Every assertion keys
// on something the real core produced across the wire — a revision only the concurrent writer bumped,
// a refusal only the gate raises, an author only a bound session earns — that no repaint can fake.
describe("the coordination panels", () => {
  let project: ProjectView;

  before(async () => {
    project = await openProject("orchestration");

    // Put the lead into its coordination arm before it launches, so it seeds the documents as a
    // genuinely bound agent — the writes come over the real MCP/IPC wire, not from the window.
    await requestLeadCoordination();
    await launchAgent(LEAD);
    await sidebar.waitForRowStatus(LEAD, RUNNING);

    await sidebar.openOrchestration(project.name);
  });

  after(async () => {
    await sidebar.stopIfRunning(LEAD);
  });

  it("refuses a stale scratchpad save and keeps the concurrent edit", async () => {
    await orchestrationPane.showView("scratchpads");
    await scratchpadPanel.waitForRoster();

    // Open the scratchpad the lead created; the editor now holds the revision it opened at.
    const opened = await scratchpadPanel.waitForRow(COORDINATION.scratchpad);
    await scratchpadPanel.open(COORDINATION.scratchpad);

    // The lead re-writes the same scratchpad over the wire, bumping its revision under our stale
    // editor. Wait for that concurrent write to land — the roster moves off the opened revision.
    await triggerScratchpadRewrite();
    const bumped = await scratchpadPanel.waitForRevisionChange(COORDINATION.scratchpad, opened);
    expect(bumped).toBeGreaterThan(opened);

    // Our edit, saved against the now-stale revision, is refused by the core — and the conflict
    // banner names the revision the scratchpad actually moved to, which only the concurrent writer
    // produced. A guard that had been dropped would let this save through with no conflict at all.
    await scratchpadPanel.edit();
    await scratchpadPanel.save();
    expect(await scratchpadPanel.waitForConflictRevision()).toBe(bumped);

    // Nothing was clobbered: reloading shows the concurrent writer's content, not our rejected edit.
    await scratchpadPanel.reload();
    await scratchpadPanel.waitForBody(COORDINATION.bodyV2);
  });

  it("refuses to complete a blocked todo until its blocker is done", async () => {
    await orchestrationPane.showView("todos");
    await todoBoard.waitForTodo(COORDINATION.blocked);

    // The blocked todo shows its gate, and completing it is refused by the core with its blocker
    // named — surfaced verbatim, never pre-empted by the UI.
    await todoBoard.waitForBlocked(COORDINATION.blocked, true);
    await todoBoard.complete(COORDINATION.blocked);
    expect(await todoBoard.waitForRefusal(COORDINATION.blocked)).not.toHaveLength(0);
    expect(await todoBoard.status(COORDINATION.blocked)).not.toBe(DONE);

    // Complete the blocker; the gate then clears live (a real TodoChanged re-refresh), and the
    // once-blocked todo completes.
    await todoBoard.complete(COORDINATION.blocker);
    await todoBoard.waitForStatus(COORDINATION.blocker, DONE);
    await todoBoard.waitForBlocked(COORDINATION.blocked, false);
    await todoBoard.complete(COORDINATION.blocked);
    await todoBoard.waitForStatus(COORDINATION.blocked, DONE);
  });

  it("shows the bound author of a comment", async () => {
    await orchestrationPane.showView("todos");
    await todoBoard.waitForTodo(COORDINATION.commented);

    // The comment renders its body and its author — the lead's own label, which the core stamped
    // from the creating bound session, not anything the window or the caller supplied. An unbound
    // author would read "unattributed"; only a real bound session earns the label.
    const text = await todoBoard.expandedText(COORDINATION.commented);
    expect(text).toContain(COORDINATION.comment);
    expect(text).toContain(LEAD);
  });
});
