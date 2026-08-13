import { $ } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";

const RAIL = 'aside[aria-label="Version control"]';
const CHANGES = '[role="tree"][aria-label="Changed files"]';

/** The version-control rail beside the main area: what has changed under the open project. */
export const gitRail = {
  /** Grants the repository trust needed for its changed-path controls to appear. */
  async trust(): Promise<void> {
    const trust = await $(RAIL).$("aria/Trust this project");
    await trust.waitForClickable({ timeout: WAIT.core });
    await trust.click();
    await trust.waitForExist({ reverse: true, timeout: WAIT.core });
  },

  /** Closes and reopens every folder, exercising the nested-row layout a reader sees. */
  async reexpandFolders(): Promise<void> {
    const collapse = await $(RAIL).$("aria/Collapse all folders");
    await collapse.waitForClickable({ timeout: WAIT.core });
    await collapse.click();

    const expand = await $(RAIL).$("aria/Expand all folders");
    await expand.waitForClickable({ timeout: WAIT.render });
    await expand.click();
  },

  /** The visible right edges of the rail and one changed path's trailing action. */
  async actionRightEdges(
    path: string,
  ): Promise<{ rail: number; action: number; actionWidth: number }> {
    const rail = await $(RAIL);
    const row = await rail.$(CHANGES).$(`aria/${path.split("/").at(-1)}`);
    await row.waitForDisplayed({ timeout: WAIT.core });

    const action = await rail.$(`aria/Discard the changes to ${path}`);
    // The control reserves real layout space while its hover/focus treatment is transparent. The
    // walk measures that box, so requiring its transient paint state makes the result depend on
    // whether a WebKit driver preserves synthetic hover between commands.
    await action.waitForExist({ timeout: WAIT.render });
    const [railX, railWidth, actionX, actionWidth] = await Promise.all([
      rail.getLocation("x"),
      rail.getSize("width"),
      action.getLocation("x"),
      action.getSize("width"),
    ]);
    return {
      rail: railX + railWidth,
      action: actionX + actionWidth,
      actionWidth,
    };
  },

  /**
   * Opens a changed path, clicking its row the way a reader does.
   *
   * The row carries the path's own name as its accessible name, so it is reached by that rather
   * than by where it sits — the tree groups by folder and re-walks itself whenever version
   * control says something moved, so a position is not a handle. Waiting on the row is a core
   * round trip: the rail shows nothing until the first real status read has answered.
   */
  async openChange(name: string): Promise<void> {
    const row = await $(RAIL).$(CHANGES).$(`aria/${name}`);
    await row.waitForDisplayed({ timeout: WAIT.core });
    await row.waitForClickable({ timeout: WAIT.render });
    await row.click();
  },
};
