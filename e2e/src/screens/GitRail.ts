import { $, browser } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";

const RAIL = 'aside[aria-label="Version control"]';
const CHANGES = '[role="tree"][aria-label="Changed files"]';

/** Where a changed row's trailing actions sit, against the rail edge they must stay inside. */
export interface ActionPlacement {
  /** The rail's own right edge. */
  railRight: number;
  /** The right edge of the row's actions, the last of them included. */
  actionsRight: number;
  /** Whether a click at the centre of the last action lands on that action. */
  reachable: boolean;
}

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

  /**
   * Where a changed path's trailing actions sit, and whether the last of them still answers a
   * click there.
   *
   * Reached through the discard control — the one action named for the path — and then measured as
   * the group it belongs to. The group is what the rail's edge constrains: the controls sit on one
   * line, so whatever that edge cuts off, it cuts off from the end, and measuring the first of them
   * leaves the rest free to be clipped away.
   *
   * Nothing is hovered, and no hovered paint state is required: the discard control keeps its box
   * whatever its opacity, so this is the box a hovering reader gets, while the pointer this driver
   * synthesizes never puts WebKitGTK into `:hover` at all — measured, `document.querySelectorAll(
   * ":hover")` stays empty across a `moveTo`, so waiting for an `opacity-0` control to become
   * visible can only time out. The sidebar's own ••• control is revealed the same way and avoids
   * the pointer for the same reason.
   *
   * One evaluation, so every edge comes from the same layout: the tree rebuilds itself whenever
   * version control reports a change, and reads either side of one would compare two frames.
   */
  async actionPlacement(path: string): Promise<ActionPlacement> {
    const rail = await $(RAIL);
    const discard = await rail.$(CHANGES).$(`aria/Discard the changes to ${path}`);
    // The actions appear only once the core has answered which paths hold restorable work, and
    // only while the row's folders are open — so this existence is the whole arrange, waited on.
    await discard.waitForExist({ timeout: WAIT.core });

    return browser.execute(
      (railElement: HTMLElement, discardElement: HTMLElement) => {
        const actions = discardElement.parentElement;
        if (actions === null) {
          throw new Error("the discard control no longer sits among a changed row's actions");
        }
        const last = actions.lastElementChild ?? actions;
        const lastBox = last.getBoundingClientRect();
        const hit = document.elementFromPoint(
          lastBox.x + lastBox.width / 2,
          lastBox.y + lastBox.height / 2,
        );
        return {
          railRight: railElement.getBoundingClientRect().right,
          actionsRight: actions.getBoundingClientRect().right,
          reachable: hit !== null && last.contains(hit),
        };
      },
      rail,
      discard,
    );
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
