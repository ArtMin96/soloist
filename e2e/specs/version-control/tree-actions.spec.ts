import { openProject } from "../../src/flows/openProject.js";
import {
  addUntrackedFiles,
  makeRepository,
} from "../../src/harness/repository.js";
import { gitRail } from "../../src/screens/GitRail.js";
import { sidebar } from "../../src/screens/Sidebar.js";

const CHANGED_PATH =
  "packages/desktop/src/features/version-control/components/tree/rows/presentation/state/controllers/change-actions/items/changed-file.ts";
const UNTRACKED_DIRECTORY = "untracked-work";
const UNTRACKED_CHILDREN = ["first-draft.ts", "review-notes.md"];
const SELECTED = "Echo";

describe("repository tree overflow", () => {
  before(async () => {
    await openProject("basic", (root) => {
      makeRepository(root, {
        path: CHANGED_PATH,
        committed: "export const state = 'before';\n",
        working: "export const state = 'after';\n",
      });
      addUntrackedFiles(
        root,
        Object.fromEntries(
          UNTRACKED_CHILDREN.map((name) => [
            `${UNTRACKED_DIRECTORY}/${name}`,
            `${name}\n`,
          ]),
        ),
      );
    });
    await sidebar.select(SELECTED);
    await gitRail.trust();
  });

  it("keeps a nested row's actions inside the rail after its directories expand", async () => {
    await gitRail.reexpandFolders();

    const placement = await gitRail.actionPlacement(CHANGED_PATH);
    expect(placement.horizontallyScrollable).toBe(true);
    expect(placement.nameVisibleAtEnd).toBe(true);
    expect(placement.actionsRight).toBeLessThanOrEqual(placement.railRight);
    expect(placement.actionsVisible).toBe(true);
    expect(placement.statusVisible).toBe(true);
    expect(placement.controlsReachable).toBe(true);
  });

  it("opens an untracked directory and reveals the files inside it", async () => {
    const expansion = await gitRail.expandChangedFolder(
      UNTRACKED_DIRECTORY,
      UNTRACKED_CHILDREN,
    );

    expect(expansion.folder).toBe(true);
    expect(expansion.collapsedBefore).toBe(true);
    expect(expansion.expandedAfter).toBe(true);
    expect(expansion.visibleChildren).toEqual(UNTRACKED_CHILDREN);
  });

  it("reveals a long nested filename at the end of the Files tree", async () => {
    const placement = await gitRail.filePlacement(CHANGED_PATH);

    expect(placement.horizontallyScrollable).toBe(true);
    expect(placement.nameVisibleAtEnd).toBe(true);
  });
});
