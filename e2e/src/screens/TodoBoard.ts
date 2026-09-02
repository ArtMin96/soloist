import { $, browser } from "@wdio/globals";
import type { TodoStatus } from "@domain";
import { WAIT } from "../harness/waits.js";
import { waitUntilOr } from "../harness/waitUntilOr.js";

// The to-do board: the project's shared work items, each a Collapsible row expanding to its
// document, blockers, comments, and actions. Selectors live only here, addressed by the row's
// `data-todo-*` handles rather than DOM position — the trigger and, on a locked row, the agent
// control are siblings, so neither structure nor styling can move a handle out from under this
// file.
//
// A row is found by the exact text of its title (`data-todo-title`), the same justified structural
// handle the sidebar uses for a process row.

/** The status labels the board renders (the single source is the UI's `lib/todo` TODO_STATUS map). */
const STATUS_LABEL: Record<TodoStatus, string> = {
  open: "Open",
  blocked: "Blocked",
  in_progress: "In progress",
  done: "Done",
};

/** The agent control a locked row carries: who it names, and the process it opens. */
interface TodoLock {
  owner: string;
  process: number;
}

/**
 * One todo row's readable state: the id the core assigned it, its declared status, whether the
 * blocked gate shows, and the lock the core reports (or `null` while it is unlocked).
 */
interface TodoState {
  id: number;
  status: string;
  blocked: boolean;
  lock: TodoLock | null;
}

/** How wide one box's content is against how wide the box is — a horizontal-fit reading. */
interface FitReading {
  what: string;
  scrollWidth: number;
  clientWidth: number;
}

/** The board and its rows. */
export const todoBoard = {
  /**
   * One todo row's state — its declared status label and derived blocked gate — read in one pass by
   * the exact title, re-querying the DOM so a live re-render (a completion elsewhere) cannot stale
   * the read. `null` when no row carries that title yet.
   */
  async read(title: string): Promise<TodoState | null> {
    return browser.execute((todoTitle: string) => {
      const item = [...document.querySelectorAll("[data-todo-id]")].find(
        (candidate) => candidate.querySelector("[data-todo-title]")?.textContent?.trim() === todoTitle,
      );
      if (!item) return null;
      const status = item.querySelector("[data-todo-status]");
      const agent = item.querySelector("[data-todo-agent]");
      return {
        id: Number(item.getAttribute("data-todo-id")),
        status: status?.getAttribute("data-status") ?? "",
        blocked: item.querySelector("[data-todo-blockers]") !== null,
        lock:
          agent === null
            ? null
            : {
                owner: agent.textContent?.trim() ?? "",
                process: Number(agent.getAttribute("data-process-id")),
              },
      };
    }, title);
  },

  /**
   * Waits until the row titled `title` carries its agent control — rendered only once the core
   * reports the todo locked — and returns who it names and which process it opens.
   */
  async waitForLock(title: string): Promise<TodoLock> {
    let lock: TodoLock | null | undefined;
    await waitUntilOr(
      async () => {
        lock = (await this.read(title))?.lock;
        return lock != null;
      },
      () =>
        `todo "${title}" never showed an agent control; last seen: ${
          lock === undefined ? "no such todo" : "unlocked"
        }`,
    );
    return lock as TodoLock;
  },

  /** The row's declared status as the domain enum — read directly off `data-status`. */
  async status(title: string): Promise<TodoStatus | null> {
    const state = await this.read(title);
    if (state === null) return null;
    return (state.status as TodoStatus) || null;
  },

  /** Waits until a row titled `title` is rendered. */
  async waitForTodo(title: string): Promise<void> {
    await waitUntilOr(
      async () => (await this.read(title)) !== null,
      async () =>
        `no todo titled "${title}" appeared; rendered todos: ${JSON.stringify(await this.titles())}`,
    );
  },

  /** Every rendered todo title, for reporting a miss. */
  async titles(): Promise<string[]> {
    return browser.execute(() =>
      [...document.querySelectorAll("[data-todo-title]")].map(
        (title) => title.textContent?.trim() ?? "",
      ),
    );
  },

  /** Waits until the row titled `title` reports the declared `status`. */
  async waitForStatus(title: string, status: TodoStatus): Promise<void> {
    let last: string | undefined;
    await waitUntilOr(
      async () => {
        last = (await this.read(title))?.status;
        return last === status;
      },
      () =>
        `todo "${title}" never reported status "${status}"; last seen: ${last ?? "no such todo"}`,
    );
  },

  /** Waits until the row titled `title` shows its blocked gate as `blocked`. */
  async waitForBlocked(title: string, blocked: boolean): Promise<void> {
    let last: boolean | undefined;
    await waitUntilOr(
      async () => {
        last = (await this.read(title))?.blocked;
        return last === blocked;
      },
      () =>
        `todo "${title}" never showed blocked=${blocked}; last seen: ${last ?? "no such todo"}`,
    );
  },

  /** Expands the row titled `title` so its content (actions, alert, comments) is present. */
  async expand(title: string): Promise<void> {
    const trigger = this.trigger(title);
    await trigger.waitForClickable({ timeout: WAIT.render });
    if ((await trigger.getAttribute("aria-expanded")) === "true") return;
    await trigger.click();
    await trigger.waitForClickable({ timeout: WAIT.render });
    await browser.waitUntil(
      async () => (await trigger.getAttribute("aria-expanded")) === "true",
      {
        timeout: WAIT.render,
      },
    );
  },

  /** Whether the row titled `title` is currently expanded. */
  async isExpanded(title: string): Promise<boolean> {
    return (await this.trigger(title).getAttribute("aria-expanded")) === "true";
  },

  /** Activates the row's agent control — the way a user opens the agent that locked it. */
  async openAgent(title: string): Promise<void> {
    const control = this.agentControl(title);
    await control.waitForClickable({ timeout: WAIT.render });
    await control.click();
  },

  /**
   * The board's horizontal fit, read in one layout pass: its toolbar and the scroll container the
   * rows live in, each with its content width against its own. A box whose content is wider than
   * itself is one the user would have to scroll sideways. The rows are measured through their
   * scroll container rather than one by one: a truncated title clips on purpose, and counting each
   * row would report the ellipsis as a defect.
   */
  async horizontalOverflow(): Promise<FitReading[]> {
    return browser.execute(() => {
      const fit = (what: string, box: Element | null) =>
        box === null
          ? []
          : [{ what, scrollWidth: box.scrollWidth, clientWidth: box.clientWidth }];
      // The board's own scroll container: the nearest ancestor of a row that actually scrolls,
      // past a group's clipping wrapper (which hides overflow to animate its disclosure).
      let scroller: Element | null = document.querySelector("[data-todo-id]");
      while (scroller && !/^(auto|scroll)$/.test(getComputedStyle(scroller).overflowX)) {
        scroller = scroller.parentElement;
      }
      return [
        ...fit("toolbar", document.querySelector("[data-todo-toolbar]")),
        ...fit("rows", scroller),
      ];
    });
  },

  /** Expands the row and clicks its Complete action — the write that routes to the core's gate. */
  async complete(title: string): Promise<void> {
    await this.expand(title);
    const button = this.itemElement(title).$("button=Complete");
    await button.waitForClickable({ timeout: WAIT.render });
    await button.click();
  },

  /**
   * Expands the row and waits for its refusal alert, returning its text — the core's verbatim
   * `TodoBlocked` message, surfaced when a blocked todo is completed (the UI never pre-empts it).
   */
  async waitForRefusal(title: string): Promise<string> {
    await this.expand(title);
    const alert = this.itemElement(title).$('[role="alert"]');
    await alert.waitForDisplayed({ timeout: WAIT.core });
    return alert.getText();
  },

  /** Expands the row and returns all of its rendered text — used to read a comment and its author. */
  async expandedText(title: string): Promise<string> {
    await this.expand(title);
    return this.itemElement(title).getText();
  },

  /** The `#<id>` text a row carries, read off its exact title. */
  async todoRef(title: string): Promise<string> {
    return this.itemElement(title).$("[data-todo-ref]").getText();
  },

  /** The unmet-blocker text a row carries (`"1 unmet blocker"` / `"<n> unmet blockers"`), or null. */
  async blockerText(title: string): Promise<string | null> {
    const el = this.itemElement(title).$("[data-todo-blockers]");
    if (!(await el.isExisting())) return null;
    return el.getText();
  },

  /**
   * The row element itself, found by the exact text of its title (the justified structural handle,
   * as in the sidebar), anchored on the `data-todo-id` list item the item uniquely carries.
   */
  itemElement(title: string) {
    return $(
      `//li[@data-todo-id][.//*[@data-todo-title][normalize-space(text())="${title}"]]`,
    );
  },

  /** The row's Collapsible trigger. */
  trigger(title: string) {
    return this.itemElement(title).$("[data-todo-trigger]");
  },

  /**
   * The row's agent control — present only while it is locked. A sibling of the trigger, never a
   * descendant, so activating it never also toggles the row.
   */
  agentControl(title: string) {
    return this.itemElement(title).$("[data-todo-agent]");
  },
};
