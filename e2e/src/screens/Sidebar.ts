import type { ProcStatus } from "@domain";
import { $, browser } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";
import { ROW_ACTIVITY, ROW_STATUS, ROW_TEXT } from "./indicatorRow.js";
import { trustDialog } from "./TrustDialog.js";

const NAV = 'nav[aria-label="Projects"]';
const ROW = '[role="treeitem"]';
const META = '[data-testid="process-meta"]';

// The indicator only ever swaps `data-status` out for `data-activity` while the process is
// Running (a stopped agent has no activity to report), so an activity marker *is* a Running
// status. Any row carrying neither marker means the markup changed — that must fail the read,
// never default it, or a status assertion could pass against a row that reports nothing.
const RUNNING: ProcStatus = "Running";
const STOPPED: ProcStatus = "Stopped";

/** The per-row control cluster's actions, by their accessible names. */
type RowControl =
  | "Trust"
  | "Resume last session"
  | "Start"
  | "Stop"
  | "Restart";

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
    await $(NAV).$(`span*=${name}`).waitForDisplayed({ timeout: WAIT.core });
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
      (
        nav: string,
        row: string,
        label: string,
        status: string,
        activity: string,
        meta: string,
      ) => {
        const tree = document.querySelector(nav);
        if (!tree) return [];
        return [...tree.querySelectorAll(row)].map((node) => ({
          label:
            (
              node.querySelector(label) as HTMLElement | null
            )?.innerText.trim() ?? "",
          status:
            node.querySelector(status)?.getAttribute("data-status") ?? null,
          hasActivity: node.querySelector(activity) !== null,
          selected: node.getAttribute("aria-selected") === "true",
          // textContent rather than innerText: the read-out hides under the controls while the
          // row is selected or hovered, and a hidden element's innerText reads empty.
          meta: node.querySelector(meta)?.textContent ?? null,
        }));
      },
      NAV,
      ROW,
      ROW_TEXT,
      ROW_STATUS,
      ROW_ACTIVITY,
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
          const row = (await this.rows()).find(
            (candidate) => candidate.label === label,
          );
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

  /** Reviews the exact command shown by the row's Trust affordance, then grants it. */
  async trust(label: string, command: string): Promise<void> {
    await this.clickControl(label, "Trust");
    await trustDialog.waitUntilOpen();
    if (!(await trustDialog.listsCommand(label))) {
      throw new Error(`the trust review did not list "${label}"`);
    }
    if (!(await trustDialog.showsCommand(command))) {
      throw new Error(
        `the trust review did not show command ${JSON.stringify(command)}`,
      );
    }
    await trustDialog.trust(label);
    await trustDialog.waitUntilClosed();
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
    const row = (await this.rows()).find(
      (candidate) => candidate.label === label,
    );
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
          const row = (await this.rows()).find(
            (candidate) => candidate.label === label,
          );
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
   * Opens a project's orchestration pane the way a keyboard user does — reveal the project row's
   * ••• actions button, open its menu, and choose the Orchestration view.
   *
   * Two WebKitGTK-under-classic-WebDriver realities shape this: the button is `opacity-0` until the
   * row is hovered or focused, and the synthetic pointer neither reliably triggers `:hover` nor
   * fires the `pointerdown` the menu opens on. So the button is focused (the row's `:focus-within`
   * reveals it) and its menu opened with Enter — the genuine keyboard path — then the view's menu
   * item is chosen inside the open menu.
   *
   * Both the synthetic focus and the Enter are racy on a slow runner: an Enter dispatched before the
   * focus settles on the button is dropped, and the menu never opens. Pressing once and waiting would
   * then fail the whole spec — the suite runs with no retries — so the focus+Enter is repeated until
   * the menu is actually displayed. The re-press is skipped once the menu is open, so a menu that has
   * opened is never toggled shut.
   */
  async openOrchestration(project: string): Promise<void> {
    const actions = await $(`aria/Actions for ${project}`);
    await actions.waitForExist({ timeout: WAIT.render });

    const menu = $('[role="menu"]');
    await browser.waitUntil(
      async () => {
        if (await menu.isDisplayed()) return true;
        await browser.execute(
          (element: HTMLElement) => element.focus(),
          actions,
        );
        await browser.keys("Enter");
        return menu.isDisplayed();
      },
      {
        timeout: WAIT.render,
        timeoutMsg: `the "${project}" actions menu never opened`,
      },
    );

    // Scoped to the open menu rather than looked up globally: the pane this opens also carries an
    // accessible "Orchestration views" name once rendered, so a global name lookup could mis-target
    // on a re-open. Exact text keeps the match on the one menu item — the menu's wrapper holds
    // every label's text, so it can never match exactly.
    const item = await menu.$("div=Orchestration");
    await item.waitForClickable({ timeout: WAIT.render });
    await item.click();
  },

  /**
   * The row element itself, found by the text of its label span. The controls a spec clicks are
   * revealed for the selected row, so callers `select` before reaching for one; a control is
   * waited on until clickable, which is when the reveal has landed.
   */
  rowElement(label: string) {
    return $(NAV).$(
      `.//*[@role="treeitem"][./span[normalize-space(text())="${label}"]]`,
    );
  },

  /** Whether Start is offered at all. Irrelevant actions are absent rather than disabled. */
  async hasStart(label: string): Promise<boolean> {
    const control = await this.control(label, "Start");
    return control.isExisting();
  },

  async clickControl(label: string, action: RowControl): Promise<void> {
    await this.select(label);
    const control = await this.control(label, action);
    if (await control.isClickable()) {
      await control.click();
      return;
    }

    // Secondary actions are progressively disclosed in the row's shadcn dropdown. Open it via
    // the genuine keyboard path—the same reliable WebKit strategy used for project actions.
    const more = await this.rowElement(label).$(
      `.//button[@aria-label="More actions for ${label}"]`,
    );
    await more.waitForExist({ timeout: WAIT.render });
    await browser.execute((element: HTMLElement) => element.focus(), more);
    await browser.keys("Enter");
    const menu = $('[role="menu"]');
    await menu.waitForDisplayed({ timeout: WAIT.render });
    const item = await menu.$(`div=${action}`);
    await item.waitForClickable({ timeout: WAIT.render });
    await item.click();
  },

  control(label: string, action: RowControl) {
    return this.rowElement(label).$(`.//button[@aria-label="${action}"]`);
  },
};
