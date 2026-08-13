import { openProject } from "../../src/flows/openProject.js";
import { captureProof } from "../../src/harness/artifacts.js";
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

    const right = await gitRail.actionRightEdges(CHANGED_PATH);
    expect(right.actionWidth).toBeGreaterThan(0);
    expect(right.action).toBeLessThanOrEqual(right.rail);
    await captureProof("nested-tree-action-layout", right);
  });
});
