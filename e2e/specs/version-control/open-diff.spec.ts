import { openProject } from "../../src/flows/openProject.js";
import { makeRepository } from "../../src/harness/repository.js";
import { diffPane } from "../../src/screens/DiffPane.js";
import { gitRail } from "../../src/screens/GitRail.js";
import { sidebar } from "../../src/screens/Sidebar.js";

// One changed path, with a line on each side of the change. Both lines are sentences no part of
// the app writes, so finding them in the split means version control really produced this diff
// from a real commit and a real edit.
const CHANGED_PATH = "notes.md";
const COMMITTED_LINE = "Ship the rail before the split";
const WORKING_LINE = "Ship the split beside the rail";

// The fixture process the reader has in front of them. The rail shows the repository the selected
// row belongs to, so a change is read beside something being worked on rather than on its own.
const SELECTED = "Echo";

// Opening a change from the version-control rail. The split is loaded on demand, on its own, the
// first time a path is opened — so this is the walk that first evaluates the diff viewer and the
// highlighting it is built on, inside the real webview, in the app as it is really built. Nothing
// headless reaches that: the bundler decides what the viewer's dependencies resolve to, and the
// test runners resolve them for themselves.
describe("opening a change from the version-control rail", () => {
  before(async () => {
    await openProject("basic", (root) =>
      makeRepository(root, {
        path: CHANGED_PATH,
        committed: `# Notes\n\n${COMMITTED_LINE}\n`,
        working: `# Notes\n\n${WORKING_LINE}\n`,
      }),
    );
    await sidebar.select(SELECTED);
  });

  it("shows the change with both of its sides", async () => {
    await gitRail.openChange(CHANGED_PATH);

    const shown = await diffPane.waitForText(WORKING_LINE);
    // The line the working tree no longer holds. It exists only in the commit, so the split can
    // only be showing it because the core really compared the two.
    expect(shown).toContain(COMMITTED_LINE);
  });

  it("leaves the rest of the window standing", async () => {
    // The whole app renders under one React root. A split that fails while it is being brought in
    // takes that root down with it, so the window a reader is left with is empty rather than a
    // window with no diff in it — the shell around the split is part of what opening one owes.
    expect(await sidebar.isRendered()).toBe(true);
    expect(await diffPane.text()).toContain(CHANGED_PATH);
  });
});
