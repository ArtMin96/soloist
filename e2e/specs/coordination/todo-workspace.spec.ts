import type { ProcStatus, ProjectView, TodoStatus } from "@domain";
import {
  COORDINATION,
  LEAD_AGENT,
  requestLeadCoordination,
} from "../../src/harness/leadAgent.js";
import { shrinkWindowToMinimum } from "../../src/harness/window.js";
import { launchAgent } from "../../src/flows/launch.js";
import { openProject } from "../../src/flows/openProject.js";
import { orchestrationPane } from "../../src/screens/OrchestrationPane.js";
import { scratchpadPanel } from "../../src/screens/ScratchpadPanel.js";
import { sidebar } from "../../src/screens/Sidebar.js";
import { terminalPane } from "../../src/screens/TerminalPane.js";
import { todoBoard } from "../../src/screens/TodoBoard.js";

// The lead the walk launches: over the real MCP/IPC wire its stub seeds a blocker chain, locks one
// todo, and reads another todo and a scratchpad through tools — the accesses the core records as
// the lead's session context. Its label is what the locked row's agent control names.
const LEAD = LEAD_AGENT.lead;
const RUNNING: ProcStatus = "Running";
const STOPPED: ProcStatus = "Stopped";
// The status the lead declared on every todo it created; the blocker gate is derived, not declared.
const OPEN: TodoStatus = "open";

// The todo workspace as a user moves through it, in the real window against the real core: a
// board row carries the id and blocker count the core computed from a chain built over the wire;
// the row the lead locked names the lead and opens its terminal; that terminal's header lists the
// lock as current work and the tool reads as this session, and each item leads straight back to
// its own row, expanded and focused; the board's own surface fits the narrowest window the app
// allows; and stopping the lead empties its context. Every assertion keys on state only the core produced — a lock held by a
// bound session, an access recorded from a real tool call, a focus the engine really moved.
describe("the todo workspace", () => {
  let project: ProjectView;
  /** The lead's process id as the core assigned it, read off the lineage tree. */
  let lead: number;
  /** The id of the todo the lead locked over the wire. */
  let locked: number;
  /** The id of the todo the lead read through a tool without locking it. */
  let loaded: number;

  before(async () => {
    project = await openProject("orchestration");

    await requestLeadCoordination();
    await launchAgent(LEAD);
    await sidebar.waitForRowStatus(LEAD, RUNNING);

    await sidebar.openOrchestration(project.name);
    const [leadNode] = await orchestrationPane.waitForNodes(LEAD);
    lead = leadNode!.id;

    await orchestrationPane.showView("todos");
    await todoBoard.waitForTodo(COORDINATION.blocked);
    await todoBoard.waitForTodo(COORDINATION.commented);
  });

  after(async () => {
    await sidebar.stopIfRunning(LEAD);
  });

  it("shows a row's id, declared status, and the unmet blockers of the chain the lead built", async () => {
    const row = await todoBoard.read(COORDINATION.blocked);
    expect(row).not.toBeNull();
    locked = row!.id;
    loaded = (await todoBoard.read(COORDINATION.commented))!.id;

    expect(await todoBoard.todoRef(COORDINATION.blocked)).toBe(`#${locked}`);
    expect(row!.status).toBe(OPEN);

    // The counts are the core's derived `blocked_by` for two different chains the lead set over
    // the wire — one blocker on the first todo, two on the second — so the singular and the plural
    // each come from a distinct real chain, not from one number the window could have guessed.
    await todoBoard.waitForBlocked(COORDINATION.blocked, true);
    expect(await todoBoard.blockerText(COORDINATION.blocked)).toBe("1 unmet blocker");
    expect(await todoBoard.blockerText(COORDINATION.commented)).toBe("2 unmet blockers");
  });

  it("names the lead on the row it locked, and opens the lead's terminal from it", async () => {
    // The control appears only once the core reports the lock the lead took over the wire, and it
    // names the lead by label and targets the lead's real process id — the one the tree carries.
    const lock = await todoBoard.waitForLock(COORDINATION.blocked);
    expect(lock.owner).toBe(LEAD);
    expect(lock.process).toBe(lead);

    await todoBoard.openAgent(COORDINATION.blocked);
    await sidebar.waitForRowSelected(LEAD);
    expect(await terminalPane.isMounted()).toBe(true);
  });

  it("lists the locked todo as current work and what the lead read as this session", async () => {
    const work = await terminalPane.waitForCurrentTodo(locked);
    expect(work.currentTodos).toEqual([locked]);
    expect(work.sessionTodos).toContain(loaded);
    expect(work.sessionScratchpads).toContain(COORDINATION.scratchpad);
  });

  it("returns from a current-work item to that todo, expanded and focused", async () => {
    await terminalPane.openSessionTodo(locked);
    await todoBoard.waitForTodo(COORDINATION.blocked);
    await orchestrationPane.waitForFocusedTodo(locked);
    expect(await todoBoard.isExpanded(COORDINATION.blocked)).toBe(true);
  });

  it("returns from a this-session item to that scratchpad, selected and focused", async () => {
    await sidebar.select(LEAD);
    await terminalPane.openSessionScratchpad(COORDINATION.scratchpad);
    await scratchpadPanel.waitForRoster();
    expect(await scratchpadPanel.waitForFocused(COORDINATION.scratchpad)).toEqual({
      name: COORDINATION.scratchpad,
      selected: true,
    });
  });

  it("fits the board with no horizontal overflow at the app's minimum window width", async () => {
    await orchestrationPane.showView("todos");
    await todoBoard.waitForTodo(COORDINATION.blocked);
    await shrinkWindowToMinimum();

    const overflowing = (await todoBoard.horizontalOverflow()).filter(
      (box) => box.scrollWidth > box.clientWidth,
    );
    expect(overflowing).toEqual([]);
  });

  it("empties the lead's session context when the lead stops", async () => {
    await sidebar.select(LEAD);
    await terminalPane.waitForCurrentTodo(locked);

    await sidebar.stop(LEAD);
    await sidebar.waitForRowStatus(LEAD, STOPPED);

    // Stopping the selected process moves the window off its pane, so re-open the stopped lead's
    // pane and read the header that is actually on screen: a context still recorded would render
    // there again, and only a record the core really dropped leaves it empty.
    await sidebar.select(LEAD);
    await terminalPane.waitForSessionWorkCleared();
  });
});
