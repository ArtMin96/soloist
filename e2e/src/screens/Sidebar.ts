import type { ProcStatus } from "@domain";
import { $, browser } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";

const NAV = 'nav[aria-label="Projects"]';
const ROW = '[role="treeitem"]';

// A row lays out an optional spacer or chevron, the status indicator, the label, then its
// telemetry. The indicator carries `data-status` — or `data-activity` instead, once a running agent
// reports what it is doing — so the label is the only direct child span carrying none of those
// markers. That is a structural handle rather than a styling one: it survives a restyle and breaks
// only if the row genuinely stops rendering a label.
const LABEL = ":scope > span:not([aria-hidden]):not([data-status]):not([data-activity])";
const STATUS = ":scope > span[data-status]";

/** One process row as the sidebar renders it. */
export interface RowHandle {
  label: string;
  status: ProcStatus;
  selected: boolean;
}

/** The left rail: the project tree and its process rows. */
export const sidebar = {
  async waitUntilReady(): Promise<void> {
    await $(NAV).waitForDisplayed({ timeout: WAIT.render });
  },

  /** Waits for a project to appear in the tree by its display name. */
  async waitForProject(name: string): Promise<void> {
    await $(NAV)
      .$(`span*=${name}`)
      .waitForDisplayed({ timeout: WAIT.core });
  },

  /**
   * Every process row currently rendered, read in one pass.
   *
   * Read atomically rather than row-by-row: a live agent re-renders its row as its status and
   * activity change, so walking the rows one driver call at a time races the re-render and dies on
   * a stale element reference. One snapshot cannot tear, and cannot flake for a reason that has
   * nothing to do with what is being asserted.
   */
  async rows(): Promise<RowHandle[]> {
    return browser.execute(
      (nav: string, row: string, label: string, status: string) => {
        const tree = document.querySelector(nav);
        if (!tree) return [];
        return [...tree.querySelectorAll(row)].map((node) => ({
          label: (node.querySelector(label) as HTMLElement | null)?.innerText.trim() ?? "",
          // The indicator swaps its status marker for an activity once an agent is running and
          // reporting one, so a row showing an activity is by definition Running.
          status: node.querySelector(status)?.getAttribute("data-status") ?? "Running",
          selected: node.getAttribute("aria-selected") === "true",
        }));
      },
      NAV,
      ROW,
      LABEL,
      STATUS,
    ) as Promise<RowHandle[]>;
  },

  /** Waits until a row labelled exactly `label` is rendered, then returns it. */
  async waitForRow(label: string): Promise<RowHandle> {
    let found: RowHandle | undefined;
    let seen: string[] = [];
    try {
      await browser.waitUntil(
        async () => {
          const rows = await this.rows();
          seen = rows.map((row) => row.label);
          found = rows.find((row) => row.label === label);
          return found !== undefined;
        },
        { timeout: WAIT.core },
      );
    } catch {
      // Reported here rather than through `timeoutMsg`, which is interpolated when the options
      // object is built — before a single poll has run, so it can only ever describe the initial
      // state. Naming the rows that were actually rendered turns "not found" into the answer.
      throw new Error(
        `no sidebar row labelled "${label}" appeared; rendered rows: ${JSON.stringify(seen)}`,
      );
    }
    return found as RowHandle;
  },

  /** Whether a subtype group header (Agents / Terminals / Commands) is rendered. */
  async hasGroup(name: string): Promise<boolean> {
    return $(NAV).$(`span*=${name}`).isDisplayed();
  },
};
