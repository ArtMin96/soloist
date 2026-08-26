import { $, browser } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";

const RAIL = 'aside[aria-label="Version control"]';
const CHANGES = '[role="tree"][aria-label="Changed files"]';
const FILES = '[role="tree"][aria-label="Project files"]';

export interface PathPlacement {
  /** The scroll viewport is narrower than the tree it carries. */
  horizontallyScrollable: boolean;
  /** The full filename is visible after the tree is scrolled to its trailing edge. */
  nameVisibleAtEnd: boolean;
}

/** Where a changed row's trailing actions sit, against the rail edge they must stay inside. */
export interface ActionPlacement extends PathPlacement {
  /** The rail's own right edge. */
  railRight: number;
  /** The right edge of the row's actions, the last of them included. */
  actionsRight: number;
  /** Every trailing action remains inside the visible rail. */
  actionsVisible: boolean;
  /** The change-status letter remains visible beside the controls. */
  statusVisible: boolean;
  /** A click at each interactive action's centre lands on that action. */
  controlsReachable: boolean;
}

/** One row of the project's files: what it is called, and whether the app says it is ignored. */
export interface ProjectFileRow {
  name: string;
  ignored: boolean;
}

/** What the Files tree appends to a row's name in place of the dimming an ignored path shows. */
const IGNORED_SUFFIX = " (ignored)";

export interface FolderExpansion {
  /** The row is represented as an expandable folder, not as an unknown file. */
  folder: boolean;
  /** The folder was closed before the user opened it. */
  collapsedBefore: boolean;
  /** The folder reports itself open after the user acts on it. */
  expandedAfter: boolean;
  /** Child rows visible after opening the folder. */
  visibleChildren: string[];
}

/**
 * Switches the rail to the whole project and hands back the tree it lists it in.
 *
 * The listing is fetched only while that tab is the one being shown, so the tree existing is the
 * core having answered — which is why waiting on it is the whole arrange.
 */
async function openFilesTab() {
  const rail = await $(RAIL);
  const filesTab = await rail.$("aria/Files");
  await filesTab.waitForClickable({ timeout: WAIT.core });
  await filesTab.click();

  const tree = await rail.$(FILES);
  await tree.waitForExist({ timeout: WAIT.core });
  return tree;
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

  /** Closes the tree, opens one changed folder, and reads the children that become visible. */
  async expandChangedFolder(
    folderName: string,
    childNames: string[],
  ): Promise<FolderExpansion> {
    const rail = await $(RAIL);
    const tree = await rail.$(CHANGES);
    const collapse = await rail.$("aria/Collapse all folders");
    await collapse.waitForClickable({ timeout: WAIT.core });
    await collapse.click();

    const folder = await tree.$(`aria/${folderName}`);
    await folder.waitForDisplayed({ timeout: WAIT.render });
    const folderMarker = await folder.getAttribute("data-folder");
    const collapsedBefore = (await folder.getAttribute("aria-expanded")) === "false";
    await folder.click();

    await browser.waitUntil(
      async () =>
        (await tree.$(`aria/${folderName}`).getAttribute("aria-expanded")) === "true",
      {
        timeout: WAIT.render,
        timeoutMsg: `the changed folder ${folderName} never opened`,
      },
    );

    const visibleChildren: string[] = [];
    for (const childName of childNames) {
      const child = await tree.$(`aria/${childName}`);
      await child.waitForDisplayed({ timeout: WAIT.render });
      visibleChildren.push(childName);
    }

    return {
      folder: folderMarker !== null,
      collapsedBefore,
      expandedAfter:
        (await tree.$(`aria/${folderName}`).getAttribute("aria-expanded")) === "true",
      visibleChildren,
    };
  },

  /**
   * Where a changed path's trailing actions sit, and whether its controls still answer clicks.
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
    const discard = await rail
      .$(CHANGES)
      .$(`aria/Discard the changes to ${path}`);
    const stage = await rail.$(CHANGES).$(`aria/Stage ${path}`);
    // The actions appear only once the core has answered which paths hold restorable work, and
    // only while the row's folders are open — so this existence is the whole arrange, waited on.
    await discard.waitForExist({ timeout: WAIT.core });
    await stage.waitForExist({ timeout: WAIT.core });

    return browser.execute(
      (
        railElement: HTMLElement,
        discardElement: HTMLElement,
        stageElement: HTMLElement,
      ) => {
        const actions = discardElement.parentElement;
        if (actions === null) {
          throw new Error(
            "the discard control no longer sits among a changed row's actions",
          );
        }
        const row = actions.closest('[data-slot="tree-item"]');
        const viewport = actions.closest('[data-slot="scroll-area-viewport"]');
        const label = row?.querySelector<HTMLElement>(
          '[data-slot="tree-item-label"]',
        );
        if (!(viewport instanceof HTMLElement) || label == null) {
          throw new Error(
            "the changed row no longer sits in the repository tree scroll viewport",
          );
        }
        const horizontallyScrollable =
          viewport.scrollWidth > viewport.clientWidth;
        viewport.scrollLeft = viewport.scrollWidth;
        const labelBox = label.getBoundingClientRect();
        const viewportBox = viewport.getBoundingClientRect();
        const actionsBox = actions.getBoundingClientRect();
        const railBox = railElement.getBoundingClientRect();
        const actionElements = Array.from(actions.children).filter(
          (element): element is HTMLElement => element instanceof HTMLElement,
        );
        const status = actions.querySelector<HTMLElement>('[role="img"]');
        const reaches = (control: HTMLElement) => {
          const box = control.getBoundingClientRect();
          const hit = document.elementFromPoint(
            box.x + box.width / 2,
            box.y + box.height / 2,
          );
          return !control.hasAttribute("disabled") && hit !== null && control.contains(hit);
        };
        return {
          railRight: railBox.right,
          actionsRight: actionsBox.right,
          actionsVisible: actionElements.every((element) => {
            const box = element.getBoundingClientRect();
            return box.left >= viewportBox.left && box.right <= railBox.right;
          }),
          statusVisible:
            status !== null &&
            status.getBoundingClientRect().left >= viewportBox.left &&
            status.getBoundingClientRect().right <= railBox.right,
          controlsReachable: reaches(discardElement) && reaches(stageElement),
          horizontallyScrollable,
          nameVisibleAtEnd:
            labelBox.left >= viewportBox.left &&
            labelBox.right <= actionsBox.left,
        };
      },
      rail,
      discard,
      stage,
    );
  },

  /**
   * Every row the Files tab shows, and whether the app reports each path as ignored.
   *
   * One evaluation, because the listing is one answer from the core: the rows are all there or
   * none of them are, and reading them one at a time would only race the tree rebuilding itself
   * when version control reports a change.
   */
  async projectFiles(): Promise<ProjectFileRow[]> {
    const tree = await openFilesTab();
    const anyRow = await tree.$('[data-slot="tree-item-label"]');
    await anyRow.waitForExist({ timeout: WAIT.core });

    return browser.execute(
      (treeElement: HTMLElement, suffix: string) =>
        Array.from(
          treeElement.querySelectorAll<HTMLElement>('[data-slot="tree-item-label"]'),
        ).map((label) => {
          const shown = label.textContent ?? "";
          const ignored = shown.endsWith(suffix);
          return {
            name: ignored ? shown.slice(0, -suffix.length) : shown,
            ignored,
          };
        }),
      tree,
      IGNORED_SUFFIX,
    );
  },

  /** Reveals a nested project file and measures it at the tree's trailing scroll edge. */
  async filePlacement(path: string): Promise<PathPlacement> {
    const tree = await openFilesTab();
    const rail = await $(RAIL);
    const expand = await rail.$("aria/Expand all folders");
    await expand.waitForClickable({ timeout: WAIT.render });
    await expand.click();

    const name = path.split("/").at(-1) ?? path;
    const row = await tree.$(`aria/${name}`);
    await row.waitForExist({ timeout: WAIT.render });

    return browser.execute((rowElement: HTMLElement) => {
      const viewport = rowElement.closest('[data-slot="scroll-area-viewport"]');
      const label = rowElement.querySelector<HTMLElement>(
        '[data-slot="tree-item-label"]',
      );
      if (!(viewport instanceof HTMLElement) || label === null) {
        throw new Error(
          "the project file no longer sits in its tree scroll viewport",
        );
      }
      const horizontallyScrollable =
        viewport.scrollWidth > viewport.clientWidth;
      viewport.scrollLeft = viewport.scrollWidth;
      const labelBox = label.getBoundingClientRect();
      const viewportBox = viewport.getBoundingClientRect();
      return {
        horizontallyScrollable,
        nameVisibleAtEnd:
          labelBox.left >= viewportBox.left &&
          labelBox.right <= viewportBox.right,
      };
    }, row);
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
