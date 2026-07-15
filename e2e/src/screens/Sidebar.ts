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
const ACTIVITY = ":scope > span[data-activity]";
const META = '[data-testid="process-meta"]';

// The indicator only ever swaps `data-status` out for `data-activity` while the process is
// Running (a stopped agent has no activity to report), so an activity marker *is* a Running
// status. Any row carrying neither marker means the markup changed — that must fail the read,
// never default it, or a status assertion could pass against a row that reports nothing.
const RUNNING: ProcStatus = "Running";
const STOPPED: ProcStatus = "Stopped";

/** One process row as the sidebar renders it. */
export interface RowHandle {
  label: string;
  status: ProcStatus;
  selected: boolean;
  /** The first discovered port the row's telemetry shows, or `null` while it shows none. */
  port: number | null;
}

/** What one row's DOM carries, before the status/activity markers are resolved to a status. */
interface RowSnapshot {
  label: string;
  status: string | null;
  hasActivity: boolean;
  selected: boolean;
  meta: string | null;
}

// The telemetry read-out formats a discovered port as `:1234` (see the UI's formatPorts).
function portOf(meta: string | null): number | null {
  const match = meta === null ? null : /:(\d+)/.exec(meta);
  return match === null ? null : Number(match[1]);
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
    const snapshots: RowSnapshot[] = await browser.execute(
      (nav: string, row: string, label: string, status: string, activity: string, meta: string) => {
        const tree = document.querySelector(nav);
        if (!tree) return [];
        return [...tree.querySelectorAll(row)].map((node) => ({
          label: (node.querySelector(label) as HTMLElement | null)?.innerText.trim() ?? "",
          status: node.querySelector(status)?.getAttribute("data-status") ?? null,
          hasActivity: node.querySelector(activity) !== null,
          selected: node.getAttribute("aria-selected") === "true",
          // textContent rather than innerText: the read-out hides under the controls while the
          // row is selected or hovered, and a hidden element's innerText reads empty.
          meta: node.querySelector(meta)?.textContent ?? null,
        }));
      },
      NAV,
      ROW,
      LABEL,
      STATUS,
      ACTIVITY,
      META,
    );
    return snapshots.map(({ label, status, hasActivity, selected, meta }) => {
      if (status === null && !hasActivity) {
        throw new Error(
          `sidebar row "${label}" renders neither data-status nor data-activity — ` +
            `the indicator markup changed and the harness can no longer read its status`,
        );
      }
      // The attribute is written from the typed `ProcStatus` the UI renders, so the string is
      // trusted rather than re-validated against a second copy of the enum's values.
      return {
        label,
        status: status === null ? RUNNING : (status as ProcStatus),
        selected,
        port: portOf(meta),
      };
    });
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

  /** Waits until the row labelled `label` reports `status` — a supervision round trip. */
  async waitForRowStatus(label: string, status: ProcStatus): Promise<void> {
    let last: ProcStatus | undefined;
    try {
      await browser.waitUntil(
        async () => {
          const row = (await this.rows()).find((candidate) => candidate.label === label);
          last = row?.status;
          return last === status;
        },
        { timeout: WAIT.core },
      );
    } catch {
      throw new Error(
        `sidebar row "${label}" never reported "${status}"; last seen: ${last ?? "no such row"}`,
      );
    }
  },

  /** Clicks the row labelled `label`, selecting its process. */
  async select(label: string): Promise<void> {
    // First prove the row is rendered at all — that failure names the rows that are — so a
    // clickability timeout below can only mean the row exists and something obscures it.
    await this.waitForRow(label);
    const row = await this.rowElement(label);
    await row.waitForClickable({ timeout: WAIT.render });
    await row.click();
  },

  /** Clicks Trust on the row's control cluster — the core trust gate, cleared per command. */
  async trust(label: string): Promise<void> {
    await this.clickControl(label, "Trust");
  },

  /** Clicks Start on the row's control cluster. */
  async start(label: string): Promise<void> {
    await this.clickControl(label, "Start");
  },

  /** Clicks Stop on the row's control cluster. */
  async stop(label: string): Promise<void> {
    await this.clickControl(label, "Stop");
  },

  /** Clicks Restart on the row's control cluster. */
  async restart(label: string): Promise<void> {
    await this.clickControl(label, "Restart");
  },

  /**
   * Stops the row's process if it is currently Running, and waits for it to rest. For spec-file
   * cleanup: every spec file leaves nothing running, so no later app session boots into another
   * session's leftovers. Tolerant of the row not existing — a failed spec must not have its
   * real failure masked by cleanup.
   */
  async stopIfRunning(label: string): Promise<void> {
    const row = (await this.rows()).find((candidate) => candidate.label === label);
    if (row === undefined || row.status !== RUNNING) return;
    await this.select(label);
    await this.stop(label);
    await this.waitForRowStatus(label, STOPPED);
  },

  /**
   * Waits until the row's telemetry shows a discovered port — one differing from `previous`,
   * when given — then returns it. Ports are discovered on a sampling interval while the process
   * runs, so this is a core round trip; the telemetry only renders while the row is Running.
   */
  async waitForPort(label: string, previous?: number): Promise<number> {
    let port: number | null = null;
    try {
      await browser.waitUntil(
        async () => {
          const row = (await this.rows()).find((candidate) => candidate.label === label);
          port = row?.port ?? null;
          return port !== null && port !== previous;
        },
        { timeout: WAIT.core },
      );
    } catch {
      /* fall through to the check below, which reports it */
    }
    if (port === null || port === previous) {
      throw new Error(
        previous === undefined
          ? `sidebar row "${label}" never showed a discovered port`
          : `sidebar row "${label}" never showed a port other than :${previous}`,
      );
    }
    return port;
  },

  /** Whether a subtype group header (Agents / Terminals / Commands) is rendered. */
  async hasGroup(name: string): Promise<boolean> {
    return $(NAV).$(`span*=${name}`).isDisplayed();
  },

  /**
   * The row element itself, found by the text of its label span. The controls a spec clicks are
   * revealed for the selected row, so callers `select` before reaching for one; a control is
   * waited on until clickable, which is when the reveal has landed.
   */
  rowElement(label: string) {
    return $(NAV).$(`.//*[@role="treeitem"][./span[normalize-space(text())="${label}"]]`);
  },

  async clickControl(label: string, action: "Trust" | "Start" | "Stop" | "Restart"): Promise<void> {
    const control = await this.rowElement(label).$(`.//button[@aria-label="${action}"]`);
    await control.waitForClickable({ timeout: WAIT.render });
    await control.click();
  },
};
