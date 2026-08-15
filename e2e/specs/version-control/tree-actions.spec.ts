import { openProject } from "../../src/flows/openProject.js";
import { makeRepository } from "../../src/harness/repository.js";
import { gitRail } from "../../src/screens/GitRail.js";
import { sidebar } from "../../src/screens/Sidebar.js";

const CHANGED_PATH =
  "packages/desktop/src/features/version-control/components/tree/rows/changed-file.ts";
const SELECTED = "Echo";

describe("changed-file tree actions", () => {
  before(async () => {
    await openProject("basic", (root) =>
      makeRepository(root, {
        path: CHANGED_PATH,
        committed: "export const state = 'before';\n",
        working: "export const state = 'after';\n",
      }),
    );
    await sidebar.select(SELECTED);
    await gitRail.trust();
  });

  it("keeps a nested row's actions inside the rail after its directories expand", async () => {
    await gitRail.reexpandFolders();

    const placement = await gitRail.actionPlacement(CHANGED_PATH);
    expect(placement.actionsRight).toBeLessThanOrEqual(placement.railRight);
    // Two halves of one claim. A row wide enough to carry its actions past the rail's edge has
    // them clipped away rather than merely misplaced, so the reader is left with a control they
    // can see no part of and cannot press — which the edges alone would not say.
    expect(placement.reachable).toBe(true);
  });
});
