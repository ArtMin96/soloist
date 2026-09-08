import type { DocumentRole, ProcStatus, ProjectView, TodoStatus } from "@domain";
import {
  COORDINATION,
  LEAD_AGENT,
  requestLeadCoordination,
} from "../../src/harness/leadAgent.js";
import { shrinkWindowToMinimum } from "../../src/harness/window.js";
import { launchAgent } from "../../src/flows/launch.js";
import { openProject } from "../../src/flows/openProject.js";
import { orchestrationPane } from "../../src/screens/OrchestrationPane.js";
import { scratchpadBoard } from "../../src/screens/ScratchpadBoard.js";
import { sidebar } from "../../src/screens/Sidebar.js";
import { terminalPane } from "../../src/screens/TerminalPane.js";
import { todoBoard, type FitReading } from "../../src/screens/TodoBoard.js";

// The lead the walk launches: over the real MCP/IPC wire its stub seeds a blocker chain, locks one
// todo, and reads another todo and a scratchpad through tools — the accesses the core records as
// the lead's session context. Its label is what the locked row's agent control names.
const LEAD = LEAD_AGENT.lead;
const RUNNING: ProcStatus = "Running";
const STOPPED: ProcStatus = "Stopped";
// The status the lead declared on every todo it created; the blocker gate is derived, not declared.
const OPEN: TodoStatus = "open";
const DONE: TodoStatus = "done";
// The roles the sidebar derives from the lead's lock and recorded tool accesses. Every todo here
// was *created* by the lead through a tool, which the core records as a write, and a later tool
// read never downgrades a recorded write — so nothing in this fixture can render as merely read.
const IMPLEMENTING: DocumentRole = "implementing";
const EDITING: DocumentRole = "editing";

/**
 * The handle a humanized document name was rendered from — "Release readiness" → "release-readiness".
 * Asserting the handle rather than the prose keeps the walk on the identity the core carried, so a
 * change to how a name is dressed for reading is not a failure of this walk.
 */
const asHandle = (label: string) => label.trim().toLowerCase().replace(/\s+/g, "-");

/** The boxes whose content is wider than themselves — the ones a user would have to scroll sideways. */
const overflowing = (boxes: FitReading[]) =>
  boxes.filter((box) => box.scrollWidth > box.clientWidth);

// The todo workspace as a user moves through it, in the real window against the real core: a
// board row carries the id and blocker count the core computed from a chain built over the wire;
// a card hands the pane to its detail panel and Back retraces the way in; the row the lead locked
// names the lead and opens its terminal; the sidebar's Todos and Scratchpads groups list what the
// lead holds and touched, each at the role the core derived, and each row leads straight to the
// surface holding that document; the board's own surface fits the narrowest window the app allows;
// a todo marked done leaves the group while the rest stay; and stopping the lead empties both
// groups. Every assertion keys on state only the core produced — a lock held by a bound session,
// an access recorded from a real tool call, a focus the engine really moved.
describe("the todo workspace", () => {
  let project: ProjectView;
  /** The lead's process id as the core assigned it, read off the lineage tree. */
  let lead: number;
  /** The id of the todo the lead locked over the wire. */
  let locked: number;
  /** The id of the todo the lead read through a tool without locking it. */
  let loaded: number;
  /** The id of the todo gating the locked one — created by the lead, never read back. */
  let blocker: number;
  /** The id of the scratchpad the lead wrote and then read back over the wire. */
  let scratchpad: number;

  before(async () => {
    project = await openProject("orchestration");

    await requestLeadCoordination();
    await launchAgent(LEAD);
    await sidebar.waitForRowStatus(LEAD, RUNNING);

    await sidebar.openOrchestration(project.name);
    const [leadNode] = await orchestrationPane.waitForNodes(LEAD);
    lead = leadNode!.id;

    // The scratchpad's id comes from its own board, the way each todo id below comes from the todo
    // board: the sidebar rows are what this walk checks, so the identity they must carry has to be
    // read from the surface that owns the document, never from the row under test.
    await orchestrationPane.showView("scratchpads");
    scratchpad = await scratchpadBoard.waitForRowId(COORDINATION.scratchpad);

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
    blocker = (await todoBoard.read(COORDINATION.blocker))!.id;

    expect(await todoBoard.todoRef(COORDINATION.blocked)).toBe(`#${locked}`);
    expect(row!.status).toBe(OPEN);

    // The counts are the core's derived `blocked_by` for two different chains the lead set over
    // the wire — one blocker on the first todo, two on the second — so the singular and the plural
    // each come from a distinct real chain, not from one number the window could have guessed.
    await todoBoard.waitForBlocked(COORDINATION.blocked, true);
    expect(await todoBoard.blockerText(COORDINATION.blocked)).toBe("1 unmet blocker");
    expect(await todoBoard.blockerText(COORDINATION.commented)).toBe("2 unmet blockers");
  });

  it("hands the pane to a card's detail panel, and returns to the row it came from", async () => {
    const opened = await todoBoard.open(COORDINATION.blocked);
    expect(opened.detail?.status).toBe(OPEN);

    // The panel is where a todo states its provenance now that the row has stopped naming it, and
    // this todo's is a link the lead made over the wire — so only the core can have put it here.
    expect(asHandle(opened.detail?.scratchpad ?? "")).toBe(COORDINATION.scratchpad);

    // Where focus ends up is the assertion, read off `document.activeElement` rather than off any
    // component's intent: the board aims it at the arriving panel in a layout effect, and both
    // halves of that were proven to fail — deleting the aim reddens this, and pointing the return
    // aim at a dead handle reddens the row focus below, each on its own.
    expect(opened.backFocused).toBe(true);

    await todoBoard.back();

    // The panel that left is really dropped, not parked off screen: the board keeps the todo
    // rendered for the length of the slide out and unmounts it on the track's own `transitionend`,
    // which only a transition that really ran can raise.
    await todoBoard.waitForDetailDropped();
    await todoBoard.waitForFocusedRow(locked);
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

  it("lists the lead's documents under the project at the roles the core derived", async () => {
    // Implementing is the lock the lead took over the wire, and only a lock produces it: the wait
    // ends on the exact pair, so a row painted at any other role is the failure, not a slow read.
    const row = await sidebar.waitForDocumentRole("todos", String(locked), IMPLEMENTING);
    expect(row.text).toContain(COORDINATION.blocked);

    // One snapshot of each group, checked whole: the two other todos were created by the lead and
    // hold no lock, so they are editing — and the one it later read through a tool stays editing,
    // because a recorded write is never downgraded by a read. The scratchpad was written and then
    // read, so it carries the same answer for the same reason.
    const todos = await sidebar.documentRows("todos");
    expect(Object.fromEntries(todos.map((todo) => [todo.handle, todo.role]))).toEqual({
      [String(locked)]: IMPLEMENTING,
      [String(loaded)]: EDITING,
      [String(blocker)]: EDITING,
    });
    const scratchpads = await sidebar.documentRows("scratchpads");
    expect(scratchpads.map(({ handle, role }) => ({ handle, role }))).toEqual([
      { handle: String(scratchpad), role: EDITING },
    ]);
  });

  it("opens that todo from its Todos row, focused", async () => {
    await sidebar.openDocument("todos", String(locked));
    await todoBoard.waitForTodo(COORDINATION.blocked);

    // The inbound half lands on the todo itself, not on a board the reader then has to search: the
    // detail panel opens on it whatever the list's filter and grouping happen to be, and focus goes
    // with it rather than staying behind in the panel that just went inert.
    const landed = await todoBoard.waitForDetail(COORDINATION.blocked);
    expect(landed.backFocused).toBe(true);
  });

  it("opens that scratchpad from its Scratchpads row, focused", async () => {
    await sidebar.openDocument("scratchpads", String(scratchpad));

    // The inbound half lands on the document itself, not on a board the reader then has to search:
    // the detail panel opens on it whatever the list's filter and sort happen to be, and focus goes
    // with it rather than staying behind in the panel that just went inert — the same shape the
    // to-do half of this walk asserts.
    const landed = await scratchpadBoard.waitForDetail(COORDINATION.scratchpad);
    expect(landed.backFocused).toBe(true);
  });

  it("fits the board with no horizontal overflow at the app's minimum window width", async () => {
    await orchestrationPane.showView("todos");
    await todoBoard.waitForTodo(COORDINATION.blocked);
    await shrinkWindowToMinimum();

    expect(overflowing(await todoBoard.horizontalOverflow())).toEqual([]);
  });

  it("fits the detail panel at that width too, where the whole todo is finally readable", async () => {
    // The refactor moved half the board onto this surface — the document, its blockers, its
    // discussion and every action — so the width the list was measured against is owed here too.
    await todoBoard.open(COORDINATION.blocked);

    expect(overflowing(await todoBoard.detailOverflow())).toEqual([]);
  });

  it("drops a todo from the group once it is done, keeping the rest", async () => {
    // Done is the one status the group never lists, however many processes hold or touched the
    // todo — the lead still holds the lock on the one it completes here. The lead stub has no
    // complete verb, so the status change is driven through the board's detail panel, which routes
    // to the same core command; the locked todo is gated by its blocker, so that goes first, and
    // each completion is watched leave on its own.
    await todoBoard.complete(COORDINATION.blocker);
    await todoBoard.waitForStatus(COORDINATION.blocker, DONE);
    await sidebar.waitForDocumentGone("todos", String(blocker));

    await todoBoard.complete(COORDINATION.blocked);
    await todoBoard.waitForStatus(COORDINATION.blocked, DONE);
    await sidebar.waitForDocumentGone("todos", String(locked));

    // What stays is checked whole: the read todo at its role, and the scratchpad untouched by any
    // of it — a drop that swept the group, or that keyed on the wrong todo, cannot pass this.
    const todos = await sidebar.documentRows("todos");
    expect(todos.map(({ handle, role }) => ({ handle, role }))).toEqual([
      { handle: String(loaded), role: EDITING },
    ]);
    const scratchpads = await sidebar.documentRows("scratchpads");
    expect(scratchpads.map((row) => row.handle)).toEqual([String(scratchpad)]);
  });

  it("empties both groups when the lead stops", async () => {
    await sidebar.waitForDocumentRole("todos", String(loaded), EDITING);

    await sidebar.stop(LEAD);
    await sidebar.waitForRowStatus(LEAD, STOPPED);

    // The sidebar is always on screen, so the drop is read where it happens. A stopped process
    // stays in the registry, and the close hook releases its todo lock straight into the store
    // without announcing it — so the only thing that tells the window these rows are gone is the
    // session record announcing its own drop — this step is what that announcement exists for.
    await sidebar.waitForNoDocuments("todos");
    await sidebar.waitForNoDocuments("scratchpads");
  });
});
