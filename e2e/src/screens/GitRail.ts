import { $ } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";

const RAIL = 'aside[aria-label="Version control"]';
const CHANGES = '[role="tree"][aria-label="Changed files"]';

/** The version-control rail beside the main area: what has changed under the open project. */
export const gitRail = {
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
