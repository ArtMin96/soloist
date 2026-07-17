import { $, browser } from "@wdio/globals";
import type { TodoStatus } from "@domain";
import { WAIT } from "../harness/waits.js";

// The to-do board: the project's shared work items, each a Collapsible row expanding to its
// document, blockers, comments, and actions. Selectors live only here. State (the declared status
// label and the derived blocked gate) is read from the always-visible trigger; actions and the
// expanded content (Complete, the refusal alert, comments) need the row expanded first.
//
// A todo row has no accessible name distinct from its title, so it is found by the exact text of its
// title span — the same justified structural handle the sidebar uses for a process row. The trigger
// is anchored as the Collapsible button (`aria-expanded`), which the item uniquely carries.

/** The status labels the board renders (the single source is the UI's `lib/todo` TODO_STATUS map). */
const STATUS_LABEL: Record<TodoStatus, string> = {
  open: "Open",
  blocked: "Blocked",
  in_progress: "In progress",
  done: "Done",
};

/** One todo row's readable state: its declared status label and whether the blocked gate shows. */
interface TodoState {
  status: string;
  blocked: boolean;
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
      // The row's trigger is the Collapsible button whose first plain span (no badge marker) is the
      // title; the last plain span is the declared status label. The derived blocked gate is an
      // outline badge, distinct from the muted lock badge and from the status label.
      const triggers = [...document.querySelectorAll('button[aria-expanded]')];
      for (const trigger of triggers) {
        const plain = [...trigger.children].filter(
          (child) => child.tagName === "SPAN" && !child.hasAttribute("data-slot"),
        );
        if (plain[0]?.textContent?.trim() !== todoTitle) continue;
        return {
          status: plain[plain.length - 1]?.textContent?.trim() ?? "",
          blocked: trigger.querySelector('[data-slot="badge"][data-variant="outline"]') !== null,
        };
      }
      return null;
    }, title);
  },

  /** The row's declared status as the domain enum, mapped back from its rendered label. */
  async status(title: string): Promise<TodoStatus | null> {
    const state = await this.read(title);
    if (state === null) return null;
    const entry = (Object.entries(STATUS_LABEL) as [TodoStatus, string][]).find(
      ([, label]) => label === state.status,
    );
    return entry?.[0] ?? null;
  },

  /** Waits until a row titled `title` is rendered. */
  async waitForTodo(title: string): Promise<void> {
    let seen: string[] = [];
    try {
      await browser.waitUntil(async () => (await this.read(title)) !== null, {
        timeout: WAIT.core,
      });
    } catch {
      seen = await this.titles();
      throw new Error(`no todo titled "${title}" appeared; rendered todos: ${JSON.stringify(seen)}`);
    }
  },

  /** Every rendered todo title, for reporting a miss. */
  async titles(): Promise<string[]> {
    return browser.execute(() =>
      [...document.querySelectorAll('button[aria-expanded]')]
        .map((trigger) => {
          const plain = [...trigger.children].filter(
            (child) => child.tagName === "SPAN" && !child.hasAttribute("data-slot"),
          );
          // Only rows carrying a declared status label (two plain spans) are todos.
          return plain.length >= 2 ? (plain[0].textContent?.trim() ?? "") : null;
        })
        .filter((title): title is string => title !== null),
    );
  },

  /** Waits until the row titled `title` reports the declared `status`. */
  async waitForStatus(title: string, status: TodoStatus): Promise<void> {
    const want = STATUS_LABEL[status];
    let last: string | undefined;
    try {
      await browser.waitUntil(
        async () => {
          last = (await this.read(title))?.status;
          return last === want;
        },
        { timeout: WAIT.core },
      );
    } catch {
      throw new Error(
        `todo "${title}" never reported status "${want}"; last seen: ${last ?? "no such todo"}`,
      );
    }
  },

  /** Waits until the row titled `title` shows its blocked gate as `blocked`. */
  async waitForBlocked(title: string, blocked: boolean): Promise<void> {
    let last: boolean | undefined;
    try {
      await browser.waitUntil(
        async () => {
          last = (await this.read(title))?.blocked;
          return last === blocked;
        },
        { timeout: WAIT.core },
      );
    } catch {
      throw new Error(
        `todo "${title}" never showed blocked=${blocked}; last seen: ${last ?? "no such todo"}`,
      );
    }
  },

  /** Expands the row titled `title` so its content (actions, alert, comments) is present. */
  async expand(title: string): Promise<void> {
    const trigger = this.trigger(title);
    await trigger.waitForClickable({ timeout: WAIT.render });
    if ((await trigger.getAttribute("aria-expanded")) === "true") return;
    await trigger.click();
    await trigger.waitForClickable({ timeout: WAIT.render });
    await browser.waitUntil(async () => (await trigger.getAttribute("aria-expanded")) === "true", {
      timeout: WAIT.render,
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

  /**
   * The row element itself, found by the exact text of its title span (the justified structural
   * handle, as in the sidebar), anchored on the Collapsible trigger the item uniquely carries.
   */
  itemElement(title: string) {
    return $(`//li[.//button[@aria-expanded][.//span[normalize-space(text())="${title}"]]]`);
  },

  /** The row's Collapsible trigger — the only `aria-expanded` button within the item. */
  trigger(title: string) {
    return this.itemElement(title).$("button[aria-expanded]");
  },
};
