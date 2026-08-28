import type { ProcStatus } from "@domain";
import { $, browser } from "@wdio/globals";
import { waitUntilOr } from "../harness/waitUntilOr.js";
import { WAIT } from "../harness/waits.js";
import { ATTENTION_MARKER } from "./attention.js";
import { ROW_ACTIVITY, ROW_MARKER, ROW_STATUS, ROW_TEXT } from "./indicatorRow.js";
import { chooseFromMenu } from "./menu.js";
import { trustDialog } from "./TrustDialog.js";

const NAV = 'nav[aria-label="Projects"]';
const ROW = '[role="treeitem"]';
const META = '[data-testid="process-meta"]';
// The travelling highlight a working agent's name wears. Kept here rather than with the markers
// every process row shares: the orchestration tree renders the same rows and never sweeps them.
const SWEEP = ':scope > span [data-slot="text-shimmer"]';

// The indicator only ever swaps `data-status` out for `data-activity` while the process is
// Running (a stopped agent has no activity to report), so an activity marker *is* a Running
// status. Any row carrying neither marker means the markup changed — that must fail the read,
// never default it, or a status assertion could pass against a row that reports nothing.
const RUNNING: ProcStatus = "Running";
const STOPPED: ProcStatus = "Stopped";

// How many times Start is asked for before the harness calls it a refusal rather than a dropped
// click. More than one because a click can be swallowed; few, because a start the core refuses
// will not take however often it is asked.
const START_ATTEMPTS = 3;

/** The per-row control cluster's actions, by their accessible names. */
type RowControl =
  | "Trust"
  | "Resume last session"
  | "Start"
  | "Stop"
  | "Restart"
  | "Remove";

/** One process row as the sidebar renders it. */
export interface RowHandle {
  label: string;
  status: ProcStatus;
  selected: boolean;
  /** The first discovered port the row's telemetry shows, or `null` while it shows none. */
  port: number | null;
  /** Whether the row wears the unread marker — something happened here nobody has looked at. */
  unread: boolean;
}

/** The highlight travelling across a working agent's name, as the window has laid it out. */
export interface Sweep {
  /** The name the highlight travels across, read from the text it is painted over. */
  label: string;
  /** The highlight's own box — what its mask is sized and travelled in percentages of. */
  overlay: number;
  /** The width of the name's glyph run, measured over the text itself. */
  ink: number;
  /** The width the row gives the name before it has to be clipped. */
  cell: number;
}

/** One sweeping row, before its name is known to have been readable. */
interface RawSweep extends Omit<Sweep, "label" | "ink"> {
  label: string | null;
  ink: number | null;
}

/** Every sweeping row in one pass, with what the window says about reduced motion. */
interface SweepReading {
  reducedMotion: boolean;
  sweeps: RawSweep[];
}

/** What one row's DOM carries, before the status/activity markers are resolved to a status. */
interface RowSnapshot {
  label: string;
  status: string | null;
  hasActivity: boolean;
  selected: boolean;
  meta: string | null;
  unread: boolean;
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

  /**
   * Whether the project tree is on screen at all — read rather than waited on, so it answers for
   * the frame it is asked about. The app mounts one React root: anything that throws while
   * rendering takes the root with it and leaves an empty window, which is indistinguishable from
   * a slow render until something asserts that the shell is still there.
   */
  async isRendered(): Promise<boolean> {
    return $(NAV).isDisplayed();
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
        marker: string,
      ) => {
        const tree = document.querySelector(nav);
        if (!tree) return [];
        return [...tree.querySelectorAll(row)].map((node) => {
          // A working agent's name is painted twice — a solid base and a copy the travelling
          // highlight sweeps over, hidden from the accessibility tree — so the name is read from
          // a clone with the hidden copies taken out. Reading the label whole reports the name
          // twice, and the row is then lost to every lookup by the name the user reads.
          const name = node.querySelector(label)?.cloneNode(true) as
            | HTMLElement
            | undefined;
          name?.querySelectorAll("[aria-hidden]").forEach((copy) => {
            copy.remove();
          });
          return {
            label: name?.textContent?.trim() ?? "",
            status:
              node.querySelector(status)?.getAttribute("data-status") ?? null,
            hasActivity: node.querySelector(activity) !== null,
            selected: node.getAttribute("aria-selected") === "true",
            // textContent rather than innerText: the read-out hides under the controls while the
            // row is selected or hovered, and a hidden element's innerText reads empty.
            meta: node.querySelector(meta)?.textContent ?? null,
            unread: node.querySelector(marker) !== null,
          };
        });
      },
      NAV,
      ROW,
      ROW_TEXT,
      ROW_STATUS,
      ROW_ACTIVITY,
      META,
      ROW_MARKER,
    );
    return snapshots.map(
      ({ label, status, hasActivity, selected, meta, unread }) => {
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
          unread,
        };
      },
    );
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

  /** Waits until the row labelled `label` is the selected one. */
  async waitForRowSelected(label: string): Promise<void> {
    let selected: string[] = [];
    await waitUntilOr(
      async () => {
        const rows = await this.rows();
        selected = rows.filter((row) => row.selected).map((row) => row.label);
        return selected.includes(label);
      },
      () =>
        `sidebar row "${label}" never became the selected one; selected: ${JSON.stringify(selected)}`,
    );
  },

  /**
   * Every row currently sweeping its name, measured in one pass.
   *
   * One pass because the three widths only mean something together: the highlight is positioned
   * against its own box, and a working agent's row re-renders as its activity changes, so widths
   * gathered a call apart can describe two different layouts. The name is measured with a range
   * over the text itself rather than over any box enclosing it — the highlight fills that box, so
   * comparing the two could only ever agree with itself.
   *
   * A sweeping row whose name cannot be read comes back with none: the read reports that rather
   * than substituting a width, so a change to the label markup fails loudly instead of quietly
   * measuring the wrong thing.
   */
  async sweeps(): Promise<SweepReading> {
    return browser.execute(
      (nav: string, row: string, sweep: string, text: string) => {
        const tree = document.querySelector(nav);
        const rows = tree === null ? [] : [...tree.querySelectorAll(row)];
        return {
          reducedMotion: window.matchMedia("(prefers-reduced-motion: reduce)")
            .matches,
          sweeps: rows.flatMap((node) => {
            const overlay = node.querySelector(sweep);
            const cell = node.querySelector(text);
            const box = overlay?.parentElement;
            if (!overlay || !cell || !box) return [];
            // The name's own glyph run: the first text under the highlight's box that the
            // highlight is not itself, since it carries a hidden copy of the same name.
            const walker = document.createTreeWalker(box, NodeFilter.SHOW_TEXT);
            let ink: number | null = null;
            let label: string | null = null;
            for (
              let written = walker.nextNode();
              written !== null;
              written = walker.nextNode()
            ) {
              const name = (written.textContent ?? "").trim();
              if (name === "") continue;
              if (written.parentElement?.closest("[aria-hidden]")) continue;
              const range = document.createRange();
              range.selectNodeContents(written);
              ink = range.getBoundingClientRect().width;
              label = name;
              break;
            }
            return [
              {
                label,
                ink,
                overlay: overlay.getBoundingClientRect().width,
                cell: cell.getBoundingClientRect().width,
              },
            ];
          }),
        };
      },
      NAV,
      ROW,
      SWEEP,
      ROW_TEXT,
    );
  },

  /**
   * Waits until the row labelled `label` is sweeping its name, and returns that sweep's geometry.
   *
   * The wait is on the agent really working: the sweep is rendered from the activity the idle
   * sampler classifies, so it comes and goes as the agent does, and a spec lands on whichever
   * working spell it arrives in.
   */
  async waitForSweep(label: string): Promise<Sweep> {
    let sweeping: (string | null)[] = [];
    let reducedMotion = false;
    let found: RawSweep | undefined;
    await waitUntilOr(
      async () => {
        const reading = await this.sweeps();
        reducedMotion = reading.reducedMotion;
        sweeping = reading.sweeps.map((sweep) => sweep.label);
        found = reading.sweeps.find((sweep) => sweep.label === label);
        return found !== undefined;
      },
      () =>
        `no sidebar row labelled "${label}" swept its name; sweeping rows: ${JSON.stringify(sweeping)}`,
    );
    const sweep = found as RawSweep;
    // Raised out here rather than from the wait, which reports its own message in place of any
    // error thrown inside it. A hidden highlight is still in the DOM and measures zero, and the
    // window drops it outright under reduced motion — so say so, rather than leave a bare zero to
    // be read as a geometry failure.
    if (sweep.overlay === 0) {
      throw new Error(
        reducedMotion
          ? `the sweep on sidebar row "${label}" measures zero because this window reports ` +
            `prefers-reduced-motion: reduce, which hides it — the suite needs a display whose ` +
            `animations are enabled`
          : `the sweep on sidebar row "${label}" measures zero width, and nothing reports reduced motion`,
      );
    }
    if (sweep.ink === null) {
      throw new Error(
        `sidebar row "${label}" sweeps a name the harness cannot measure — the label markup ` +
          `changed and the text the highlight travels across is no longer readable under it`,
      );
    }
    return { ...sweep, label, ink: sweep.ink };
  },

  /**
   * Moves keyboard focus onto the row labelled `label`, selecting it first.
   *
   * Selecting a terminal hands focus to its xterm pane, whose hidden textarea consumes
   * command-modifier chords before they reach the app's window-level hotkey handler — so a spec
   * that selects a terminal and then presses an app hotkey is really typing into the shell.
   * Focusing the row puts the keyboard back where a sidebar user has it.
   */
  async focusRow(label: string): Promise<void> {
    await this.select(label);
    const row = await this.rowElement(label);
    await browser.execute((element: HTMLElement) => element.focus(), row);
  },

  /**
   * Selects the process labelled `label`, clicking its row and making sure the selection took.
   *
   * One click attempts a selection; it does not make one. WebKitGTK under WebDriver drops a click
   * outright when the app is busy, and a menu that has just closed leaves the page refusing
   * pointer events for a beat — so the click is repeated until the row reports itself selected,
   * which is safe because selecting sets rather than toggles. Nothing is clicked at all when the
   * row is already the selected one. Observed as a lost selection after a project's ••• menu, in a
   * full-suite run where the same walk passed alone.
   */
  async select(label: string): Promise<void> {
    // First prove the row is rendered at all — that failure names the rows that are — so a
    // clickability timeout below can only mean the row exists and something obscures it.
    await this.waitForRow(label);
    const row = await this.rowElement(label);
    await row.waitForClickable({ timeout: WAIT.render });
    let selected: string[] = [];
    await waitUntilOr(
      async () => {
        selected = (await this.rows())
          .filter((candidate) => candidate.selected)
          .map((candidate) => candidate.label);
        if (selected.includes(label)) return true;
        await row.click();
        return false;
      },
      () =>
        `clicking sidebar row "${label}" never made it the selected one; selected: ${JSON.stringify(selected)}`,
      WAIT.render,
    );
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

  /**
   * Starts the row's process, asking again if the row is still resting.
   *
   * The failure a repeat answers is a dropped click, not a refused start: WebKitGTK under
   * WebDriver swallows one outright when the app is busy, and Start is offered right up until the
   * process runs, so asking twice is only ever the same intent asked twice. A start the core
   * actually refuses is unmoved by any number of asks and still fails here.
   */
  async start(label: string): Promise<void> {
    for (let attempt = 1; attempt <= START_ATTEMPTS; attempt += 1) {
      await this.clickControl(label, "Start");
      const moved = await this.leftStopped(label);
      if (moved) return;
    }
    throw new Error(
      `Start was clicked ${START_ATTEMPTS} times on sidebar row "${label}" and it never left ${STOPPED}`,
    );
  },

  /** Whether the row leaves Stopped within a local render — how a click is seen to have landed. */
  async leftStopped(label: string): Promise<boolean> {
    try {
      await waitUntilOr(
        async () =>
          (await this.rows()).find((candidate) => candidate.label === label)
            ?.status !== STOPPED,
        () => "",
        WAIT.render,
      );
      return true;
    } catch {
      return false;
    }
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
   * Clicks Remove on the row's control cluster. Only raises the confirmation when the process is
   * still live; a resting one is removed outright, so callers decide which they are driving.
   */
  async remove(label: string): Promise<void> {
    await this.clickControl(label, "Remove");
  },

  /** Waits until no row labelled `label` is rendered — the process was forgotten, not just stopped. */
  async waitForRowGone(label: string): Promise<void> {
    let seen: string[] = [];
    await waitUntilOr(
      async () => {
        seen = (await this.rows()).map((row) => row.label);
        return !seen.includes(label);
      },
      () =>
        `sidebar row "${label}" never left; rendered rows: ${JSON.stringify(seen)}`,
    );
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
   * Whether the project's own header wears the unread marker — the dot that says something under
   * this project wants the user, whether or not the row that raised it is on screen.
   *
   * Read from the header's disclosure control, which is what holds both the project's name and its
   * dot; a header that cannot be found at all throws rather than reporting "no dot", so a markup
   * change can never answer a negative assertion for the app.
   */
  async projectUnread(name: string): Promise<boolean> {
    const marked: boolean | null = await browser.execute(
      (nav: string, project: string, marker: string) => {
        const tree = document.querySelector(nav);
        if (!tree) return null;
        const label = [...tree.querySelectorAll("span")].find(
          (span) => span.textContent?.trim() === project,
        );
        const header = label?.closest("button");
        if (!header) return null;
        return header.querySelector(marker) !== null;
      },
      NAV,
      name,
      ATTENTION_MARKER,
    );
    if (marked === null) {
      throw new Error(
        `no sidebar header found for project "${name}" — the harness cannot read its unread marker`,
      );
    }
    return marked;
  },

  /** Shows a project's orchestration pane, chosen from the project's own ••• menu. */
  async openOrchestration(project: string): Promise<void> {
    await this.chooseProjectAction(project, "Orchestration");
  },

  /** Shows a project's settings pane, chosen from the project's own ••• menu. */
  async openProjectSettings(project: string): Promise<void> {
    await this.chooseProjectAction(project, "Project settings");
  },

  /**
   * Runs one of a project's own actions the way a keyboard user does — reveal the project row's
   * ••• actions button, open its menu, and choose the item.
   */
  async chooseProjectAction(project: string, action: string): Promise<void> {
    await chooseFromMenu(
      `"${project}" actions`,
      () => $(`aria/Actions for ${project}`),
      action,
    );
  },

  /**
   * The row element itself, found by the text of its label span. The controls a spec clicks are
   * revealed for the selected row, so callers `select` before reaching for one; a control is
   * waited on until clickable, which is when the reveal has landed.
   *
   * The name is matched wherever it sits under the row rather than as a direct child: the label
   * carries its own box for the working-agent sweep, so the text is nested below the label span
   * rather than directly inside it. The sweep's hidden copy of the name is excluded, so the row
   * stays addressed by the name the user actually reads.
   */
  rowElement(label: string) {
    return $(NAV).$(
      `.//*[@role="treeitem"][.//span[not(@aria-hidden)][normalize-space(text())="${label}"]]`,
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

    // Secondary actions are progressively disclosed in the row's shadcn dropdown, opened the same
    // way a project's own menu is.
    await chooseFromMenu(
      `"${label}" more actions`,
      () =>
        this.rowElement(label).$(
          `.//button[@aria-label="More actions for ${label}"]`,
        ),
      action,
    );
  },

  control(label: string, action: RowControl) {
    return this.rowElement(label).$(`.//button[@aria-label="${action}"]`);
  },
};
