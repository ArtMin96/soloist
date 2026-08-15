import { $, browser } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";
import { waitUntilOr } from "../harness/waitUntilOr.js";
import { ATTENTION_NAME } from "./attention.js";

// The unread count in the window chrome. Absent — not zero — when nothing is waiting, so its very
// presence is part of what a spec reads.
const CONTROL = `button[aria-label="${ATTENTION_NAME}"]`;
const POPOVER = '[data-slot="popover-content"]';
const ENTRY = "li button";
// The entry's own text, past the kind glyph that sits beside it hidden from assistive tech.
const ENTRY_LABEL = "span:not([aria-hidden])";
const CLEAR_ALL = "button=Clear all";

/** The title bar's unread count, the list behind it, and clearing everything from there. */
export const attentionControl = {
  /**
   * How many alerts the count reads, or `null` while the control is absent. The reading is the
   * app's own — capped at "99+" — so an uncapped number here means the cap did not apply.
   */
  async count(): Promise<number | null> {
    const control = await $(CONTROL);
    if (!(await control.isExisting())) return null;
    const text = (await control.getText()).trim();
    const total = Number.parseInt(text, 10);
    if (Number.isNaN(total)) {
      throw new Error(
        `the unread count read ${JSON.stringify(text)}, which is not a number`,
      );
    }
    return total;
  },

  /** Waits until the count reads `total` — an unread change is a round trip through the core. */
  async waitForCount(total: number): Promise<void> {
    let last: number | null = null;
    await waitUntilOr(
      async () => {
        last = await this.count();
        return last === total;
      },
      () =>
        `the title bar never showed ${total} unread; last read: ${last === null ? "no count at all" : last}`,
    );
  },

  /** Waits until no count is shown at all — nothing is waiting on the user any more. */
  async waitUntilAbsent(): Promise<void> {
    let last: number | null = null;
    await waitUntilOr(
      async () => {
        last = await this.count();
        return last === null;
      },
      () => `the title bar still showed ${last} unread`,
    );
  },

  /** Opens the list behind the count. */
  async open(): Promise<void> {
    const control = await $(CONTROL);
    await control.waitForClickable({ timeout: WAIT.render });
    await control.click();
    await $(POPOVER).waitForDisplayed({ timeout: WAIT.render });
  },

  /**
   * The processes the open list names, in the order it shows them. Read in one pass: the list
   * re-renders on every unread change, and one snapshot cannot tear.
   */
  async entries(): Promise<string[]> {
    const labels: string[] | null = await browser.execute(
      (popover: string, entry: string, label: string) => {
        const list = document.querySelector(popover);
        if (!list) return null;
        return [...list.querySelectorAll(entry)].map(
          (node) => node.querySelector(label)?.textContent?.trim() ?? "",
        );
      },
      POPOVER,
      ENTRY,
      ENTRY_LABEL,
    );
    if (labels === null) {
      throw new Error("the unread list is not open");
    }
    return labels;
  },

  /** Dismisses everything unread from the open list. */
  async clearAll(): Promise<void> {
    const clear = await $(POPOVER).$(CLEAR_ALL);
    await clear.waitForClickable({ timeout: WAIT.render });
    await clear.click();
  },
};
